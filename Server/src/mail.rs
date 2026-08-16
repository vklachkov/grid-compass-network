use std::io;

use crate::{
    gridlink::{
        Tlv,
        utils::{CursorExt, ReadExt, u16_len},
    },
    protocol::app::{MORE, TAG_TERMINATOR, TRANSPORT_HEADER_LEN},
};

const RECORD_MARKER: u8 = 0xfd;
const TAG_BODY: u8 = b'n';
/// A three byte selector, a six byte mail id, then the tags the client wants back.
const TAG_SELECT: u8 = b'S';
const TAG_INITIALIZE: u8 = b'I';

/// Walk unread mail rather than fetch one item by id.
const READ_NEW_SELECTOR: [u8; 3] = [1, 0, 1];

const MAX_REQUEST: usize = 64 * 1024;
const MAX_TAG_VALUE: usize = u16::MAX as usize - 1;

pub struct MailServer {
    fragments: Vec<Pending>,
    messages: Vec<MailMessage>,
    next_mail_id: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MailResponse {
    pub note: u16,
    pub payload: Vec<u8>,
}

impl MailServer {
    pub fn new() -> Self {
        Self {
            fragments: vec![Pending::default(); 17],
            messages: vec![MailMessage::demo()],
            next_mail_id: 2,
        }
    }

    pub fn accept_outgoing(&mut self, data: Vec<u8>) -> bool {
        let Some(message) = MailMessage::from_outgoing(mail_id(self.next_mail_id), data) else {
            return false;
        };
        // TODO: fragment large Mail responses to the PDL payload limit instead of rejecting them.
        if !message.fits_vipc_payload() {
            return false;
        }
        self.next_mail_id = self.next_mail_id.wrapping_add(1).max(2);
        self.messages.push(message);
        true
    }

    pub fn process(&mut self, note: u16, payload: &[u8]) -> Option<Vec<MailResponse>> {
        let channel = note as usize;
        if channel >= self.fragments.len() {
            return None;
        }

        let fragment = match Fragment::parse(payload) {
            Some(fragment) if fragment.error == 0 => fragment,
            _ => {
                self.fragments[channel] = Pending::default();
                return None;
            }
        };
        let pending = &mut self.fragments[channel];
        if !pending.data.is_empty() && pending.connection_id != fragment.connection_id {
            *pending = Pending::default();
        }
        pending.connection_id = fragment.connection_id;
        if pending.data.len().saturating_add(fragment.data.len()) > MAX_REQUEST {
            *pending = Pending::default();
            return None;
        }
        pending.data.extend_from_slice(fragment.data);
        if fragment.flags & MORE != 0 {
            return None;
        }

        let pending = std::mem::take(pending);
        let responses = match note {
            0 if pending.data == [0xfe, 4, 0, b'a', 0x88, 0x2c, 1] => {
                vec![(0, app_frame(0xfe, b"z"))]
            }
            4 if pending.data.is_empty() => vec![(0, app_frame(RECORD_MARKER, &[TAG_TERMINATOR]))],
            6 if valid_tagged_request(&pending.data) => vec![(0, app_frame(RECORD_MARKER, b"T"))],
            7 => match SRequest::parse(&pending.data)? {
                SRequest::List => mailbox_list(&self.messages),
                SRequest::Detail(id) => vec![(
                    0,
                    self.messages
                        .iter()
                        .find(|message| message.id == id)
                        .map(mail_detail)
                        .unwrap_or_else(empty_mail_result),
                )],
                SRequest::ReadNew => vec![(
                    0,
                    self.messages
                        .iter_mut()
                        .find(|message| !message.read)
                        .map(|message| {
                            message.read = true;
                            mail_detail(message)
                        })
                        .unwrap_or_else(empty_mail_result),
                )],
            },
            8 if pending.data.is_empty() => vec![(0, app_frame(RECORD_MARKER, &[TAG_TERMINATOR]))],
            0x0d if valid_i_request(&pending.data) => {
                vec![(0, app_frame(RECORD_MARKER, &[TAG_TERMINATOR]))]
            }
            0x0e | 0x10 if pending.data.is_empty() => {
                vec![(0, app_frame(RECORD_MARKER, &[TAG_TERMINATOR]))]
            }
            _ => return None,
        };

        Some(
            responses
                .into_iter()
                .map(|(flags, data)| MailResponse {
                    note,
                    payload: transport(flags, pending.connection_id, &data),
                })
                .collect(),
        )
    }
}

#[derive(Clone, Default)]
struct Pending {
    connection_id: u8,
    data: Vec<u8>,
}

struct Fragment<'a> {
    flags: u8,
    connection_id: u8,
    error: u16,
    data: &'a [u8],
}

