mod data_frame;
mod frame;
mod link;
mod raw_frame;

#[cfg(test)]
mod behavior_tests;

pub use data_frame::{ConnectHeader, DataFrameRequest, DataFrameResponse, SignOnProperty};
#[cfg(test)]
use frame::{Frame, FrameBody};
use link::{PdlLink, next_initial_seq_number};
#[cfg(test)]
use raw_frame::RawFrame;
use std::{
    any::Any,
    io,
    num::NonZeroU8,
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    thread,
};

use bstr::BStr;
use log::{debug, error, info, warn};
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::db;
use crate::shared::{FrameError, bitmap::IdMap8};
use crate::transport::Transport;
use crate::vfs::VfsDirManager;
use crate::vipc::{
    Vipc,
    protocol::{property, status},
    sentry::Authority,
};

const STATUS_INVALID_PASSWORD: u16 = 1003; // eInvalidPassword
const STATUS_UNKNOWN_USER: u16 = 1005; // eUnknownUser
const STATUS_NOT_SIGNED_ON: u16 = 801; // eUserNotSignedON

/// A connection id only has to be unique among the clients currently attached,
/// so it is handed back to the pool as soon as the session holding it is gone.
struct ConnectionId {
    id: NonZeroU8,
    pool: Arc<Mutex<IdMap8>>,
}

impl ConnectionId {
    fn acquire(pool: &Arc<Mutex<IdMap8>>) -> Option<Self> {
        let mut ids = pool.lock();
        let id = ids.find_free()?;
        ids.set(id);

        Some(Self {
            id,
            pool: pool.clone(),
        })
    }

    fn get(&self) -> u8 {
        self.id.get()
    }
}

impl Drop for ConnectionId {
    fn drop(&mut self) {
        self.pool.lock().clear(self.id);
    }
}

pub struct Server {
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
    connection_ids: Arc<Mutex<IdMap8>>,
}

impl Server {
    pub fn new(conn: Arc<db::Database>, vfs_root: Arc<VfsDirManager>) -> Self {
        Self {
            conn,
            vfs_root,
            connection_ids: Arc::new(Mutex::new(IdMap8::new())),
        }
    }

    pub fn spawn(&self, transport: impl Transport, peer: String) {
        let Some(connection_id) = ConnectionId::acquire(&self.connection_ids) else {
            error!(target: "server", "rejected client {peer}: no free connection id");
            return;
        };

        info!(target: "server", "accepted client {peer} as connection {}", connection_id.get());
        let conn = self.conn.clone();
        let vfs_root = self.vfs_root.clone();
        thread::spawn(move || worker(transport, peer, connection_id, conn, vfs_root));
    }
}

fn worker(
    transport: impl Transport,
    peer: String,
    connection_id: ConnectionId,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
) {
    // A session is the only thing a bug in request handling is allowed to cost:
    // unwinding here drops the transport and returns the connection id, and the
    // client is free to connect again.
    let served = panic::catch_unwind(AssertUnwindSafe(|| {
        try_worker(peer.clone(), transport, connection_id, conn, vfs_root)
    }));

    match served {
        Ok(Ok(())) => {}
        Ok(Err(err)) => error!(target: "server", "worker({peer}): fatal error: {err}"),
        Err(payload) => {
            error!(target: "server", "worker({peer}): panicked: {}", panic_reason(&payload));
        }
    }
}

fn panic_reason(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown reason")
}

fn try_worker(
    peer: String,
    transport: impl Transport,
    connection_id: ConnectionId,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
) -> io::Result<()> {
    let initial_seq_number = next_initial_seq_number();

    let session = Session {
        peer,
        link: PdlLink::new(transport, connection_id, initial_seq_number),
        vipc: None,
        conn,
        vfs_root,
        scratch: Scratch::default(),
    };

    session.run()
}

struct Session<T: Transport> {
    peer: String,
    link: PdlLink<T>,
    /// Built by sign-on and dropped by sign-off: every resource this server
    /// offers is reached through it, so an unauthenticated link has nothing to
    /// address at all.
    vipc: Option<Box<Vipc>>,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
    scratch: Scratch,
}

#[derive(Default)]
struct Scratch {
    message: Vec<u8>,
    body: Vec<u8>,
}

impl<T: Transport> Session<T> {
    fn run(mut self) -> io::Result<()> {
        loop {
            match self.link.receive_data() {
                Ok(Some(data)) => {
                    self.process_data_frame(&data).map_err(|err| match err {
                        FrameError::Io(err) => err,
                        err => io::Error::other(err),
                    })?;
                }
                Ok(None) => {
                    info!(target: "server", "worker({peer}): connection closed", peer=self.peer);
                    return Ok(());
                }
                Err(FrameError::Io(err)) => {
                    return Err(err);
                }
                Err(err) => {
                    return Err(io::Error::other(err));
                }
            }
        }
    }

