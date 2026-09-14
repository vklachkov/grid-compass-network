use std::{io, mem::size_of};

use log::error;

use crate::shared::{
    Tlv,
    io::{CursorExt, ReadExt},
};

pub const RECORD_MARKER: u8 = 0xfd;
pub const SESSION_MARKER: u8 = 0xfe;
pub const TAG_TERMINATOR: u8 = b'z';
pub const SESSION_COMMAND: u8 = b'a';

pub const MAIL_SERVICE_ID: u16 = 11_400;
pub const BROADCAST_SERVICE_ID: u16 = 11_500;
pub const PROTOCOL_VERSION: u8 = 1;

pub const MORE: u8 = 1;
pub const TRANSPORT_HEADER_LEN: usize = 4;
pub const TRANSPORT_SUCCESS: u16 = 0;
pub const RECORD_HEADER_LEN: usize = 3;
pub const MAIL_ID_LEN: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionInitialize {
    pub service_id: u16,
    pub protocol_version: u8,
}

impl SessionInitialize {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let value = single_record(data, SESSION_MARKER, SESSION_COMMAND)?;
        let mut cursor = io::Cursor::new(value);
        let initialize = Self {
            service_id: cursor.read_u16().ok()?,
            protocol_version: cursor.read_u8().ok()?,
        };
        cursor.read_remainder().is_empty().then_some(initialize)
    }

    #[cfg(test)]
    pub fn encode(self) -> Vec<u8> {
        let mut value = vec![SESSION_COMMAND];
        value.extend(self.service_id.to_le_bytes());
        value.push(self.protocol_version);
        app_frame(SESSION_MARKER, &value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailId([u8; MAIL_ID_LEN]);

impl MailId {
    pub fn from_wire(bytes: [u8; MAIL_ID_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_u32(value: u32) -> Self {
        let mut bytes = [0; MAIL_ID_LEN];
        if let Some(low) = bytes.first_chunk_mut() {
            *low = value.to_le_bytes();
        }
        Self(bytes)
    }

    pub fn wire_bytes(self) -> [u8; MAIL_ID_LEN] {
        self.0
    }

    /// The wire form of a mail id is six bytes of which the client only ever
    /// fills the lower four, so a query naming the upper two addresses no
    /// stored message.
    pub fn value(self) -> Option<u32> {
        let (value, reserved) = self.0.split_at(size_of::<u32>());
        let value = *value.first_chunk()?;

        reserved
            .iter()
            .all(|byte| *byte == 0)
            .then(|| u32::from_le_bytes(value))
    }
}

pub struct TransportFragment<'a> {
    pub flags: u8,
    pub connection_id: u8,
    pub error: u16,
    pub data: &'a [u8],
}

impl<'a> TransportFragment<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        let mut cursor = io::Cursor::new(data);

        Some(Self {
            flags: cursor.read_u8().ok()?,
            connection_id: cursor.read_u8().ok()?,
            error: cursor.read_u16().ok()?,
            data: cursor.read_remainder(),
        })
    }
}

pub fn single_record(data: &[u8], marker: u8, tag: u8) -> Option<&[u8]> {
    match Tlv::marker_u16(data, marker).collect_all().ok()?.as_slice() {
        [entry] if entry.tag == tag => Some(entry.value),
        _ => None,
    }
}

/// The length field cannot describe a payload this long, so the record is cut
/// down to what the field can name: a client reading a truncated record stays in
/// sync with the stream, one reading a record whose header lies does not.
pub fn app_frame(marker: u8, payload: &[u8]) -> Vec<u8> {
    let payload = if payload.len() > usize::from(u16::MAX) {
        error!(target: "mail", "truncated an application record of {} bytes", payload.len());
        payload.get(..usize::from(u16::MAX)).unwrap_or_default()
    } else {
        payload
    };

    // Taking the length from the truncated payload rather than the other way
    // round is what keeps the header from ever describing more than it carries.
    let length = u16::try_from(payload.len()).unwrap_or(u16::MAX);

    let mut frame = Vec::with_capacity(payload.len() + RECORD_HEADER_LEN);
    frame.push(marker);
    frame.extend(length.to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

pub fn transport(flags: u8, connection_id: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + TRANSPORT_HEADER_LEN);
    payload.push(flags);
    payload.push(connection_id);
    payload.extend(TRANSPORT_SUCCESS.to_le_bytes());
    payload.extend_from_slice(data);
    payload
}
