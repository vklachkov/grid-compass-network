use std::sync::Arc;

use bstr::BStr;
use log::{debug, error, info, warn};
use rusqlite::Connection;

use super::data_frame::{ConnectHeader, DataFrameRequest, DataFrameResponse, SignOnProperty};
use crate::{
    db,
    shared::FrameError,
    vfs::VfsDirManager,
    vipc::{
        OutgoingMessage, Vipc,
        protocol::{property, status},
        sentry::Authority,
    },
};

const STATUS_INVALID_PASSWORD: u16 = 1003;
const STATUS_UNKNOWN_USER: u16 = 1005;
const STATUS_NOT_SIGNED_ON: u16 = 801;

pub(super) struct Session {
    vipc: Option<Box<Vipc>>,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
}

pub(super) enum Response {
    Control(DataFrameResponse<'static>),
    Message {
        header: ConnectHeader,
        message: OutgoingMessage,
    },
}

impl Response {
    pub(super) fn write_into(
        &self,
        message: &mut Vec<u8>,
        body: &mut Vec<u8>,
    ) -> Result<(), FrameError> {
        body.clear();
        match self {
            Self::Control(response) => response.write_into(body),
            Self::Message {
                header,
                message: outgoing,
            } => {
                message.clear();
                outgoing.write_into(message)?;
                DataFrameResponse::Msg {
                    header: *header,
                    payload: message,
                }
                .write_into(body)
            }
        }
    }
}

impl Session {
    pub(super) fn new(conn: Arc<db::Database>, vfs_root: Arc<VfsDirManager>) -> Self {
        Self {
            vipc: None,
            conn,
            vfs_root,
        }
    }

