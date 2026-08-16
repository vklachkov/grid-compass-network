use std::io;

use super::{error::FrameError, raw_frame::RawFrame, utils::ReadExt};

/// End-of-message flag.
pub const EOM_FLAG_ON: u8 = 1;

/// Phone data link protocol version bytes.
const PDL_VERSION: [u8; 2] = *b"02";

#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub flags: u8,           // flags
    pub window_size: u8,     // windowSize
    pub seq_number: u8,      // seqNumber
    pub body: FrameBody<'a>, // frameType combined with data
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum FrameType {
    Rfc = 1,
    Ack = 2,
    Disc = 3,
    Ping = 4,
    Data = 5,
}

impl FrameType {
    fn from_repr(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::Rfc,
            2 => Self::Ack,
            3 => Self::Disc,
            4 => Self::Ping,
            5 => Self::Data,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FrameBody<'a> {
    Rfc(RfcFrameBody),
    Ack(ShortFrameBody),
    Disc(ShortFrameBody),
    Ping(ShortFrameBody),
    Data(&'a [u8]),
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct RfcFrameBody {
    pub connection_id: u8, // pdlConn
    pub version: [u8; 2],  // pdlVersion
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct ShortFrameBody {
    pub connection_id: u8, // pdlConn
}

impl<'a> Frame<'a> {
    pub fn rfc(conn_id: u8, seq_number: u8) -> Self {
        Frame {
            flags: EOM_FLAG_ON,
            window_size: 1,
            seq_number,
            body: FrameBody::Rfc(RfcFrameBody {
                connection_id: conn_id,
                version: PDL_VERSION,
            }),
        }
    }

    pub fn ack(conn_id: u8, seq_number: u8) -> Self {
        Self {
            flags: EOM_FLAG_ON,
            window_size: b'A',
            seq_number,
            body: FrameBody::Ack(ShortFrameBody {
                connection_id: conn_id,
            }),
        }
    }

    pub fn data(flags: u8, seq_number: u8, body: &'a [u8]) -> Self {
        Self {
            flags,
            window_size: b'D',
            seq_number,
            body: FrameBody::Data(body),
        }
    }

    pub fn try_from_raw(raw: &'a RawFrame) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(&raw.data);

        let [ty, flags, window_size, seq_number] = cursor.read_array()?;

        let Some(ty) = FrameType::from_repr(ty) else {
            return Err(FrameError::Validation {
                reason: format!("unknown frame type {ty:#04x}"),
            });
        };

        let body = match ty {
            FrameType::Rfc => FrameBody::Rfc(RfcFrameBody {
                connection_id: cursor.read_u8()?,
                version: cursor.read_array()?,
            }),
            FrameType::Ack => FrameBody::Ack(ShortFrameBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Disc => FrameBody::Disc(ShortFrameBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Ping => FrameBody::Ping(ShortFrameBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Data => {
                let body = &raw.data[cursor.position() as usize..];
                FrameBody::Data(body)
            }
        };

        // TODO: check that the buffer has no unparsed fields left.

        Ok(Self {
            flags,
            window_size,
            seq_number,
            body,
        })
    }

    /// Writing into a caller owned buffer lets a session reuse one allocation
    /// for every frame it sends instead of building a fresh `Vec` per response.
    pub fn write_into(&self, dst: &mut Vec<u8>) {
        dst.push(self.body.repr());
        dst.push(self.flags);
        dst.push(self.window_size);
        dst.push(self.seq_number);

        match self.body {
            FrameBody::Rfc(body) => {
                dst.push(body.connection_id);
                dst.extend_from_slice(&body.version);
            }
            FrameBody::Ack(body) | FrameBody::Disc(body) | FrameBody::Ping(body) => {
                dst.push(body.connection_id);
            }
            FrameBody::Data(body) => {
                dst.extend_from_slice(body);
            }
        }
    }

    pub fn into_raw(self) -> RawFrame {
        let mut data = Vec::with_capacity(5);
        self.write_into(&mut data);
        RawFrame::new(data)
    }
}

impl FrameBody<'_> {
    fn repr(self) -> u8 {
        (match self {
            FrameBody::Rfc(_) => FrameType::Rfc,
            FrameBody::Ack(_) => FrameType::Ack,
            FrameBody::Disc(_) => FrameType::Disc,
            FrameBody::Ping(_) => FrameType::Ping,
            FrameBody::Data(_) => FrameType::Data,
        }) as u8
    }
}
