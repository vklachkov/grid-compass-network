use std::{io, rc::Rc};

use log::{debug, error, warn};
use rusqlite::Connection;

use super::protocol::app::{MORE, TAG_TERMINATOR, TRANSPORT_HEADER_LEN};
use crate::{
    db::mailbox::{self, Message, NewMessage},
    shared::{
        Tlv,
        io::{CursorExt, ReadExt, u16_len},
    },
};

const RECORD_MARKER: u8 = 0xfd;
const SESSION_MARKER: u8 = 0xfe;

const TAG_HEADER: u8 = b'b';
const TAG_RECIPIENT: u8 = b't';
const TAG_SENDER: u8 = b'f';
const TAG_DISPLAY_SENDER: u8 = b'k';
const TAG_SUBJECT: u8 = b's';
const TAG_BODY: u8 = b'n';
const TAG_SELECT: u8 = b'S';
const TAG_INITIALIZE: u8 = b'I';
const TAG_REQUEST_ACCEPTED: u8 = b'T';

const CHANNEL_SESSION: u16 = 0;
const CHANNEL_CLOSE: u16 = 4;
const CHANNEL_TAGGED_REQUEST: u16 = 6;
const CHANNEL_SELECT: u16 = 7;
const CHANNEL_FLUSH: u16 = 8;
const CHANNEL_INITIALIZE: u16 = 0x0d;
const CHANNEL_FINALIZE: u16 = 0x0e;
const CHANNEL_DRAIN: u16 = 0x10;
const CHANNEL_COUNT: usize = CHANNEL_DRAIN as usize + 1;

const SESSION_COMMAND: u8 = b'a';

const MAIL_SERVICE_ID: u16 = 11_400;
const MAIL_PROTOCOL_VERSION: u8 = 1;

const SESSION_INITIALIZE_RESPONSE: &[u8] = b"z";

const MAIL_ID_LEN: usize = 6;
const MAIL_HEADER_RESERVED_LEN: usize = 4;
const MAIL_HEADER_VALUE_LEN: usize =
    TAG_LEN + MAIL_ID_LEN + MAIL_HEADER_RESERVED_LEN + MailStatus::WIRE_LEN;
const RECORD_HEADER_LEN: usize = 3;
const TAG_LEN: usize = 1;
const MAX_REQUEST: usize = 64 * 1024;
const MAX_TAG_VALUE: usize = u16::MAX as usize - TAG_LEN;
const INITIALIZE_VALUE_LEN: usize = std::mem::size_of::<u16>();
// The observed 1/7 values match a bit mask over the confirmed 0/1/2 mail statuses.
const NEW_MAIL_FILTER: u8 = 1 << MailStatus::New as u8;
const ALL_MAIL_FILTER: u8 =
    NEW_MAIL_FILTER | 1 << MailStatus::Read as u8 | 1 << MailStatus::Sent as u8;
// The third S control byte is 1 in every observed read-new request.
const READ_NEW_OPERATION: u8 = 1;
// The inner Mail transport prefix carries a little-endian error code.
const TRANSPORT_SUCCESS: u16 = 0;