    #[rustfmt::skip]
    pub(super) fn process_data_frame(&mut self, data: &[u8]) -> Result<Vec<Response>, FrameError> {
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

    fn connect(&mut self, remote_path_id: u16, path: &BStr) -> Result<Vec<Response>, FrameError> {
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

        Ok(vec![Response::Control(body)])
    }

    fn disconnect(
        &mut self,
        remote_path_id: u16,
        _reason: u16,
    ) -> Result<Vec<Response>, FrameError> {
        debug!(target: "session", "requested disconnect");

        // TODO: proper disconnect from resource.

        let body = DataFrameResponse::Disconnect {
            header: ConnectHeader {
                local_path_id: 1,
                remote_path_id,
            },
        };

        Ok(vec![Response::Control(body)])
    }

    fn sign_on(
        &mut self,
        properties: Vec<SignOnProperty<'_>>,
    ) -> Result<Vec<Response>, FrameError> {
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

        Ok(vec![Response::Control(body)])
    }

    fn sign_off(&mut self) -> Result<Vec<Response>, FrameError> {
        info!(target: "session", "requested sign off");

        // Dropping the servers is the whole of it: the open files, the mailbox
        // and the authority all belonged to the account that signed on, and the
        // link is back to where it was before it ever did.
        self.vipc = None;

        Ok(Vec::new())
    }

    fn process_msg(
        &mut self,
        header: ConnectHeader,
        payload: &[u8],
    ) -> Result<Vec<Response>, FrameError> {
        let Some(vipc) = self.vipc.as_mut() else {
            warn!(target: "session", "ignored a message before sign-on");
            return Ok(Vec::new());
        };

        Ok(vipc
            .process_message(payload)?
            .into_iter()
            .map(|message| Response::Message {
                header: ConnectHeader {
                    local_path_id: header.remote_path_id,
                    remote_path_id: header.local_path_id,
                },
                message,
            })
            .collect())
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
    use tempfile::TempDir;

    use super::*;

    use super::sign_on_properties as properties;

    fn session() -> (TempDir, Session) {
        let root = TempDir::new().expect("create test FS root");
        let vfs_root = Arc::new(VfsDirManager::new(root.path()).expect("open the test FS root"));
        (
            root,
            Session::new(db::users::tests::demo_database(), vfs_root),
        )
    }

    fn response_bytes(responses: Vec<Response>) -> Vec<u8> {
        assert_eq!(responses.len(), 1);
        let mut message = Vec::new();
        let mut body = Vec::new();
        responses[0]
            .write_into(&mut message, &mut body)
            .expect("serialize the response");
        body
    }

    fn connect_status(session: &mut Session) -> u16 {
        let body = response_bytes(
            session
                .connect(1, BStr::new(b"Sentry"))
                .expect("answer the connect"),
        );
        u16::from_le_bytes([body[6], body[7]])
    }

    fn sign_on_status(session: &mut Session, password: &[u8]) -> u16 {
        let body = response_bytes(
            session
                .sign_on(properties(b"GRiD", b"Systems", b"MANAGER", password))
                .expect("answer the sign on"),
        );
        u16::from_le_bytes([body[2], body[3]])
    }

    #[test]
    fn refuses_a_connect_before_sign_on() {
        let (_fs_root, mut session) = session();

        assert_eq!(connect_status(&mut session), STATUS_NOT_SIGNED_ON);
    }

    #[test]
    fn refuses_a_connect_after_a_failed_sign_on() {
        let (_fs_root, mut session) = session();

        assert_eq!(
            sign_on_status(&mut session, b"WRONG"),
            STATUS_INVALID_PASSWORD
        );
        assert_eq!(connect_status(&mut session), STATUS_NOT_SIGNED_ON);
    }

    #[test]
    fn accepts_a_connect_once_signed_on() {
        let (_fs_root, mut session) = session();

        assert_eq!(sign_on_status(&mut session, b"MANAGER"), status::OK);
        assert_eq!(connect_status(&mut session), status::OK);
    }

    /// Signing off has to put the link back where it started, or an account
    /// would keep its reach over a link somebody else can now sign on to.
    #[test]
    fn signing_off_closes_the_gate_again() {
        let (_fs_root, mut session) = session();

        assert_eq!(sign_on_status(&mut session, b"MANAGER"), status::OK);
        session.sign_off().expect("sign off");

        assert_eq!(connect_status(&mut session), STATUS_NOT_SIGNED_ON);
    }

    /// A message carries no status field to refuse through, so the refusal can
    /// only be silence — and silence is what has to be asserted.
    #[test]
    fn drops_a_message_before_sign_on() {
        let (_fs_root, mut session) = session();

        let header = ConnectHeader {
            local_path_id: 1,
            remote_path_id: 1,
        };

        let responses = session
            .process_msg(header, &[0xFF, 0xFF, 0, 0, 0, 0])
            .expect("drop the message");
        assert!(responses.is_empty());
    }

    #[test]
    fn dispatches_commands_and_returns_owned_responses_without_a_link() {
        let (_root, mut session) = session();
        assert_eq!(sign_on_status(&mut session, b"MANAGER"), status::OK);

        let responses = session
            .process_data_frame(&[0, 0, 0x25, 0x7e, 1, 0, 0xff, 0xff, 0xff, 0xff, 1, 0, 4])
            .expect("dispatch a Sentry variant query");
        assert!(
            session
                .process_data_frame(&[7, 0])
                .expect("sign off")
                .is_empty()
        );

        assert_eq!(
            response_bytes(responses),
            [
                0, 0, 1, 0, 0x25, 0x7e, 0xff, 0xff, 0xff, 0xff, 4, 0, 3, 0x25, 1, b'3'
            ]
        );
        assert!(
            session
                .process_data_frame(&[0, 0, 0x25, 0x7e, 1, 0, 0xff, 0xff, 0xff, 0xff, 1, 0, 4])
                .expect("ignore an unauthenticated message")
                .is_empty()
        );
    }

    #[test]
    fn serializing_a_control_response_replaces_the_previous_message_body() {
        let (_root, mut session) = session();
        assert_eq!(sign_on_status(&mut session, b"MANAGER"), status::OK);
        let messages = session
            .process_data_frame(&[0, 0, 0x25, 0x7e, 1, 0, 0xff, 0xff, 0xff, 0xff, 1, 0, 4])
            .expect("dispatch a message");
        let disconnect = session
            .process_data_frame(&[3, 0, 0x25, 0x7e, 1, 0, 0, 0])
            .expect("disconnect a resource");
        assert_eq!(messages.len(), 1);
        assert_eq!(disconnect.len(), 1);
        let mut message = vec![0xaa; 128];
        let mut body = vec![0xbb; 128];
        messages[0]
            .write_into(&mut message, &mut body)
            .expect("serialize a message");
        assert_eq!(
            body,
            [
                0, 0, 1, 0, 0x25, 0x7e, 0xff, 0xff, 0xff, 0xff, 4, 0, 3, 0x25, 1, b'3'
            ]
        );
        disconnect[0]
            .write_into(&mut message, &mut body)
            .expect("serialize disconnect");
        assert_eq!(body, [4, 0, 1, 0, 0x25, 0x7e]);
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
