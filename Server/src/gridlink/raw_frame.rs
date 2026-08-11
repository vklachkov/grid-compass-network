use std::io;

use super::{
    error::FrameError,
    utils::{ReadExt, WriteExt},
};

/// Data Link Escape. Used to prefix special commands or escape data bytes.
const DLE: u8 = 0x10;
/// Start of Text. Marks the beginning of the payload.
const STX: u8 = 0x02;
/// End of Text. Marks the end of the payload.
const ETX: u8 = 0x03;

/// Size of the preallocated buffer for a frame.
const AVERAGE_FRAME_SIZE: usize = 64;

/// Four-byte PDL header plus the maximum 526-byte DLC data area.
const MAX_FRAME_SIZE: usize = 4 + 526;

#[derive(Clone, Debug)]
pub struct RawFrame {
    pub data: Vec<u8>,
}

impl RawFrame {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Reads and unstuffs frame data from an I/O source.
    pub fn read_from_io(mut src: impl io::Read) -> Result<Self, FrameError> {
        let buffer = Self::read_unstuffed(&mut src)?;
        let buffer_crc = crc16(&buffer);

        let crc = src.read_u16()?;

        if crc != buffer_crc {
            return Err(FrameError::InvalidCrc {
                expected: buffer_crc,
                found: crc,
            });
        }

        Ok(Self::new(buffer))
    }

    fn read_unstuffed(mut src: impl io::Read) -> Result<Vec<u8>, FrameError> {
        let mut buffer = Vec::with_capacity(AVERAGE_FRAME_SIZE);

        loop {
            let byte = src.read_u8()?;
            if byte != DLE {
                Self::push_unstuffed(&mut buffer, byte)?;
                continue;
            }

            let byte = src.read_u8()?;
            match byte {
                DLE => Self::push_unstuffed(&mut buffer, DLE)?,
                STX => buffer.clear(),
                ETX => break,
                _ => {
                    return Err(FrameError::MalformedFrameMarker { marker: byte });
                }
            }
        }

        Ok(buffer)
    }

    fn push_unstuffed(buffer: &mut Vec<u8>, byte: u8) -> Result<(), FrameError> {
        if buffer.len() >= MAX_FRAME_SIZE {
            return Err(FrameError::FrameTooLarge {
                max: MAX_FRAME_SIZE,
            });
        }

        buffer.push(byte);
        Ok(())
    }

    /// Stuffs and writes frame data to an I/O destination.
    pub fn write_to_io(&self, dst: impl io::Write) -> Result<(), FrameError> {
        let crc = crc16(&self.data);

        let count_of_dle = self.data.iter().filter(|&&b| b == DLE).count();
        if count_of_dle == 0 {
            return Self::write_stuffed(dst, &self.data, crc);
        }

        let mut stuffed_frame_data = Vec::with_capacity(self.data.len() + count_of_dle);
        for &b in self.data.iter() {
            stuffed_frame_data.push(b);
            if b == DLE {
                stuffed_frame_data.push(DLE);
            }
        }

        Self::write_stuffed(dst, &stuffed_frame_data, crc)
    }

    fn write_stuffed(mut dst: impl io::Write, data: &[u8], crc: u16) -> Result<(), FrameError> {
        info!("write data stuffed: {data:02x?}");

        dst.write_all(&[DLE, STX])?;
        dst.write_all(data)?;
        dst.write_all(&[DLE, ETX])?;
        dst.write_u16(crc)?;

        Ok(())
    }
}

/// Calculates the CRC16 ARC checksum for the given data.
fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0;

    for byte in data {
        crc ^= *byte as u16;

        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_maximum_unstuffed_frame() {
        let data = vec![DLE; MAX_FRAME_SIZE];
        let mut encoded = Vec::new();
        RawFrame::new(data.clone())
            .write_to_io(&mut encoded)
            .unwrap();

        let decoded = RawFrame::read_from_io(encoded.as_slice()).unwrap();
        assert_eq!(decoded.data, data);
    }

    #[test]
    fn rejects_frame_one_byte_over_maximum() {
        let mut encoded = vec![DLE, STX];
        encoded.extend(std::iter::repeat_n(0x55, MAX_FRAME_SIZE + 1));

        assert!(matches!(
            RawFrame::read_from_io(encoded.as_slice()),
            Err(FrameError::FrameTooLarge {
                max: MAX_FRAME_SIZE
            })
        ));
    }

    #[test]
    fn accepts_full_vfs_write_frame() {
        let mut vipc = Vec::with_capacity(518);
        vipc.extend(83u16.to_le_bytes());
        vipc.extend(3u16.to_le_bytes());
        vipc.extend(512u16.to_le_bytes());
        vipc.extend(5u16.to_le_bytes());
        vipc.extend(0x7db2u16.to_le_bytes());
        vipc.extend(3u16.to_le_bytes());
        vipc.extend(504u16.to_le_bytes());
        vipc.extend(std::iter::repeat_n(DLE, 504));

        let mut data_frame = Vec::with_capacity(524);
        data_frame.extend(0u16.to_le_bytes());
        data_frame.extend(0x6542u16.to_le_bytes());
        data_frame.extend(1u16.to_le_bytes());
        data_frame.extend(vipc);

        let frame = crate::gridlink::Frame::data(1, 54, &data_frame).to_raw();
        assert_eq!(frame.data.len(), 528);

        let mut encoded = Vec::new();
        frame.write_to_io(&mut encoded).unwrap();
        let decoded = RawFrame::read_from_io(encoded.as_slice()).unwrap();
        let parsed = crate::gridlink::Frame::try_from_raw(&decoded).unwrap();
        let crate::gridlink::FrameBody::Data(data) = parsed.body else {
            panic!("expected data frame");
        };
        let crate::gridlink::data_frame::DataFrameRequest::Msg { payload, .. } =
            crate::gridlink::data_frame::DataFrameRequest::try_from_slice(data).unwrap()
        else {
            panic!("expected VIPC message");
        };
        let message = crate::gridlink::vipc::IncomingMessage::try_from_slice(payload).unwrap();
        let crate::gridlink::vipc::IncomingMessageBody::Vfs(request) = message.body else {
            panic!("expected VFS request");
        };
        let crate::gridlink::vipc::VfsRequestBody::Write(write) = request.body else {
            panic!("expected VFS write");
        };
        assert_eq!(write.data.len(), 504);
        assert!(write.data.iter().all(|&byte| byte == DLE));
    }
}