    #[rustfmt::skip]
    fn process_data_frame(&mut self, data: &[u8]) -> Result<(), FrameError> {
        let req = DataFrameRequest::try_from_slice(data)?;

        debug!(target: "session", "received request {req:?}");

        match req {
            DataFrameRequest::Connect { header, path } => {
                self.connect(header.local_path_id, path)
            }
            DataFrameRequest::Disconnect { header, reason } => {
                self.disconnect(header.local_path_id, reason)
            }
            DataFrameRequest::SignOn { properties } => {
                self.sign_on(properties)
            }
            DataFrameRequest::SignOff {} => {
                self.sign_off()
            }
            DataFrameRequest::Msg { header, payload } => {
                self.process_msg(header, payload)
            }
        }
    }

    fn connect(&mut self, remote_path_id: u16, path: &BStr) -> Result<(), FrameError> {
        debug!(target: "session", "requested connect to {path}");

        // TODO: proper connect to resource.

        let status = if self.vipc.is_some() {
            status::OK
        } else {
            warn!(target: "session", "refused a connect before sign-on");
            STATUS_NOT_SIGNED_ON
        };

        let body = DataFrameResponse::Connect {
            header: ConnectHeader {
                local_path_id: 1,
                remote_path_id,
            },
            status,
        };

        self.scratch.body.clear();
        body.write_into(&mut self.scratch.body)?;

        self.send_response()
    }

    fn disconnect(&mut self, remote_path_id: u16, _reason: u16) -> Result<(), FrameError> {
        debug!(target: "session", "requested disconnect");

        // TODO: proper disconnect from resource.

        let body = DataFrameResponse::Disconnect {
            header: ConnectHeader {
                local_path_id: 1,
                remote_path_id,
            },
        };

        self.scratch.body.clear();
        body.write_into(&mut self.scratch.body)?;

        self.send_response()
    }

    fn sign_on(&mut self, properties: Vec<SignOnProperty<'_>>) -> Result<(), FrameError> {
        let status = match authenticate(&self.conn.get_conn(), &properties) {
            Ok(account) => {
                let authority = Authority::from_stored(account.authority);
                info!(
                    target: "session",
                    "signed on as {}/{}/{}, authority {authority} ({})",
                    account.company,
                    account.group,
                    account.user,
                    authority.name(),
                );

                self.vipc = Some(Box::new(Vipc::new(
                    self.conn.clone(),
                    account,
                    self.vfs_root.clone(),
                )?));

                status::OK
            }
            Err(status) => status,
        };

        let body = DataFrameResponse::SignOn {
            status,
            server_name: BStr::new("vklachkov server"),
        };

        self.scratch.body.clear();
        body.write_into(&mut self.scratch.body)?;

        self.send_response()
    }

    fn sign_off(&mut self) -> Result<(), FrameError> {
        info!(target: "session", "requested sign off");

        // Dropping the servers is the whole of it: the open files, the mailbox
        // and the authority all belonged to the account that signed on, and the
        // link is back to where it was before it ever did.
        self.vipc = None;

        Ok(())
    }

    fn process_msg(&mut self, header: ConnectHeader, payload: &[u8]) -> Result<(), FrameError> {
        let Some(vipc) = self.vipc.as_mut() else {
            // There is no message-level status field to refuse through, and the
            // client cannot reach here on its own — it would have to have
            // ignored the refused connect — so the message is dropped.
            warn!(target: "session", "ignored a message before sign-on");
            return Ok(());
        };

        for outgoing in vipc.process_message(payload)? {
            let Scratch { message, body, .. } = &mut self.scratch;

            message.clear();
            outgoing.write_into(message)?;

            let response = DataFrameResponse::Msg {
                header: ConnectHeader {
                    local_path_id: header.remote_path_id,
                    remote_path_id: header.local_path_id,
                },
                payload: message,
            };

            body.clear();
            response.write_into(body)?;

            self.send_response()?;
        }

        Ok(())
    }

    fn send_response(&mut self) -> Result<(), FrameError> {
        self.link.send_data(&self.scratch.body)
    }
}

#[cfg(test)]
pub(crate) fn sign_on_properties<'a>(
    company: &'a [u8],
    group: &'a [u8],
    user: &'a [u8],
    password: &'a [u8],
) -> Vec<SignOnProperty<'a>> {
    vec![
        SignOnProperty {
            ty: property::COMPANY,
            value: company,
        },
        SignOnProperty {
            ty: property::GROUP,
            value: group,
        },
        SignOnProperty {
            ty: property::USER,
            value: user,
        },
        SignOnProperty {
            ty: property::PASSWORD,
            value: password,
        },
    ]
}