impl<'a> Fragment<'a> {
    fn parse(data: &'a [u8]) -> Option<Self> {
        let mut cursor = io::Cursor::new(data);

        Some(Self {
            flags: cursor.read_u8().ok()?,
            connection_id: cursor.read_u8().ok()?,
            error: cursor.read_u16().ok()?,
            data: cursor.read_remainder(),
        })
    }
}

fn valid_tagged_request(data: &[u8]) -> bool {
    records(data).all_records_valid()
}

/// Walks a GRiDMail application payload as `<0xfd><u16 length><tag + value>`
/// records.
fn records(data: &[u8]) -> Tlv<'_> {
    Tlv::marker_u16(data, RECORD_MARKER)
}

enum SRequest {
    List,
    Detail([u8; 6]),
    ReadNew,
}

impl SRequest {
    fn parse(data: &[u8]) -> Option<Self> {
        // The whole payload must be exactly one `S` record: a trailing second
        // record would mean the client asked for something this parse ignores.
        let value = match records(data).collect_all().ok()?.as_slice() {
            [entry] if entry.tag == TAG_SELECT => entry.value,
            _ => return None,
        };

        let mut cursor = io::Cursor::new(value);
        let selector: [u8; 3] = cursor.read_array().ok()?;
        let id: [u8; 6] = cursor.read_array().ok()?;
        let requested_tags = cursor.read_remainder();

        if !requested_tags.contains(&TAG_BODY) {
            return Some(Self::List);
        }

        if selector == READ_NEW_SELECTOR {
            Some(Self::ReadNew)
        } else {
            Some(Self::Detail(id))
        }
    }
}

fn valid_i_request(data: &[u8]) -> bool {
    matches!(
        records(data).collect_all().as_deref(),
        Ok([entry]) if entry.tag == TAG_INITIALIZE && entry.value.len() == 2
    )
}

fn app_frame(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.push(marker);
    // Every caller builds the payload from fields already checked against
    // `MAX_TAG_VALUE`, so an oversized one is a bug here rather than something
    // a client can trigger.
    frame.extend(
        u16_len(payload.len(), "GRiDMail record")
            .unwrap()
            .to_le_bytes(),
    );
    frame.extend_from_slice(payload);
    frame
}

const DEMO_MAIL_ID: [u8; 6] = [1, 0, 0, 0, 0, 0];
const DEMO_MAIL_BODY: &[u8] =
    b"Welcome to GRiD Mail. This is a demo message from the local server.";
const SERVER_NAME: &[u8] = b"GRiD Mail Server";

struct MailMessage {
    id: [u8; 6],
    recipient: Vec<u8>,
    sender: Vec<u8>,
    subject: Vec<u8>,
    body: Vec<u8>,
    read: bool,
}

impl MailMessage {
    fn demo() -> Self {
        Self {
            id: DEMO_MAIL_ID,
            recipient: b"Demo User".to_vec(),
            sender: SERVER_NAME.to_vec(),
            subject: b"Demo mail".to_vec(),
            body: DEMO_MAIL_BODY.to_vec(),
            read: false,
        }
    }

    fn from_outgoing(id: [u8; 6], original: Vec<u8>) -> Option<Self> {
        let recipient = tagged_value(&original, b't')?.to_vec();
        let subject = tagged_value(&original, b's')?.to_vec();
        let body = outgoing_body(&original)?.to_vec();
        Some(Self {
            id,
            recipient,
            sender: b"User".to_vec(),
            subject,
            body,
            read: false,
        })
    }

    fn fits_vipc_payload(&self) -> bool {
        self.recipient.len() <= MAX_TAG_VALUE
            && self.sender.len() <= MAX_TAG_VALUE
            && self.subject.len() <= MAX_TAG_VALUE
            && self.body.len() <= MAX_TAG_VALUE
            && mail_detail_len(self)
                .checked_add(TRANSPORT_HEADER_LEN)
                .is_some_and(|length| length <= u16::MAX as usize)
    }
}