pub struct MailServer {
    fragments: Vec<Pending>,
    conn: Rc<Connection>,
    /// The mailbox every request in this session reads and writes, and the
    /// sender of everything it writes: mail is stored per account, so a session
    /// can only ever reach the one it signed on to.
    owner_id: i64,
    /// The name the owner appears under in the mail it sends. It is held here
    /// rather than read back per message because the size of a response has to
    /// be known before the message is stored.
    owner_name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MailResponse {
    pub note: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
struct ChannelResponse {
    flags: u8,
    data: Vec<u8>,
}

impl ChannelResponse {
    fn final_message(data: Vec<u8>) -> Self {
        Self { flags: 0, data }
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

struct SessionInitialize;

enum MailStatusFilter {
    NewOnly,
    All,
    Other,
}

enum MailOrder {
    OldestFirst,
    NewestFirst,
    Other,
}

struct SelectControl {
    status_filter: MailStatusFilter,
    order: MailOrder,
    operation: u8,
}

enum SRequest {
    List,
    Detail([u8; MAIL_ID_LEN]),
    ReadNew,
}

struct Outgoing<'a> {
    recipient: &'a str,
    subject: &'a str,
    body: &'a str,
}

#[derive(Clone, Copy)]
enum MailStatus {
    New = 0,
    Read = 1,
    Sent = 2,
}

struct MailHeader {
    id: [u8; MAIL_ID_LEN],
    status: MailStatus,
}

impl MailServer {
    pub fn new(conn: Rc<Connection>, owner_id: i64, owner_name: String) -> Self {
        Self {
            fragments: vec![Pending::default(); CHANNEL_COUNT],
            conn,
            owner_id,
            owner_name,
        }
    }

    pub fn accept_outgoing(&mut self, data: Vec<u8>) -> bool {
        let Some(outgoing) = Outgoing::parse(&data) else {
            return false;
        };

        // TODO: fragment large Mail responses to the PDL payload limit instead of rejecting them.
        if !outgoing.fits_vipc_payload(&self.owner_name) {
            warn!(target: "mail", "refused a message too large for one VIPC payload");
            return false;
        }

        let recipient_id = match mailbox::find_recipient(
            &self.conn,
            self.owner_id,
            outgoing.recipient,
        ) {
            Ok(Some(id)) => id,
            Ok(None) => {
                warn!(target: "mail", "refused a message to the unknown recipient {}", outgoing.recipient);
                return false;
            }
            Err(err) => {
                error!(target: "mail", "failed to look up a recipient: {err}");
                return false;
            }
        };

        match mailbox::insert(
            &self.conn,
            &outgoing.as_new_message(self.owner_id, recipient_id),
        ) {
            Ok(mail_id) => {
                debug!(target: "mail", "stored an outgoing message as mail {mail_id}");
                true
            }
            Err(err) => {
                error!(target: "mail", "failed to store an outgoing message: {err}");
                false
            }
        }
    }

    /// A failed read is answered with nothing at all rather than an empty
    /// mailbox: the client would take the latter for a definite answer, and a
    /// database that is momentarily unreadable has said nothing definite.
    fn select(&mut self, request: SRequest) -> Option<Vec<ChannelResponse>> {
        let responses = match request {
            SRequest::List => mailbox_list(&self.read(mailbox::list)?),
            SRequest::Detail(id) => {
                let message = match mail_id_value(id) {
                    Some(id) => self.read(|conn, owner| mailbox::find(conn, owner, id))?,
                    None => None,
                };
                vec![ChannelResponse::final_message(
                    message.as_ref().map_or_else(empty_mail_result, mail_detail),
                )]
            }
            SRequest::ReadNew => {
                let message = self.read(mailbox::first_unread)?;
                if let Some(message) = &message
                    && let Err(err) = mailbox::mark_read(&self.conn, message.id)
                {
                    error!(target: "mail", "failed to mark mail {} read: {err}", message.mail_id);
                    return None;
                }
                vec![ChannelResponse::final_message(
                    message.as_ref().map_or_else(empty_mail_result, mail_detail),
                )]
            }
        };

        Some(responses)
    }

    fn read<T>(&self, query: impl FnOnce(&Connection, i64) -> rusqlite::Result<T>) -> Option<T> {
        match query(&self.conn, self.owner_id) {
            Ok(value) => Some(value),
            Err(err) => {
                error!(target: "mail", "failed to read the mailbox: {err}");
                None
            }
        }
    }

    pub fn process(&mut self, note: u16, payload: &[u8]) -> Option<Vec<MailResponse>> {
        let channel = note as usize;
        if channel >= self.fragments.len() {
            warn!(target: "mail", "ignored a request on the unknown channel {note}");
            return None;
        }

        let fragment = match Fragment::parse(payload) {
            Some(fragment) if fragment.error == 0 => fragment,
            _ => {
                warn!(target: "mail", "dropped a malformed fragment on channel {note}");
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
            warn!(target: "mail", "dropped a request over {MAX_REQUEST} bytes on channel {note}");
            *pending = Pending::default();
            return None;
        }
        pending.data.extend_from_slice(fragment.data);
        if fragment.flags & MORE != 0 {
            return None;
        }

        let pending = std::mem::take(pending);
        let responses = match note {
            CHANNEL_SESSION if SessionInitialize::parse(&pending.data).is_some() => {
                vec![ChannelResponse::final_message(app_frame(
                    SESSION_MARKER,
                    SESSION_INITIALIZE_RESPONSE,
                ))]
            }
            CHANNEL_CLOSE if pending.data.is_empty() => {
                vec![ChannelResponse::final_message(empty_mail_result())]
            }
            CHANNEL_TAGGED_REQUEST if valid_tagged_request(&pending.data) => {
                vec![ChannelResponse::final_message(app_frame(
                    RECORD_MARKER,
                    &[TAG_REQUEST_ACCEPTED],
                ))]
            }
            CHANNEL_SELECT => self.select(SRequest::parse(&pending.data)?)?,
            CHANNEL_FLUSH if pending.data.is_empty() => {
                vec![ChannelResponse::final_message(empty_mail_result())]
            }
            CHANNEL_INITIALIZE if valid_i_request(&pending.data) => {
                vec![ChannelResponse::final_message(empty_mail_result())]
            }
            CHANNEL_FINALIZE | CHANNEL_DRAIN if pending.data.is_empty() => {
                vec![ChannelResponse::final_message(empty_mail_result())]
            }
            _ => {
                warn!(target: "mail", "ignored an unsupported request on channel {note}");
                return None;
            }
        };

        debug!(target: "mail", "answered the request on channel {note}");

        Some(
            responses
                .into_iter()
                .map(|response| MailResponse {
                    note,
                    payload: transport(response.flags, pending.connection_id, &response.data),
                })
                .collect(),
        )
    }
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

impl SessionInitialize {
    fn parse(data: &[u8]) -> Option<Self> {
        let value = single_record(data, SESSION_MARKER, SESSION_COMMAND)?;
        let mut cursor = io::Cursor::new(value);
        let service_id = cursor.read_u16().ok()?;
        let protocol_version = cursor.read_u8().ok()?;

        (service_id == MAIL_SERVICE_ID
            && protocol_version == MAIL_PROTOCOL_VERSION
            && cursor.read_remainder().is_empty())
        .then_some(Self)
    }
}

fn valid_tagged_request(data: &[u8]) -> bool {
    Tlv::marker_u16(data, RECORD_MARKER).all_records_valid()
}

impl SelectControl {
    fn parse([status_filter, order, operation]: [u8; 3]) -> Self {
        Self {
            status_filter: match status_filter {
                NEW_MAIL_FILTER => MailStatusFilter::NewOnly,
                ALL_MAIL_FILTER => MailStatusFilter::All,
                _ => MailStatusFilter::Other,
            },
            // These values are protocol literals emitted by Mail_GetQueryOrder.
            order: match order {
                0 => MailOrder::OldestFirst,
                1 => MailOrder::NewestFirst,
                _ => MailOrder::Other,
            },
            operation,
        }
    }

    fn reads_new_mail(&self) -> bool {
        matches!(self.status_filter, MailStatusFilter::NewOnly)
            && matches!(self.order, MailOrder::OldestFirst)
            && self.operation == READ_NEW_OPERATION
    }
}

impl SRequest {
    fn parse(data: &[u8]) -> Option<Self> {
        // The whole payload must be exactly one `S` record: a trailing second
        // record would mean the client asked for something this parse ignores.
        let value = single_record(data, RECORD_MARKER, TAG_SELECT)?;

        let mut cursor = io::Cursor::new(value);
        let control = SelectControl::parse(cursor.read_array().ok()?);
        let id: [u8; MAIL_ID_LEN] = cursor.read_array().ok()?;
        let requested_tags = cursor.read_remainder();

        if !requested_tags.contains(&TAG_BODY) {
            return Some(Self::List);
        }

        if control.reads_new_mail() {
            Some(Self::ReadNew)
        } else {
            Some(Self::Detail(id))
        }
    }
}

fn valid_i_request(data: &[u8]) -> bool {
    single_record(data, RECORD_MARKER, TAG_INITIALIZE)
        .is_some_and(|value| value.len() == INITIALIZE_VALUE_LEN)
}

fn single_record(data: &[u8], marker: u8, tag: u8) -> Option<&[u8]> {
    match Tlv::marker_u16(data, marker).collect_all().ok()?.as_slice() {
        [entry] if entry.tag == tag => Some(entry.value),
        _ => None,
    }
}

fn app_frame(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + RECORD_HEADER_LEN);
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

impl<'a> Outgoing<'a> {
    fn parse(original: &'a [u8]) -> Option<Self> {
        Some(Self {
            recipient: text(tagged_value(original, TAG_RECIPIENT)?)?,
            subject: text(tagged_value(original, TAG_SUBJECT)?)?,
            body: text(outgoing_body(original)?)?,
        })
    }

    fn as_new_message(&self, sender_id: i64, recipient_id: i64) -> NewMessage<'_> {
        NewMessage {
            sender_id,
            recipient_id,
            subject: self.subject,
            body: self.body,
            // TODO: store the attachment the `a` and `g` tags describe once
            // their grammar is recovered.
            attachment_path: None,
        }
    }

    fn fits_vipc_payload(&self, sender: &str) -> bool {
        let fields = [self.recipient, sender, sender, self.subject, self.body];
        let record_overhead = fields.len() * (RECORD_HEADER_LEN + TAG_LEN);
        let fixed_length = MAIL_HEADER_VALUE_LEN
            + RECORD_HEADER_LEN
            + RECORD_HEADER_LEN
            + TAG_LEN
            + TRANSPORT_HEADER_LEN;

        fields.iter().all(|field| field.len() <= MAX_TAG_VALUE)
            && fields
                .iter()
                .try_fold(
                    fixed_length + record_overhead + self.body.len(),
                    |total, field| total.checked_add(field.len()),
                )
                .is_some_and(|length| length <= u16::MAX as usize)
    }
}

/// GRiD text is ASCII, which every byte the client sends should already satisfy;
/// anything above it is not text this server can store and the message is
/// refused rather than mangled.
fn text(value: &[u8]) -> Option<&str> {
    value.is_ascii().then(|| str::from_utf8(value).ok())?
}

/// The wire form of a mail id is six bytes of which the client only ever fills
/// the lower four, so a query naming the upper two addresses no stored message.
fn mail_id_bytes(value: u32) -> [u8; MAIL_ID_LEN] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0]
}

fn mail_id_value(id: [u8; MAIL_ID_LEN]) -> Option<u32> {
    (id[4] == 0 && id[5] == 0).then(|| u32::from_le_bytes([id[0], id[1], id[2], id[3]]))
}

fn tagged_value(data: &[u8], wanted: u8) -> Option<&[u8]> {
    Tlv::marker_u16(data, RECORD_MARKER).find_tag(wanted)
}

/// The body of an outgoing mail object: the `n` record that is immediately
/// followed by the `z` terminator, which is where GRiDMail stops writing.
/// A rewritten mail object can keep a tail of the longer version it replaced,
/// so the records before it are read rather than the whole buffer rejected.
fn outgoing_body(data: &[u8]) -> Option<&[u8]> {
    Tlv::marker_u16(data, RECORD_MARKER)
        .well_formed_prefix()
        .windows(2)
        .find(|pair| {
            pair[0].tag == TAG_BODY && pair[1].tag == TAG_TERMINATOR && pair[1].value.is_empty()
        })
        .map(|pair| pair[0].value)
}

impl MailStatus {
    const WIRE_LEN: usize = std::mem::size_of::<u8>();
}

impl MailHeader {
    fn encode(&self) -> Vec<u8> {
        let mut value = Vec::with_capacity(MAIL_HEADER_VALUE_LEN);
        value.push(TAG_HEADER);
        value.extend(self.id);
        value.extend([0; MAIL_HEADER_RESERVED_LEN]);
        value.push(self.status as u8);
        app_frame(RECORD_MARKER, &value)
    }
}

fn mail_header(message: &Message) -> Vec<u8> {
    MailHeader {
        id: mail_id_bytes(message.mail_id),
        status: MailStatus::Read,
    }
    .encode()
}

fn mailbox_list(messages: &[Message]) -> Vec<ChannelResponse> {
    if messages.is_empty() {
        return vec![ChannelResponse::final_message(empty_mail_result())];
    }

    let last = messages.len() - 1;
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let mut data = mail_header(message);
            data.extend(tagged_record(TAG_DISPLAY_SENDER, &message.sender.name));
            data.extend(tagged_record(TAG_SUBJECT, &message.subject));
            data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

            let flags = if index == last { 0 } else { MORE };
            ChannelResponse { flags, data }
        })
        .collect()
}

