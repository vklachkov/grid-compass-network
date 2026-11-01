use std::io;

use crate::shared::{FrameError, io::ReadExt};

/// End-of-message flag.
pub const EOM_FLAG_ON: u8 = 1;

/// Phone data link protocol version bytes.
const PDL_VERSION: [u8; 2] = *b"02";

#[derive(Clone, Copy, Debug)]
pub struct Frame<D> {
    pub flags: u8,
    pub window_size: u8,
    pub seq_number: u8,
    pub body: FrameBody<D>,
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
pub enum FrameBody<D> {
    Rfc(RfcFrameBody),
    Ack(ShortFrameBody),
    Disc(ShortFrameBody),
    Ping(ShortFrameBody),
    Data(D),
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct RfcFrameBody {
    pub connection_id: u8,
    pub version: [u8; 2],
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct ShortFrameBody {
    pub connection_id: u8,
}

impl<'a> Frame<&'a [u8]> {
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

    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(data);

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
                let body = data.get(cursor.position() as usize..).unwrap_or_default();
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
}

impl Frame<Vec<u8>> {
    pub fn try_from_vec(mut data: Vec<u8>) -> Result<Self, FrameError> {
        let frame = Frame::try_from_slice(&data)?;
        let body = match frame.body {
            FrameBody::Rfc(body) => FrameBody::Rfc(body),
            FrameBody::Ack(body) => FrameBody::Ack(body),
            FrameBody::Disc(body) => FrameBody::Disc(body),
            FrameBody::Ping(body) => FrameBody::Ping(body),
            FrameBody::Data(_) => {
                let flags = frame.flags;
                let window_size = frame.window_size;
                let seq_number = frame.seq_number;
                data.drain(..4);
                return Ok(Self {
                    flags,
                    window_size,
                    seq_number,
                    body: FrameBody::Data(data),
                });
            }
        };
        Ok(Self {
            flags: frame.flags,
            window_size: frame.window_size,
            seq_number: frame.seq_number,
            body,
        })
    }
}

impl<D: AsRef<[u8]>> Frame<D> {
    pub fn write_into(&self, dst: &mut Vec<u8>) {
        dst.push(self.body.repr());
        dst.push(self.flags);
        dst.push(self.window_size);
        dst.push(self.seq_number);

        match &self.body {
            FrameBody::Rfc(body) => {
                dst.push(body.connection_id);
                dst.extend_from_slice(&body.version);
            }
            FrameBody::Ack(body) | FrameBody::Disc(body) | FrameBody::Ping(body) => {
                dst.push(body.connection_id);
            }
            FrameBody::Data(body) => {
                dst.extend_from_slice(body.as_ref());
            }
        }
    }
}

impl<D> FrameBody<D> {
    fn repr(&self) -> u8 {
        (match self {
            FrameBody::Rfc(_) => FrameType::Rfc,
            FrameBody::Ack(_) => FrameType::Ack,
            FrameBody::Disc(_) => FrameType::Disc,
            FrameBody::Ping(_) => FrameType::Ping,
            FrameBody::Data(_) => FrameType::Data,
        }) as u8
    }
}