fn mail_id(value: u32) -> [u8; 6] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0]
}

fn tagged_value(data: &[u8], wanted: u8) -> Option<&[u8]> {
    records(data).find_tag(wanted)
}

/// The body of an outgoing mail object: the `n` record that is immediately
/// followed by the `z` terminator, which is where GRiDMail stops writing.
/// A rewritten mail object can keep a tail of the longer version it replaced,
/// so the records before it are read rather than the whole buffer rejected.
fn outgoing_body(data: &[u8]) -> Option<&[u8]> {
    records(data)
        .well_formed_prefix()
        .windows(2)
        .find(|pair| {
            pair[0].tag == TAG_BODY && pair[1].tag == TAG_TERMINATOR && pair[1].value.is_empty()
        })
        .map(|pair| pair[0].value)
}

fn mail_header(message: &MailMessage) -> Vec<u8> {
    let mut value = Vec::with_capacity(12);
    value.push(b'b');
    value.extend(message.id);
    value.extend([0, 0, 0, 0, 1]);
    app_frame(RECORD_MARKER, &value)
}

fn mailbox_list(messages: &[MailMessage]) -> Vec<(u8, Vec<u8>)> {
    if messages.is_empty() {
        return vec![(0, empty_mail_result())];
    }

    let last = messages.len() - 1;
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let mut data = mail_header(message);
            let mut sender = vec![b'k'];
            sender.extend_from_slice(&message.sender);
            data.extend(app_frame(RECORD_MARKER, &sender));
            let mut subject = vec![b's'];
            subject.extend_from_slice(&message.subject);
            data.extend(app_frame(RECORD_MARKER, &subject));
            data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

            let flags = if index == last { 0 } else { MORE };
            (flags, data)
        })
        .collect()
}

fn empty_mail_result() -> Vec<u8> {
    app_frame(RECORD_MARKER, &[TAG_TERMINATOR])
}

fn mail_detail_len(message: &MailMessage) -> usize {
    let tagged_records = [
        message.recipient.len(),
        message.sender.len(),
        message.sender.len(),
        message.subject.len(),
        message.body.len(),
    ];

    // `b` header, five tagged fields, `z`, then the raw body drained by GRiDMail.
    15 + tagged_records
        .iter()
        .map(|length| 4 + length)
        .sum::<usize>()
        + 4
        + message.body.len()
}

fn mail_detail(message: &MailMessage) -> Vec<u8> {
    debug_assert!(message.fits_vipc_payload());
    let mut data = Vec::with_capacity(mail_detail_len(message));
    data.extend(mail_header(message));
    for (tag, value) in [
        (b't', message.recipient.as_slice()),
        (b'f', message.sender.as_slice()),
        (b'k', message.sender.as_slice()),
        (b's', message.subject.as_slice()),
        (b'n', message.body.as_slice()),
    ] {
        let mut record = vec![tag];
        record.extend_from_slice(value);
        data.extend(app_frame(RECORD_MARKER, &record));
    }
    data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

    // After parsing `z`, GRiDMail directly drains the remaining response stream.
    data.extend_from_slice(&message.body);
    data
}

