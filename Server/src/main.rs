use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    process::ExitCode,
    rc::Rc,
    thread,
};

use bstr::BStr;
use log::{debug, error, info, trace, warn};
use rusqlite::Connection;

use gridlink::*;
use protocol::{property, status};
use sentry::Authority;
use vipc::Vipc;

const STATUS_INVALID_PASSWORD: u16 = 1003; // eInvalidPassword
const STATUS_UNKNOWN_USER: u16 = 1005; // eUnknownUser
const STATUS_NOT_SIGNED_ON: u16 = 801; // eUserNotSignedON

mod broadcast;
mod db;
mod gridlink;
mod logger;
mod mail;
mod protocol;
mod sentry;
mod vfs;
mod vipc;

#[derive(PartialEq, Eq)]
enum ProcessFrameResult {
    Continue,
    Disconnect,
}

fn main() -> ExitCode {
    logger::init();

    match server() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(target: "server", "fatal error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn server() -> io::Result<()> {
    let addr = std::env::var("LISTEN_ADDR").map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "env var LISTEN_ADDR not found")
    })?;

    let db_path = std::env::var("DB_PATH")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "env var DB_PATH not found"))?;

    db::open(&db_path).map_err(io::Error::other)?;
    info!(target: "server", "using account database {db_path}");

    let listener = TcpListener::bind(&addr)?;

    info!(target: "server", "start GRiD server at {addr}");
    loop {
        match listener.accept() {
            Ok((client, addr)) => {
                info!(target: "server", "accepted client {addr}");
                let db_path = db_path.clone();
                thread::spawn(move || worker(client, addr, &db_path));
            }
            Err(err) => {
                error!(target: "server", "failed to accept client: {err}");
            }
        };
    }
}

fn worker(client: TcpStream, addr: SocketAddr, db_path: &str) {
    if let Err(err) = try_worker(client, addr, db_path) {
        error!(target: "server", "worker({addr}): fatal error: {err}");
    }
}

