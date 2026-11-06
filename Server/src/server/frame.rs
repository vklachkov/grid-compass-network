use std::{borrow::Cow, io};

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::shared::{FrameError, io::ReadExt};

/// End-of-message flag.
pub const EOM_FLAG_ON: u8 = 1;

/// Phone data link protocol version bytes.
const PDL_VERSION: [u8; 2] = *b"02";

#[derive(Clone, Debug)]
pub struct Frame<'body> {
    pub flags: u8,
    pub window_size: u8,
    pub seq_number: u8,
    pub body: FrameBody<'body>,
}

#[derive(Clone, Copy, Debug, FromPrimitive)]
#[repr(u8)]
enum FrameType {
    Rfc = 1,
    Ack = 2,
    Disc = 3,
    Ping = 4,
    Data = 5,
}

#[derive(Clone, Debug)]
pub enum FrameBody<'data> {
    Rfc(RfcBody),
    Ack(ConnectionBody),
    Disc(ConnectionBody),
    Ping(ConnectionBody),
    Data(Cow<'data, [u8]>),
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct RfcBody {
    pub connection_id: u8,
    pub version: [u8; 2],
}

// RxShortBufferDescType
#[derive(Clone, Copy, Debug)]
pub struct ConnectionBody {
    pub connection_id: u8,
}

impl<'body> Frame<'body> {
    pub fn rfc(conn_id: u8, seq_number: u8) -> Self {
        Frame {
            flags: EOM_FLAG_ON,
            window_size: 1,
            seq_number,
            body: FrameBody::Rfc(RfcBody {
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
            body: FrameBody::Ack(ConnectionBody {
                connection_id: conn_id,
            }),
        }
    }

    pub fn data(flags: u8, seq_number: u8, body: Cow<'body, [u8]>) -> Self {
        Self {
            flags,
            window_size: b'D',
            seq_number,
            body: FrameBody::Data(body),
        }
    }

    pub fn deserialize(data: Cow<'body, [u8]>) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(data.as_ref());

        let [ty, flags, window_size, seq_number] = cursor.read_array()?;

        let Some(ty) = FrameType::from_u8(ty) else {
            return Err(FrameError::Validation {
                reason: format!("unknown frame type {ty:#04x}"),
            });
        };

        let body = match ty {
            FrameType::Rfc => FrameBody::Rfc(RfcBody {
                connection_id: cursor.read_u8()?,
                version: cursor.read_array()?,
            }),
            FrameType::Ack => FrameBody::Ack(ConnectionBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Disc => FrameBody::Disc(ConnectionBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Ping => FrameBody::Ping(ConnectionBody {
                connection_id: cursor.read_u8()?,
            }),
            FrameType::Data => {
                let offset = cursor.position() as usize;
                let body = match data {
                    Cow::Borrowed(bytes) => Cow::Borrowed(&bytes[offset..]),
                    Cow::Owned(mut bytes) => {
                        bytes.drain(..offset);
                        Cow::Owned(bytes)
                    }
                };
                return Ok(Self {
                    flags,
                    window_size,
                    seq_number,
                    body: FrameBody::Data(body),
                });
            }
        };

        if cursor.position() != data.len() as u64 {
            return Err(FrameError::Validation {
                reason: "trailing bytes in a control frame".to_owned(),
            });
        }

        Ok(Self {
            flags,
            window_size,
            seq_number,
            body,
        })
    }
}

impl Frame<'_> {
    pub fn serialized_len(&self) -> usize {
        4 + match &self.body {
            FrameBody::Rfc(_) => 3,
            FrameBody::Ack(_) | FrameBody::Disc(_) | FrameBody::Ping(_) => 1,
            FrameBody::Data(body) => body.as_ref().len(),
        }
    }

    pub fn write_into(&self, mut dst: impl io::Write) -> io::Result<()> {
        dst.write_all(&[
            self.body.repr(),
            self.flags,
            self.window_size,
            self.seq_number,
        ])?;

        match &self.body {
            FrameBody::Rfc(body) => {
                dst.write_all(&[body.connection_id])?;
                dst.write_all(&body.version)?;
            }
            FrameBody::Ack(body) | FrameBody::Disc(body) | FrameBody::Ping(body) => {
                dst.write_all(&[body.connection_id])?;
            }
            FrameBody::Data(body) => {
                dst.write_all(body.as_ref())?;
            }
        }
        Ok(())
    }
}

impl FrameBody<'_> {
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
