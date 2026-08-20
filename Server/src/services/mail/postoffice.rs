use std::{io, rc::Rc};

use log::{debug, error, warn};
use rusqlite::Connection;

use super::protocol::{
    MAIL_ID_LEN, MAIL_SERVICE_ID, MORE, MailId, PROTOCOL_VERSION, RECORD_HEADER_LEN, RECORD_MARKER,
    SESSION_MARKER, SessionInitialize, TAG_TERMINATOR, TRANSPORT_HEADER_LEN, TransportFragment,
    app_frame, single_record, transport,
};
use crate::{
    db::mailbox::{self, Message, NewMessage},
    shared::{
        Tlv,
        io::{CursorExt, ReadExt},
    },
};

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

const SESSION_INITIALIZE_RESPONSE: &[u8] = b"z";

const MAIL_HEADER_RESERVED_LEN: usize = 4;
const MAIL_HEADER_VALUE_LEN: usize =
    TAG_LEN + MAIL_ID_LEN + MAIL_HEADER_RESERVED_LEN + MailStatus::WIRE_LEN;
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

pub struct MailServer {
    fragments: Vec<Pending>,
    pub(super) conn: Rc<Connection>,
    /// The mailbox every request in this session reads and writes, and the
    /// sender of everything it writes: mail is stored per account, so a session
    /// can only ever reach the one it signed on to.
    pub(super) owner_id: i64,
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
pub(super) struct ChannelResponse {
    pub(super) flags: u8,
    pub(super) data: Vec<u8>,
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
                let message = match MailId::from_wire(id).value() {
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

        let fragment = match TransportFragment::parse(payload) {
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
            CHANNEL_SESSION
                if SessionInitialize::parse(&pending.data)
                    == Some(SessionInitialize {
                        service_id: MAIL_SERVICE_ID,
                        protocol_version: PROTOCOL_VERSION,
                    }) =>
            {
                vec![ChannelResponse::final_message(app_frame(
                    SESSION_MARKER,
                    SESSION_INITIALIZE_RESPONSE,
                ))]
            }
            CHANNEL_CLOSE if pending.data.is_empty() => {
                vec![ChannelResponse::final_message(empty_mail_result())]
            }
            CHANNEL_TAGGED_REQUEST
                if Tlv::marker_u16(&pending.data, RECORD_MARKER).all_records_valid() =>
            {
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

impl<'a> Outgoing<'a> {
    fn parse(original: &'a [u8]) -> Option<Self> {
        Some(Self {
            recipient: text(Tlv::marker_u16(original, RECORD_MARKER).find_tag(TAG_RECIPIENT)?)?,
            subject: text(Tlv::marker_u16(original, RECORD_MARKER).find_tag(TAG_SUBJECT)?)?,
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
        id: MailId::from_u32(message.mail_id).wire_bytes(),
        status: MailStatus::Read,
    }
    .encode()
}

pub(super) fn mailbox_list(messages: &[Message]) -> Vec<ChannelResponse> {
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

pub(super) fn empty_mail_result() -> Vec<u8> {
    app_frame(RECORD_MARKER, &[TAG_TERMINATOR])
}

pub(super) fn tagged_record(tag: u8, value: &str) -> Vec<u8> {
    let mut record = vec![tag];
    record.extend_from_slice(value.as_bytes());
    app_frame(RECORD_MARKER, &record)
}

pub(super) fn mail_detail(message: &Message) -> Vec<u8> {
    let mut data = Vec::with_capacity(64);
    data.extend(mail_header(message));
    data.extend(tagged_record(TAG_RECIPIENT, &message.recipient.name));
    data.extend(tagged_record(TAG_SENDER, &message.sender.name));
    data.extend(tagged_record(TAG_DISPLAY_SENDER, &message.sender.name));
    data.extend(tagged_record(TAG_SUBJECT, &message.subject));
    data.extend(tagged_record(TAG_BODY, &message.body));
    data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

    // After parsing `z`, GRiDMail directly drains the remaining response stream.
    data.extend_from_slice(message.body.as_bytes());
    data
}