fn try_worker(client: TcpStream, addr: SocketAddr, db_path: &str) -> io::Result<()> {
    let conn = Rc::new(db::open(db_path).map_err(io::Error::other)?);

    let mut session = Session {
        client,
        // TODO: automatically choose the connection ID from the available IDs.
        connection_id: 0x7B,
        last_seq_number: 0x1C,
        recv_sequence: 0x1C,
        vipc: None,
        conn,
        scratch: Scratch::default(),
    };

    loop {
        match gridlink::RawFrame::read_from_io(&mut session.client) {
            Ok(frame) => {
                // println!("worker({addr}): received new frame");
                if session.process_frame(frame)? == ProcessFrameResult::Disconnect {
                    info!(target: "server", "worker({addr}): disconnect");
                    return Ok(());
                }
            }
            Err(FrameError::UnexpectedEof) => {
                info!(target: "server", "worker({addr}): connection closed");
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

struct Session {
    client: TcpStream,
    connection_id: u8,
    last_seq_number: u8,
    recv_sequence: u8,
    /// Built by sign-on and dropped by sign-off: every resource this server
    /// offers is reached through it, so an unauthenticated link has nothing to
    /// address at all.
    vipc: Option<Box<Vipc>>,
    conn: Rc<Connection>,
    /// Reused for serializing outgoing frames, one buffer per nesting level:
    /// the VIPC message, the data frame around it and the PDL frame around
    /// that. Frames are bounded by `MAX_FRAME_SIZE`, so after the first few
    /// responses these stop growing and the send path allocates nothing.
    scratch: Scratch,
}

#[derive(Default)]
struct Scratch {
    message: Vec<u8>,
    body: Vec<u8>,
    frame: Vec<u8>,
}

impl Session {
    fn process_frame(&mut self, raw: RawFrame) -> io::Result<ProcessFrameResult> {
        match self.process_frame_(raw) {
            Ok(result) => Ok(result),
            Err(FrameError::Io(err)) => Err(err),
            Err(err) => Err(io::Error::other(err)),
        }
    }

    fn process_frame_(&mut self, raw: RawFrame) -> Result<ProcessFrameResult, FrameError> {
        let frame = Frame::try_from_raw(&raw)?;

        trace!(target: "session", "received {frame:?}");

        match frame.body {
            FrameBody::Rfc(body) => {
                debug!(
                    target: "session",
                    "requested a connection, version {:?}, seq={}",
                    BStr::new(&body.version),
                    self.last_seq_number,
                );
                self.recv_sequence = self.last_seq_number;
                self.write_frame(Frame::rfc(self.connection_id, self.last_seq_number))?;
            }
            FrameBody::Ack(_) => {
                trace!(target: "session", "acknowledged seq={}", frame.seq_number);
            }
            FrameBody::Disc(_) => {
                debug!(target: "session", "requested a link disconnect");
                return Ok(ProcessFrameResult::Disconnect);
            }
            FrameBody::Ping(_) => {
                trace!(target: "session", "pinged, seq={}", self.recv_sequence);
                self.write_frame(Frame::ack(self.connection_id, self.recv_sequence))?;
            }
            FrameBody::Data(data) => {
                let expected = self.recv_sequence.wrapping_add(1);
                if frame.seq_number == self.recv_sequence {
                    trace!(
                        target: "session",
                        "ignored duplicate data frame seq={}",
                        frame.seq_number
                    );
                    return Ok(ProcessFrameResult::Continue);
                }
                if frame.seq_number != expected {
                    warn!(
                        target: "session",
                        "ignored out-of-sequence data frame seq={}, expected={expected}",
                        frame.seq_number
                    );
                    self.write_frame(Frame::ack(self.connection_id, self.recv_sequence))?;
                    return Ok(ProcessFrameResult::Continue);
                }

                self.recv_sequence = frame.seq_number;
                self.write_frame(Frame::ack(self.connection_id, self.recv_sequence))?;
                self.process_data_frame(data)?;
            }
        }

        Ok(ProcessFrameResult::Continue)
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

        self.write_response()
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

        self.write_response()
    }

    fn sign_on(&mut self, properties: Vec<SignOnProperty<'_>>) -> Result<(), FrameError> {
        let status = match authenticate(&self.conn, &properties) {
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

                self.vipc = Some(Box::new(Vipc::new(Rc::clone(&self.conn), account)));

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

        self.write_response()
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

            self.write_response()?;
        }

        Ok(())
    }

    fn write_response(&mut self) -> Result<(), FrameError> {
        self.last_seq_number = self.last_seq_number.wrapping_add(1);

        let Scratch { body, frame, .. } = &mut self.scratch;

        frame.clear();
        Frame::data(EOM_FLAG_ON, self.last_seq_number, body).write_into(frame);

        RawFrame::write_data_to_io(frame, &mut self.client)
    }

    fn write_frame(&mut self, frame: Frame<'_>) -> Result<(), FrameError> {
        self.scratch.frame.clear();
        frame.write_into(&mut self.scratch.frame);

        RawFrame::write_data_to_io(&self.scratch.frame, &mut self.client)
    }
}

#[cfg(test)]
fn sign_on_properties<'a>(
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
fn authenticate(conn: &Connection, properties: &[SignOnProperty<'_>]) -> Result<db::Account, u16> {
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
    use std::io::Read;

    use super::*;

    use super::sign_on_properties as properties;

    /// The gate lives in the session, not in `authenticate`, so binding it takes
    /// a real session — and a session answers into a socket. A loopback pair is
    /// the cheapest way to give it one and still read back what it wrote.
    fn loopback() -> (Session, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback listener");
        let addr = listener.local_addr().expect("read the loopback address");
        let peer = TcpStream::connect(addr).expect("connect to the loopback listener");
        let (client, _) = listener.accept().expect("accept the loopback connection");

        let session = Session {
            client,
            connection_id: 0x7B,
            last_seq_number: 0x1C,
            recv_sequence: 0x1C,
            vipc: None,
            conn: Rc::new(db::open_in_memory()),
            scratch: Scratch::default(),
        };

        (session, peer)
    }

    fn read_response(peer: &mut TcpStream) -> Vec<u8> {
        let raw = RawFrame::read_from_io(peer).expect("read a response frame");
        let frame = Frame::try_from_raw(&raw).expect("parse a response frame");

        match frame.body {
            FrameBody::Data(data) => data.to_vec(),
            body => panic!("expected a data frame, got {body:?}"),
        }
    }

    /// `<type:u16><local:u16><remote:u16><status:u16>`.
    fn connect_status(session: &mut Session, peer: &mut TcpStream) -> u16 {
        session
            .connect(1, BStr::new(b"Sentry"))
            .expect("answer the connect");

        let body = read_response(peer);
        u16::from_le_bytes([body[6], body[7]])
    }

    /// `<type:u16><status:u16><name>`.
    fn sign_on_status(session: &mut Session, peer: &mut TcpStream, password: &[u8]) -> u16 {
        session
            .sign_on(properties(b"GRiD", b"Systems", b"MANAGER", password))
            .expect("answer the sign on");

        let body = read_response(peer);
        u16::from_le_bytes([body[2], body[3]])
    }

    fn assert_silent(peer: &mut TcpStream) {
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
        let (mut session, mut peer) = loopback();

        assert_eq!(
            connect_status(&mut session, &mut peer),
            STATUS_NOT_SIGNED_ON
        );
    }

    #[test]
    fn refuses_a_connect_after_a_failed_sign_on() {
        let (mut session, mut peer) = loopback();

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
        let (mut session, mut peer) = loopback();

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
        let (mut session, mut peer) = loopback();

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
        let (mut session, mut peer) = loopback();

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
        let conn = db::open_in_memory();

        let account = authenticate(
            &conn,
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
        let conn = db::open_in_memory();

        let account = authenticate(&conn, &properties(b"grid", b"demo", b"guest", b"GUEST"))
            .expect("GUEST should be able to sign on whatever the case");

        assert_eq!(account.user, "GUEST");
        assert_eq!(Authority::from_stored(account.authority), Authority::NORMAL);
    }

    #[test]
    fn rejects_a_wrong_password() {
        let conn = db::open_in_memory();

        let status = authenticate(
            &conn,
            &properties(b"GRiD", b"Systems", b"MANAGER", b"WRONG"),
        )
        .err()
        .expect("a wrong password should be refused");

        assert_eq!(status, STATUS_INVALID_PASSWORD);
    }

    #[test]
    fn rejects_an_unknown_user() {
        let conn = db::open_in_memory();

        let status = authenticate(&conn, &properties(b"GRiD", b"Demo", b"NOBODY", b""))
            .err()
            .expect("an unknown user should be refused");

        assert_eq!(status, STATUS_UNKNOWN_USER);
    }

    /// A group is not an account: its row has an empty user name, which would
    /// match an empty property and answer with the group's own authority.
    #[test]
    fn refuses_to_sign_on_as_a_group() {
        let conn = db::open_in_memory();

        let status = authenticate(&conn, &properties(b"GRiD", b"Systems", b"", b""))
            .err()
            .expect("a group should not be an account");

        assert_eq!(status, status::PROPERTY_MISSING);
    }
}