fn transport(flags: u8, connection_id: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + TRANSPORT_HEADER_LEN);
    payload.extend([flags, connection_id, 0, 0]);
    payload.extend_from_slice(data);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_mail_list() -> Vec<(u8, Vec<u8>)> {
        mailbox_list(&[MailMessage::demo()])
    }

    fn demo_mail_detail() -> Vec<u8> {
        mail_detail(&MailMessage::demo())
    }

    fn request(more: bool, data: &[u8]) -> Vec<u8> {
        let mut payload = vec![more as u8, 5, 0, 0];
        payload.extend_from_slice(data);
        payload
    }

    fn response(note: u16, marker: u8, data: &[u8]) -> Vec<MailResponse> {
        vec![MailResponse {
            note,
            payload: transport(0, 5, &app_frame(marker, data)),
        }]
    }

    #[test]
    fn initializes_session_from_observed_payload() {
        let mut mail = MailServer::new();
        assert_eq!(
            mail.process(0, &[0, 5, 0, 0, 0xfe, 4, 0, b'a', 0x88, 0x2c, 1]),
            Some(response(0, 0xfe, b"z"))
        );
    }

    #[test]
    fn assembles_transport_fragments() {
        let mut mail = MailServer::new();
        assert_eq!(
            mail.process(6, &request(true, &[0xfd, 2, 0, b't', b'x'])),
            None
        );
        assert_eq!(
            mail.process(6, &request(false, &[])),
            Some(response(6, 0xfd, b"T"))
        );
    }

    #[test]
    fn drops_interleaved_connection() {
        let mut mail = MailServer::new();
        assert_eq!(
            mail.process(6, &request(true, &[0xfd, 2, 0, b't', b'x'])),
            None
        );
        assert_eq!(mail.process(6, &[0, 6, 0, 0]), None);
    }

    #[test]
    fn rejects_malformed_commands() {
        let mut mail = MailServer::new();
        assert_eq!(mail.process(7, &request(false, &[])), None);
        assert_eq!(mail.process(8, &request(false, b"x")), None);
        assert_eq!(
            mail.process(0x0d, &request(false, &[0xfd, 3, 0, b'I'])),
            None
        );
    }

    #[test]
    fn completes_initial_server_message_drain() {
        let mut mail = MailServer::new();
        assert_eq!(
            mail.process(0x10, &[0, 1, 0, 0]),
            Some(vec![MailResponse {
                note: 0x10,
                payload: transport(0, 1, &app_frame(RECORD_MARKER, &[TAG_TERMINATOR])),
            }])
        );
    }

    #[test]
    fn returns_demo_mail_list() {
        let mut mail = MailServer::new();
        let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
        let [(flags, data)] = demo_mail_list().try_into().unwrap();
        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(flags, 5, &data),
            }])
        );
    }

    fn assert_detail_response(query: &[u8]) {
        let responses = MailServer::new()
            .process(7, &request(false, query))
            .unwrap();
        let [response] = responses.as_slice() else {
            panic!("detail query returned more than one response");
        };
        assert_eq!(response.note, 7);
        assert_eq!(response.payload, transport(0, 5, &demo_mail_detail()));

        let tagged_end = response
            .payload
            .windows(4)
            .position(|bytes| bytes == [0xfd, 1, 0, b'z'])
            .unwrap()
            + 4;
        assert_eq!(&response.payload[tagged_end..], DEMO_MAIL_BODY);
        assert_eq!(response.payload[0] & MORE, 0);
    }

    #[test]
    fn returns_complete_demo_mail_for_detail_query() {
        let mut query = vec![0xfd, 27, 0, b'S', 7, 0, 0];
        query.extend(DEMO_MAIL_ID);
        query.extend(b"kuvspoftcandgbzxy");
        assert_detail_response(&query);
    }

    fn read_new_query() -> Vec<u8> {
        let mut query = vec![0xfd, 27, 0, b'S', 1, 0, 1];
        query.extend([0; 6]);
        query.extend(b"kuvspoftcandgbzxy");
        query
    }

    #[test]
    fn returns_demo_mail_once_for_read_new_query() {
        let mut mail = MailServer::new();
        let query = read_new_query();

        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &demo_mail_detail()),
            }])
        );
        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &empty_mail_result()),
            }])
        );
    }

    #[test]
    fn read_new_state_does_not_hide_mail_from_normal_detail_query() {
        let mut mail = MailServer::new();
        let read_new = read_new_query();
        mail.process(7, &request(false, &read_new)).unwrap();

        let mut detail = vec![0xfd, 27, 0, b'S', 7, 0, 0];
        detail.extend(DEMO_MAIL_ID);
        detail.extend(b"kuvspoftcandgbzxy");
        assert_eq!(
            mail.process(7, &request(false, &detail)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &demo_mail_detail()),
            }])
        );
    }

    #[test]
    fn returns_each_mailbox_item_in_its_own_response() {
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(0xfd, b"tUser"));
        outgoing.extend(app_frame(0xfd, b"sSecond"));
        outgoing.extend(app_frame(0xfd, b"nSecond body"));
        outgoing.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

        let mut mail = MailServer::new();
        assert!(mail.accept_outgoing(outgoing));

        let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
        let responses = mail.process(7, &request(false, &query)).unwrap();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].payload[0] & MORE, MORE);
        assert_eq!(responses[1].payload[0] & MORE, 0);

        for response in &responses {
            let mut offset = 4;
            let mut tags = Vec::new();
            while offset < response.payload.len() {
                assert_eq!(response.payload[offset], 0xfd);
                let length = u16::from_le_bytes([
                    response.payload[offset + 1],
                    response.payload[offset + 2],
                ]) as usize;
                tags.push(response.payload[offset + 3]);
                offset += 3 + length;
            }
            assert_eq!(tags, [b'b', b'k', b's', b'z']);
        }

        assert_ne!(&responses[0].payload[8..14], &responses[1].payload[8..14]);
    }

    #[test]
    fn accepts_outgoing_mail_and_selects_it_by_id() {
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(0xfd, b"tUser"));
        outgoing.extend(app_frame(0xfd, b"sSent subject"));
        outgoing.extend(app_frame(0xfd, b"DDemo attachment nnoise"));
        outgoing.extend(app_frame(0xfd, b"nSent body"));
        outgoing.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

        let mut mail = MailServer::new();
        assert!(mail.accept_outgoing(outgoing));
        assert_eq!(mail.messages.len(), 2);
        assert_eq!(mail.messages[1].id, [2, 0, 0, 0, 0, 0]);
        assert_eq!(mail.messages[1].subject, b"Sent subject");
        assert_eq!(mail.messages[1].body, b"Sent body");

        let mut detail = vec![0xfd, 27, 0, b'S', 7, 0, 0];
        detail.extend(mail.messages[1].id);
        detail.extend(b"kuvspoftcandgbzxy");
        let responses = mail.process(7, &request(false, &detail)).unwrap();
        let [response] = responses.as_slice() else {
            panic!("detail query returned more than one response");
        };
        assert!(
            response
                .payload
                .windows(b"Sent subject".len())
                .any(|bytes| bytes == b"Sent subject")
        );
        assert!(response.payload.ends_with(b"Sent body"));
    }

    #[test]
    fn accepts_outgoing_mail_with_a_stale_tail() {
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(RECORD_MARKER, b"tUser"));
        outgoing.extend(app_frame(RECORD_MARKER, b"sShort"));
        outgoing.extend(app_frame(RECORD_MARKER, b"nShort body"));
        outgoing.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));
        outgoing.extend_from_slice(b" leftovers of a longer object");

        let mut mail = MailServer::new();
        assert!(mail.accept_outgoing(outgoing));
        assert_eq!(mail.messages[1].subject, b"Short");
        assert_eq!(mail.messages[1].body, b"Short body");
    }

    #[test]
    fn read_new_iterates_all_mail_then_terminates() {
        let mut mail = MailServer::new();
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(0xfd, b"tUser"));
        outgoing.extend(app_frame(0xfd, b"sSecond"));
        outgoing.extend(app_frame(0xfd, b"nSecond body"));
        outgoing.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));
        assert!(mail.accept_outgoing(outgoing));

        let query = read_new_query();
        let first = mail.process(7, &request(false, &query)).unwrap();
        let second = mail.process(7, &request(false, &query)).unwrap();
        let third = mail.process(7, &request(false, &query)).unwrap();
        assert!(first[0].payload.ends_with(DEMO_MAIL_BODY));
        assert!(second[0].payload.ends_with(b"Second body"));
        assert_eq!(third[0].payload, transport(0, 5, &empty_mail_result()));
    }

    #[test]
    fn rejects_mail_that_cannot_fit_in_one_vipc_response() {
        let body = vec![b'x'; 40_000];
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(0xfd, b"tUser"));
        outgoing.extend(app_frame(0xfd, b"sLarge"));
        let mut body_record = vec![b'n'];
        body_record.extend(body);
        outgoing.extend(app_frame(0xfd, &body_record));
        outgoing.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

        let mut mail = MailServer::new();
        assert!(!mail.accept_outgoing(outgoing));
        assert_eq!(mail.messages.len(), 1);
    }

    #[test]
    fn rejects_transport_error() {
        assert_eq!(MailServer::new().process(0, &[0, 5, 1, 0]), None);
    }
}
