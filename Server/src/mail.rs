const MORE: u8 = 1;
const MAX_REQUEST: usize = 64 * 1024;

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
            4 if pending.data.is_empty() => vec![(0, app_frame(0xfd, b"z"))],
            6 if valid_tagged_request(&pending.data) => vec![(0, app_frame(0xfd, b"T"))],
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
            8 if pending.data.is_empty() => vec![(0, app_frame(0xfd, b"z"))],
            0x0d if valid_i_request(&pending.data) => vec![(0, app_frame(0xfd, b"z"))],
            0x0e | 0x10 if pending.data.is_empty() => vec![(0, app_frame(0xfd, b"z"))],
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
        let [flags, connection_id, error_lo, error_hi, body @ ..] = data else {
            return None;
        };
        Some(Self {
            flags: *flags,
            connection_id: *connection_id,
            error: u16::from_le_bytes([*error_lo, *error_hi]),
            data: body,
        })
    }
}

fn valid_tagged_request(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    let mut offset = 0;
    while offset < data.len() {
        let Some(header) = data.get(offset..offset + 3) else {
            return false;
        };
        if header[0] != 0xfd {
            return false;
        }
        let length = u16::from_le_bytes([header[1], header[2]]) as usize;
        if length == 0 || data.get(offset + 3..offset + 3 + length).is_none() {
            return false;
        }
        offset += 3 + length;
    }
    true
}

enum SRequest {
    List,
    Detail([u8; 6]),
    ReadNew,
}

impl SRequest {
    fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 13
            || data[0] != 0xfd
            || data[3] != b'S'
            || u16::from_le_bytes([data[1], data[2]]) as usize != data.len() - 3
        {
            return None;
        }

        let requested_tags = data.get(13..)?;
        if !requested_tags.contains(&b'n') {
            return Some(Self::List);
        }

        if data[4..7] == [1, 0, 1] {
            Some(Self::ReadNew)
        } else {
            Some(Self::Detail(data.get(7..13)?.try_into().ok()?))
        }
    }
}

fn valid_i_request(data: &[u8]) -> bool {
    data.len() == 6 && data[..4] == [0xfd, 3, 0, b'I']
}

fn app_frame(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.push(marker);
    frame.extend((payload.len() as u16).to_le_bytes());
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
    #[allow(dead_code)]
    original: Vec<u8>,
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
            original: Vec::new(),
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
            original,
        })
    }
}

fn mail_id(value: u32) -> [u8; 6] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0]
}

fn tagged_value(data: &[u8], wanted: u8) -> Option<&[u8]> {
    let mut offset = 0;
    while offset + 3 <= data.len() {
        if data[offset] != 0xfd {
            offset += 1;
            continue;
        }
        let length = u16::from_le_bytes([data[offset + 1], data[offset + 2]]) as usize;
        if length == 0 {
            offset += 1;
            continue;
        }
        let end = offset.checked_add(3 + length)?;
        let record = match data.get(offset + 3..end) {
            Some(record) => record,
            None => {
                offset += 1;
                continue;
            }
        };
        if record[0] == wanted {
            return Some(&record[1..]);
        }
        offset = end;
    }
    None
}

fn outgoing_body(data: &[u8]) -> Option<&[u8]> {
    let terminator = [0xfd, 1, 0, b'z'];
    let mut offset = 0;
    while offset + 4 <= data.len() {
        if data[offset] != 0xfd {
            offset += 1;
            continue;
        }
        let length = u16::from_le_bytes([data[offset + 1], data[offset + 2]]) as usize;
        let end = offset.checked_add(3 + length)?;
        if length > 0
            && data.get(offset + 3) == Some(&b'n')
            && data.get(end..end + terminator.len()) == Some(terminator.as_slice())
        {
            return data.get(offset + 4..end);
        }
        offset += 1;
    }
    None
}

fn mail_header(message: &MailMessage) -> Vec<u8> {
    let mut value = Vec::with_capacity(12);
    value.push(b'b');
    value.extend(message.id);
    value.extend([0, 0, 0, 0, 1]);
    app_frame(0xfd, &value)
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
            data.extend(app_frame(0xfd, &sender));
            let mut subject = vec![b's'];
            subject.extend_from_slice(&message.subject);
            data.extend(app_frame(0xfd, &subject));
            data.extend(app_frame(0xfd, b"z"));

            let flags = if index == last { 0 } else { MORE };
            (flags, data)
        })
        .collect()
}

fn empty_mail_result() -> Vec<u8> {
    app_frame(0xfd, b"z")
}

fn mail_detail(message: &MailMessage) -> Vec<u8> {
    let mut data = mail_header(message);
    for (tag, value) in [
        (b't', message.recipient.as_slice()),
        (b'f', message.sender.as_slice()),
        (b'k', message.sender.as_slice()),
        (b's', message.subject.as_slice()),
        (b'n', message.body.as_slice()),
    ] {
        let mut record = vec![tag];
        record.extend_from_slice(value);
        data.extend(app_frame(0xfd, &record));
    }
    data.extend(app_frame(0xfd, b"z"));

    // After parsing `z`, GRiDMail directly drains the remaining response stream.
    data.extend_from_slice(&message.body);
    data
}

fn transport(flags: u8, connection_id: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + 4);
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
                payload: transport(0, 1, &app_frame(0xfd, b"z")),
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
        outgoing.extend(app_frame(0xfd, b"z"));

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
        outgoing.extend(app_frame(0xfd, b"z"));

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
    fn read_new_iterates_all_mail_then_terminates() {
        let mut mail = MailServer::new();
        let mut outgoing = Vec::new();
        outgoing.extend(app_frame(0xfd, b"tUser"));
        outgoing.extend(app_frame(0xfd, b"sSecond"));
        outgoing.extend(app_frame(0xfd, b"nSecond body"));
        outgoing.extend(app_frame(0xfd, b"z"));
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
    fn rejects_transport_error() {
        assert_eq!(MailServer::new().process(0, &[0, 5, 1, 0]), None);
    }
}
