use std::{io, path::PathBuf, rc::Rc};

use log::{debug, warn};
use rusqlite::Connection;

use mail::{MailBroadcastServer, MailServer};
use sentry::SentryServer;
use vfs::{Vfs, VfsRequest};

use crate::{
    db,
    gridlink::vipc::{IncomingMessage, MessageType, OutgoingMessage, OutgoingMessageBody},
    shared::FrameError,
    vfs::FsProxy,
};

mod mail;
mod vfs;

pub mod protocol;
pub mod sentry;

const CLASS_VFS: MessageType = MessageType(83);
const CLASS_MAIL: MessageType = MessageType(0x7444);
const CLASS_BROADCAST: MessageType = MessageType(0x7000);
const CLASS_SENTRY: MessageType = MessageType(0xffff);

pub struct Vipc {
    vfs: Vfs<FsProxy>,
    mail: MailServer,
    mail_broadcast: MailBroadcastServer,
    sentry: SentryServer,
}

impl Vipc {
    pub fn new(conn: Rc<Connection>, actor: db::Account, fs_root: PathBuf) -> io::Result<Self> {
        Ok(Self {
            vfs: Vfs::new(FsProxy::new(&actor, fs_root)?),
            mail: MailServer::new(conn.clone(), actor.id, actor.user.clone()),
            mail_broadcast: MailBroadcastServer::new(),
            sentry: SentryServer::new(conn, actor),
        })
    }

    pub fn process_message(&mut self, payload: &[u8]) -> Result<Vec<OutgoingMessage>, FrameError> {
        let message = IncomingMessage::try_from_slice(payload)?;

        debug!(target: "vipc", "received vipc message: {message:?}");

        let responses = match message.body.ty {
            CLASS_VFS => {
                let request = VfsRequest::try_from_slice(message.body.payload)?;
                let mut response = Vec::new();
                self.vfs
                    .process_request(request)
                    .write_into(&mut response)?;

                vec![OutgoingMessage {
                    note: message.note,
                    body: OutgoingMessageBody {
                        ty: CLASS_VFS,
                        payload: response,
                    },
                }]
            }
            CLASS_MAIL => self
                .mail
                .process(message.note, message.body.payload)
                .unwrap_or_default()
                .into_iter()
                .map(|response| OutgoingMessage {
                    note: 0x8000 | response.note,
                    body: OutgoingMessageBody {
                        ty: CLASS_MAIL,
                        payload: response.payload,
                    },
                })
                .collect(),
            CLASS_SENTRY => self
                .sentry
                .process(message.body.payload)
                .into_iter()
                .map(|payload| OutgoingMessage {
                    note: message.note,
                    body: OutgoingMessageBody {
                        ty: CLASS_SENTRY,
                        payload,
                    },
                })
                .collect(),
            CLASS_BROADCAST => self
                .mail_broadcast
                .process(message.body.payload)
                .map(|payload| OutgoingMessage {
                    note: 0x8000 | message.note,
                    body: OutgoingMessageBody {
                        ty: CLASS_BROADCAST,
                        payload,
                    },
                })
                .into_iter()
                .collect(),
            ty => {
                warn!(target: "vipc", "ignored a message for the unknown class {ty:?}");
                Vec::new()
            }
        };

        Ok(responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn vipc() -> Vipc {
        let conn = Rc::new(db::open_in_memory());
        let actor = db::find_user(&conn, "GRiD", "Systems", "MANAGER")
            .expect("read the demo directory")
            .expect("MANAGER should exist");
        let fs_root = std::env::temp_dir().join(format!(
            "setochka-vfs-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&fs_root).expect("create test FS root");

        Vipc::new(conn, actor, fs_root).expect("create VIPC services")
    }

    #[test]
    fn serializes_mail_initialization_response() {
        let mut vipc = vipc();
        let request = [
            0x44, 0x74, 0, 0, 11, 0, 0, 5, 0, 0, 0xfe, 4, 0, b'a', 0x88, 0x2c, 1,
        ];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0x44, 0x74, 0, 0x80, 8, 0, 0, 5, 0, 0, 0xfe, 1, 0, b'z']
        );
    }

    #[test]
    fn serializes_broadcast_initialization_response() {
        let mut vipc = vipc();
        let request = [
            0x00, 0x70, 0, 0, 11, 0, 0, 1, 0, 0, 0xfe, 4, 0, b'a', 0xec, 0x2c, 1,
        ];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0x00, 0x70, 0, 0x80, 8, 0, 0, 1, 0, 0, 0xfe, 1, 0, b'z']
        );
    }

    #[test]
    fn ignores_unknown_broadcast_frames() {
        let mut vipc = vipc();
        let request = [0x00, 0x70, 0, 0, 4, 0, 0, 1, 0, 0];

        assert!(vipc.process_message(&request).unwrap().is_empty());
    }

    #[test]
    fn serializes_initial_message_drain_response() {
        let mut vipc = vipc();
        let request = [0x44, 0x74, 0x10, 0, 4, 0, 0, 1, 0, 0];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0x44, 0x74, 0x10, 0x80, 8, 0, 0, 1, 0, 0, 0xfd, 1, 0, b'z']
        );
    }

    #[test]
    fn serializes_sentry_variant_response() {
        let mut vipc = vipc();
        let request = [0xff, 0xff, 0xff, 0xff, 1, 0, 4];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0xff, 0xff, 0xff, 0xff, 4, 0, 3, 0x25, 1, b'3']
        );
    }

    #[test]
    fn serializes_sentry_add_user_response() {
        let mut vipc = vipc();
        let payload = [
            6, 7, 4, b'G', b'R', b'i', b'D', 8, 4, b'D', b'e', b'm', b'o', 9, 3, b'B', b'O', b'B',
            0x0a, 2, b'P', b'W', 0x1a, 2, 0, 0, 0x26, 4, 0, 4, 0, 0,
        ];
        let mut request = vec![0xff, 0xff, 0xff, 0xff];
        request.extend((payload.len() as u16).to_le_bytes());
        request.extend_from_slice(&payload);

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0xff, 0xff, 0xff, 0xff, 3, 0, 2, 0, 0]
        );
    }

    #[test]
    fn serializes_empty_mail_list_response() {
        let mut vipc = vipc();
        let request = [
            0x44, 0x74, 7, 0, 17, 0, 0, 5, 0, 0, 0xfd, 10, 0, b'S', 1, 0, 1, 0, 0, 0, 0, 0, 0,
        ];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [0x44, 0x74, 7, 0x80, 8, 0, 0, 5, 0, 0, 0xfd, 1, 0, b'z']
        );
    }
}