fn empty_mail_result() -> Vec<u8> {
    app_frame(RECORD_MARKER, &[TAG_TERMINATOR])
}

fn tagged_record(tag: u8, value: &str) -> Vec<u8> {
    let mut record = vec![tag];
    record.extend_from_slice(value.as_bytes());
    app_frame(RECORD_MARKER, &record)
}

fn mail_detail(message: &Message) -> Vec<u8> {
    let mut data = Vec::with_capacity(64);
    data.extend(mail_header(message));
    for (tag, value) in [
        (TAG_RECIPIENT, &message.recipient.name),
        (TAG_SENDER, &message.sender.name),
        (TAG_DISPLAY_SENDER, &message.sender.name),
        (TAG_SUBJECT, &message.subject),
        (TAG_BODY, &message.body),
    ] {
        data.extend(tagged_record(tag, value));
    }
    data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

    // After parsing `z`, GRiDMail directly drains the remaining response stream.
    data.extend_from_slice(message.body.as_bytes());
    data
}

fn transport(flags: u8, connection_id: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + TRANSPORT_HEADER_LEN);
    payload.push(flags);
    payload.push(connection_id);
    payload.extend(TRANSPORT_SUCCESS.to_le_bytes());
    payload.extend_from_slice(data);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db;

    const STORED_BODY: &str = "Stored body";

    fn mail_server() -> MailServer {
        let conn = Rc::new(db::open_in_memory());
        let owner = db::find_user(&conn, "GRiD", "Demo", "GUEST")
            .expect("read the demo directory")
            .expect("GUEST should exist");

        MailServer::new(conn, owner.id, owner.user)
    }

    /// Every test that reads a mailbox has to fill it first: a fresh database
    /// holds no mail at all.
    fn with_one_message() -> MailServer {
        let mut mail = mail_server();
        assert!(mail.accept_outgoing(outgoing("Stored subject", STORED_BODY)));
        mail
    }

    /// GUEST addresses itself: a recipient has to be an account of the sender's
    /// own group, and the mailbox the tests then read is the sender's.
    fn outgoing(subject: &str, body: &str) -> Vec<u8> {
        let mut data = app_frame(RECORD_MARKER, b"tGUEST");
        data.extend(tagged_record(b's', subject));
        data.extend(tagged_record(b'n', body));
        data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));
        data
    }

    fn stored_message(mail: &MailServer, mail_id: u32) -> Message {
        mailbox::find(&mail.conn, mail.owner_id, mail_id)
            .expect("read the mailbox")
            .expect("the message should be stored")
    }

    fn detail_query(mail_id: u32) -> Vec<u8> {
        let mut query = vec![0xfd, 27, 0, b'S', 7, 0, 0];
        query.extend(mail_id_bytes(mail_id));
        query.extend(b"kuvspoftcandgbzxy");
        query
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
        let mut mail = mail_server();
        assert_eq!(
            mail.process(0, &[0, 5, 0, 0, 0xfe, 4, 0, b'a', 0x88, 0x2c, 1]),
            Some(response(0, 0xfe, b"z"))
        );
    }

    #[test]
    fn assembles_transport_fragments() {
        let mut mail = mail_server();
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
        let mut mail = mail_server();
        assert_eq!(
            mail.process(6, &request(true, &[0xfd, 2, 0, b't', b'x'])),
            None
        );
        assert_eq!(mail.process(6, &[0, 6, 0, 0]), None);
    }

    #[test]
    fn rejects_malformed_commands() {
        let mut mail = mail_server();
        assert_eq!(mail.process(7, &request(false, &[])), None);
        assert_eq!(mail.process(8, &request(false, b"x")), None);
        assert_eq!(
            mail.process(0x0d, &request(false, &[0xfd, 3, 0, b'I'])),
            None
        );
    }

    #[test]
    fn completes_initial_server_message_drain() {
        let mut mail = mail_server();
        assert_eq!(
            mail.process(0x10, &[0, 1, 0, 0]),
            Some(vec![MailResponse {
                note: 0x10,
                payload: transport(0, 1, &app_frame(RECORD_MARKER, &[TAG_TERMINATOR])),
            }])
        );
    }

    #[test]
    fn an_empty_mailbox_lists_nothing() {
        let mut mail = mail_server();
        let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &empty_mail_result()),
            }])
        );
    }

    #[test]
    fn returns_the_stored_mail_list() {
        let mut mail = with_one_message();
        let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
        let [response] = mailbox_list(&[stored_message(&mail, 1)])
            .try_into()
            .unwrap();
        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(response.flags, 5, &response.data),
            }])
        );
    }

    #[test]
    fn returns_complete_mail_for_detail_query() {
        let mut mail = with_one_message();
        let expected = mail_detail(&stored_message(&mail, 1));

        let responses = mail.process(7, &request(false, &detail_query(1))).unwrap();
        let [response] = responses.as_slice() else {
            panic!("detail query returned more than one response");
        };
        assert_eq!(response.note, 7);
        assert_eq!(response.payload, transport(0, 5, &expected));

        let tagged_end = response
            .payload
            .windows(4)
            .position(|bytes| bytes == [0xfd, 1, 0, b'z'])
            .unwrap()
            + 4;
        assert_eq!(&response.payload[tagged_end..], STORED_BODY.as_bytes());
        assert_eq!(response.payload[0] & MORE, 0);
    }

    /// The upper two bytes of a mail id are always zero, so a query that sets
    /// them addresses no message the server could have handed out.
    #[test]
    fn a_detail_query_for_an_unknown_id_returns_nothing() {
        let mut mail = with_one_message();
        let mut query = vec![0xfd, 27, 0, b'S', 7, 0, 0];
        query.extend([1, 0, 0, 0, 1, 0]);
        query.extend(b"kuvspoftcandgbzxy");

        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &empty_mail_result()),
            }])
        );
    }

    fn read_new_query() -> Vec<u8> {
        let mut query = vec![0xfd, 27, 0, b'S', 1, 0, 1];
        query.extend([0; 6]);
        query.extend(b"kuvspoftcandgbzxy");
        query
    }

    #[test]
    fn returns_stored_mail_once_for_read_new_query() {
        let mut mail = with_one_message();
        let query = read_new_query();
        let expected = mail_detail(&stored_message(&mail, 1));

        assert_eq!(
            mail.process(7, &request(false, &query)),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &expected),
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
        let mut mail = with_one_message();
        let expected = mail_detail(&stored_message(&mail, 1));
        mail.process(7, &request(false, &read_new_query())).unwrap();

        assert_eq!(
            mail.process(7, &request(false, &detail_query(1))),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &expected),
            }])
        );
    }

    #[test]
    fn returns_each_mailbox_item_in_its_own_response() {
        let mut mail = with_one_message();
        assert!(mail.accept_outgoing(outgoing("Second", "Second body")));

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
        let mut data = app_frame(RECORD_MARKER, b"tGUEST");
        data.extend(app_frame(RECORD_MARKER, b"sSent subject"));
        data.extend(app_frame(RECORD_MARKER, b"DDemo attachment nnoise"));
        data.extend(app_frame(RECORD_MARKER, b"nSent body"));
        data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

        let mut mail = with_one_message();
        assert!(mail.accept_outgoing(data));

        let stored = stored_message(&mail, 2);
        assert_eq!(stored.subject, "Sent subject");
        assert_eq!(stored.body, "Sent body");

        let responses = mail.process(7, &request(false, &detail_query(2))).unwrap();
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
        let mut data = outgoing("Short", "Short body");
        data.extend_from_slice(b" leftovers of a longer object");

        let mut mail = mail_server();
        assert!(mail.accept_outgoing(data));

        let stored = stored_message(&mail, 1);
        assert_eq!(stored.subject, "Short");
        assert_eq!(stored.body, "Short body");
    }

    #[test]
    fn read_new_iterates_all_mail_then_terminates() {
        let mut mail = with_one_message();
        assert!(mail.accept_outgoing(outgoing("Second", "Second body")));

        let query = read_new_query();
        let first = mail.process(7, &request(false, &query)).unwrap();
        let second = mail.process(7, &request(false, &query)).unwrap();
        let third = mail.process(7, &request(false, &query)).unwrap();
        assert!(first[0].payload.ends_with(STORED_BODY.as_bytes()));
        assert!(second[0].payload.ends_with(b"Second body"));
        assert_eq!(third[0].payload, transport(0, 5, &empty_mail_result()));
    }

    #[test]
    fn rejects_mail_that_cannot_fit_in_one_vipc_response() {
        let mut mail = mail_server();

        assert!(!mail.accept_outgoing(outgoing("Large", &"x".repeat(40_000))));
        assert!(mailbox::list(&mail.conn, mail.owner_id).unwrap().is_empty());
    }

    /// The mailbox belongs to the account that signed on, so a message stored
    /// by one session is invisible to a session signed on as somebody else.
    #[test]
    fn a_mailbox_is_not_shared_between_accounts() {
        let mail = with_one_message();
        let other = db::find_user(&mail.conn, "GRiD", "Systems", "MANAGER")
            .unwrap()
            .unwrap();
        let mut other_mail = MailServer::new(mail.conn.clone(), other.id, other.user);

        assert!(
            mailbox::list(&other_mail.conn, other.id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            other_mail.process(7, &request(false, &detail_query(1))),
            Some(vec![MailResponse {
                note: 7,
                payload: transport(0, 5, &empty_mail_result()),
            }])
        );
    }

    #[test]
    fn rejects_transport_error() {
        assert_eq!(mail_server().process(0, &[0, 5, 1, 0]), None);
    }
}
