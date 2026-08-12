use std::io;

use super::{
    error::FrameError,
    utils::{CursorExt, ReadExt},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageType(pub u16);

#[derive(Clone, Debug)]
pub struct IncomingMessage<'a> {
    pub note: u16,
    pub body: IncomingMessageBody<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct IncomingMessageBody<'a> {
    pub ty: MessageType,
    pub payload: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct OutgoingMessage {
    pub note: u16,
    pub body: OutgoingMessageBody,
}

#[derive(Clone, Debug)]
pub struct OutgoingMessageBody {
    pub ty: MessageType,
    pub payload: Vec<u8>,
}

impl<'a> IncomingMessage<'a> {
    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, FrameError> {
        let mut cursor = io::Cursor::new(data);

        let ty = MessageType(cursor.read_u16()?);
        let note = cursor.read_u16()?;
        let payload_length = cursor.read_u16()? as usize;
        let payload = cursor.read_slice(payload_length)?;

        let remaining = data.len().saturating_sub(cursor.position() as usize);
        if remaining != 0 {
            return Err(FrameError::Validation {
                reason: format!("VIPC message: {remaining} trailing bytes"),
            });
        }

        Ok(Self {
            note,
            body: IncomingMessageBody { ty, payload },
        })
    }
}

impl OutgoingMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_length =
            u16::try_from(self.body.payload.len()).expect("VIPC payload exceeds u16 length");
        let mut data = Vec::with_capacity(6 + self.body.payload.len());

        data.extend(self.body.ty.0.to_le_bytes());
        data.extend(self.note.to_le_bytes());
        data.extend(payload_length.to_le_bytes());
        data.extend_from_slice(&self.body.payload);

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generic_message() {
        let message = IncomingMessage::try_from_slice(&[83, 0, 7, 0, 3, 0, 1, 2, 3]).unwrap();

        assert_eq!(message.note, 7);
        assert_eq!(message.body.ty, MessageType(83));
        assert_eq!(message.body.payload, [1, 2, 3]);
    }

    #[test]
    fn serializes_generic_message() {
        let message = OutgoingMessage {
            note: 7,
            body: OutgoingMessageBody {
                ty: MessageType(83),
                payload: vec![1, 2, 3],
            },
        };

        assert_eq!(message.to_bytes(), [83, 0, 7, 0, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn rejects_trailing_bytes() {
        assert!(IncomingMessage::try_from_slice(&[83, 0, 7, 0, 1, 0, 1, 2]).is_err());
    }
}