/// The client shows any nonzero status as an error dialog, so the distinction
/// between an unknown account and a wrong password is what the user sees.
pub(crate) fn authenticate(
    conn: &Connection,
    properties: &[SignOnProperty<'_>],
) -> Result<db::Account, u16> {
    let property = |ty: u8| {
        properties
            .iter()
            .find(|property| property.ty == ty)
            .map_or(&[][..], |property| property.value)
    };

    let (company, group, user, password) = (
        property(property::COMPANY),
        property(property::GROUP),
        property(property::USER),
        property(property::PASSWORD),
    );

    debug!(
        target: "session",
        "sign on {:?}/{:?}/{:?}",
        BStr::new(company),
        BStr::new(group),
        BStr::new(user),
    );

    // An empty name would match the empty columns of a company or group row, so
    // sign-on has to refuse it before it ever reaches the lookup.
    if company.is_empty() || group.is_empty() || user.is_empty() {
        return Err(status::PROPERTY_MISSING);
    }

    // Names are stored as ASCII text, so anything else cannot name an account
    // that exists — the lookup would simply have missed.
    let (Ok(company), Ok(group), Ok(user)) = (
        str::from_utf8(company),
        str::from_utf8(group),
        str::from_utf8(user),
    ) else {
        warn!(target: "session", "refused the non-UTF-8 name {:?}", BStr::new(user));
        return Err(STATUS_UNKNOWN_USER);
    };

    let account = match db::find_user(conn, company, group, user) {
        Ok(Some(account)) => account,
        Ok(None) => {
            warn!(target: "session", "unknown user {user:?}");
            return Err(STATUS_UNKNOWN_USER);
        }
        Err(err) => {
            error!(target: "session", "failed to look up the account: {err}");
            return Err(status::AUTHORIZATION_FILE);
        }
    };

    if account.password.as_bytes() != password {
        warn!(target: "session", "wrong password for {user:?}");
        return Err(STATUS_INVALID_PASSWORD);
    }

    Ok(account)
}

#[cfg(test)]
mod tests {
    use std::{io::Read, os::unix::net::UnixStream};

    use tempfile::TempDir;

    use super::*;

    use super::sign_on_properties as properties;

    fn session_pair() -> (TempDir, Session<UnixStream>, UnixStream) {
        let (transport, peer) = UnixStream::pair().expect("create a transport pair");

        let initial_seq_number = next_initial_seq_number();

        let fs_root = TempDir::new().expect("create test FS root");
        let vfs_root = Arc::new(VfsDirManager::new(fs_root.path()).expect("open the test FS root"));

        let session = Session {
            peer: "test".to_owned(),
            link: PdlLink::new(
                transport,
                ConnectionId::acquire(&Arc::new(Mutex::new(IdMap8::new())))
                    .expect("take a connection id from an empty pool"),
                initial_seq_number,
            ),
            vipc: None,
            conn: db::users::tests::demo_database(),
            vfs_root,
            scratch: Scratch::default(),
        };

        (fs_root, session, peer)
    }

    fn read_response(peer: &mut UnixStream) -> Vec<u8> {
        let raw = RawFrame::read_from_io(peer).expect("read a response frame");
        let frame = Frame::try_from_raw(&raw).expect("parse a response frame");

        match frame.body {
            FrameBody::Data(data) => data.to_vec(),
            body => panic!("expected a data frame, got {body:?}"),
        }
    }

    /// `<type:u16><local:u16><remote:u16><status:u16>`.
    fn connect_status(session: &mut Session<UnixStream>, peer: &mut UnixStream) -> u16 {
        acknowledge_response(session, peer);
        session
            .connect(1, BStr::new(b"Sentry"))
            .expect("answer the connect");

        let body = read_response(peer);
        u16::from_le_bytes([body[6], body[7]])
    }

    /// `<type:u16><status:u16><name>`.
    fn sign_on_status(
        session: &mut Session<UnixStream>,
        peer: &mut UnixStream,
        password: &[u8],
    ) -> u16 {
        acknowledge_response(session, peer);
        session
            .sign_on(properties(b"GRiD", b"Systems", b"MANAGER", password))
            .expect("answer the sign on");

        let body = read_response(peer);
        u16::from_le_bytes([body[2], body[3]])
    }

    fn acknowledge_response(session: &Session<UnixStream>, peer: &mut UnixStream) {
        let frame = session.link.next_response_ack();
        RawFrame::write_data_to_io(&frame, peer).expect("queue the response acknowledgement");
    }

    fn assert_silent(peer: &mut UnixStream) {
        peer.set_nonblocking(true).expect("stop blocking on reads");

        let read = peer.read(&mut [0; 1]);

        peer.set_nonblocking(false).expect("block on reads again");

        match read {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            other => panic!("expected no response, got {other:?}"),
        }
    }

    #[test]
    fn refuses_a_connect_before_sign_on() {
        let (_fs_root, mut session, mut peer) = session_pair();

        assert_eq!(
            connect_status(&mut session, &mut peer),
            STATUS_NOT_SIGNED_ON
        );
    }

    #[test]
    fn refuses_a_connect_after_a_failed_sign_on() {
        let (_fs_root, mut session, mut peer) = session_pair();

        assert_eq!(
            sign_on_status(&mut session, &mut peer, b"WRONG"),
            STATUS_INVALID_PASSWORD
        );
        assert_eq!(
            connect_status(&mut session, &mut peer),
            STATUS_NOT_SIGNED_ON
        );
    }

    #[test]
    fn accepts_a_connect_once_signed_on() {
        let (_fs_root, mut session, mut peer) = session_pair();

        assert_eq!(
            sign_on_status(&mut session, &mut peer, b"MANAGER"),
            status::OK
        );
        assert_eq!(connect_status(&mut session, &mut peer), status::OK);
    }

    /// Signing off has to put the link back where it started, or an account
    /// would keep its reach over a link somebody else can now sign on to.
    #[test]
    fn signing_off_closes_the_gate_again() {
        let (_fs_root, mut session, mut peer) = session_pair();

        assert_eq!(
            sign_on_status(&mut session, &mut peer, b"MANAGER"),
            status::OK
        );
        session.sign_off().expect("sign off");

        assert_eq!(
            connect_status(&mut session, &mut peer),
            STATUS_NOT_SIGNED_ON
        );
    }

    /// A message carries no status field to refuse through, so the refusal can
    /// only be silence — and silence is what has to be asserted.
    #[test]
    fn drops_a_message_before_sign_on() {
        let (_fs_root, mut session, mut peer) = session_pair();

        let header = ConnectHeader {
            local_path_id: 1,
            remote_path_id: 1,
        };

        session
            .process_msg(header, &[0xFF, 0xFF, 0, 0, 0, 0])
            .expect("drop the message");

        assert_silent(&mut peer);
    }

    #[test]
    fn accepts_a_known_user_and_reports_their_authority() {
        let conn = db::users::tests::demo_database();

        let account = authenticate(
            &conn.get_conn(),
            &properties(b"GRiD", b"Systems", b"MANAGER", b"MANAGER"),
        )
        .expect("MANAGER should be able to sign on");

        assert_eq!(
            Authority::from_stored(account.authority),
            Authority::SYSTEM_ADMIN
        );
    }

    /// The names the client sends need not match the stored case, so sign-on has
    /// to compare them the way the client itself does.
    #[test]
    fn accepts_a_known_user_whatever_the_case_of_the_names() {
        let conn = db::users::tests::demo_database();

        let account = authenticate(
            &conn.get_conn(),
            &properties(b"grid", b"demo", b"guest", b"GUEST"),
        )
        .expect("GUEST should be able to sign on whatever the case");

        assert_eq!(account.user, "GUEST");
        assert_eq!(Authority::from_stored(account.authority), Authority::NORMAL);
    }

    #[test]
    fn rejects_a_wrong_password() {
        let conn = db::users::tests::demo_database();

        let status = authenticate(
            &conn.get_conn(),
            &properties(b"GRiD", b"Systems", b"MANAGER", b"WRONG"),
        )
        .err()
        .expect("a wrong password should be refused");

        assert_eq!(status, STATUS_INVALID_PASSWORD);
    }

    #[test]
    fn rejects_an_unknown_user() {
        let conn = db::users::tests::demo_database();

        let status = authenticate(
            &conn.get_conn(),
            &properties(b"GRiD", b"Demo", b"NOBODY", b""),
        )
        .err()
        .expect("an unknown user should be refused");

        assert_eq!(status, STATUS_UNKNOWN_USER);
    }

    /// A group is not an account: its row has an empty user name, which would
    /// match an empty property and answer with the group's own authority.
    #[test]
    fn refuses_to_sign_on_as_a_group() {
        let conn = db::Database::open_in_memory();

        let status = authenticate(&conn.get_conn(), &properties(b"GRiD", b"Systems", b"", b""))
            .err()
            .expect("a group should not be an account");

        assert_eq!(status, status::PROPERTY_MISSING);
    }
}
