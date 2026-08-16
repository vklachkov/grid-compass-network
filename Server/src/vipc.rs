use std::rc::Rc;

use log::{debug, warn};
use rusqlite::Connection;

use crate::{
    broadcast::BroadcastServer,
    db,
    gridlink::{
        FrameError,
        vipc::{IncomingMessage, MessageType, OutgoingMessage, OutgoingMessageBody},
    },
    mail::MailServer,
    sentry::SentryServer,
    vfs::{Vfs, VfsRequest},
};

const CLASS_VFS: MessageType = MessageType(83);
const CLASS_MAIL: MessageType = MessageType(0x7444);
const CLASS_BROADCAST: MessageType = MessageType(0x7000);
const CLASS_SENTRY: MessageType = MessageType(0xffff);

pub struct Vipc {
    vfs: Vfs,
    mail: MailServer,
    sentry: SentryServer,
    broadcast: BroadcastServer,
}

impl Vipc {
    pub fn new(conn: Rc<Connection>, actor: db::Account) -> Self {
        Self {
            vfs: Vfs::new(),
            mail: MailServer::new(),
            sentry: SentryServer::new(conn, actor),
            broadcast: BroadcastServer::new(),
        }
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
                if let Some(data) = self.vfs.take_finalized_mail()
                    && !self.mail.accept_outgoing(data)
                {
                    warn!(target: "vipc", "discarded malformed outgoing mail object");
                }
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
                .broadcast
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

        Vipc::new(conn, actor)
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

    fn tagged(tag: u8, value: &[u8]) -> Vec<u8> {
        let mut data = vec![0xfd];
        data.extend(((value.len() + 1) as u16).to_le_bytes());
        data.push(tag);
        data.extend_from_slice(value);
        data
    }

    fn vfs_message(note: u16, payload: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(CLASS_VFS.0.to_le_bytes());
        data.extend(note.to_le_bytes());
        data.extend((payload.len() as u16).to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn finalized_vfs_mail_appears_in_mail_list() {
        let mut vipc = vipc();
        let path = b"`vklachkov server:Mail`Mail`84/08/10 19:01:54.3~Mail~";
        let mut attach = vec![8, 0, 0, 0x7e, 0, 0, 4, 2];
        attach.extend([0; 17]);
        attach.push(path.len() as u8);
        attach.extend_from_slice(path);
        vipc.process_message(&vfs_message(1, &attach)).unwrap();

        vipc.process_message(&vfs_message(2, &[2, 0, 0, 0x7e, 1, 0, 1]))
            .unwrap();

        let mut outgoing = tagged(b't', b"User");
        outgoing.extend(tagged(b's', b"Sent through VFS"));
        outgoing.extend(tagged(b'n', b"Stored body"));
        outgoing.extend(tagged(b'z', b""));
        let mut write = vec![5, 0, 0, 0x7e, 1, 0];
        write.extend((outgoing.len() as u16).to_le_bytes());
        write.extend(outgoing);
        vipc.process_message(&vfs_message(3, &write)).unwrap();
        vipc.process_message(&vfs_message(4, &[9, 0, 0, 0x7e, 1, 0]))
            .unwrap();

        let request = [
            0x44, 0x74, 7, 0, 17, 0, 0, 5, 0, 0, 0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0,
        ];
        let responses = vipc.process_message(&request).unwrap();
        assert_eq!(responses.len(), 2);
        let first = responses[0].to_bytes();
        let second = responses[1].to_bytes();
        assert_eq!(first[6] & 1, 1);
        assert_eq!(second[6] & 1, 0);
        assert!(
            second
                .windows(b"Sent through VFS".len())
                .any(|bytes| bytes == b"Sent through VFS")
        );
    }

    #[test]
    fn serializes_demo_mail_list_response() {
        let mut vipc = vipc();
        let request = [
            0x44, 0x74, 7, 0, 17, 0, 0, 5, 0, 0, 0xfd, 10, 0, b'S', 1, 0, 1, 0, 0, 0, 0, 0, 0,
        ];

        let responses = vipc.process_message(&request).unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].to_bytes(),
            [
                0x44, 0x74, 7, 0x80, 56, 0, 0, 5, 0, 0, 0xfd, 12, 0, b'b', 1, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 1, 0xfd, 17, 0, b'k', b'G', b'R', b'i', b'D', b' ', b'M', b'a', b'i', b'l',
                b' ', b'S', b'e', b'r', b'v', b'e', b'r', 0xfd, 10, 0, b's', b'D', b'e', b'm',
                b'o', b' ', b'm', b'a', b'i', b'l', 0xfd, 1, 0, b'z',
            ]
        );
    }
}
