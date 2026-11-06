use std::ops::Deref;

use arrayvec::ArrayVec;

use crate::shared::crc::crc16;

use super::{DLE, ETX, STX};

/// PDL frame data with its header, without DLE stuffing, delimiters, or CRC.
#[derive(Clone, Default)]
#[repr(transparent)]
pub struct RawFrame(Box<ArrayVec<u8, { Self::MAX_SIZE }>>);

impl RawFrame {
    /// Maximum frame size before DLE stuffing, without delimiters or CRC.
    ///
    /// Pdl.inc defines a 4-byte PDL header and 526 bytes of DLC data:
    /// 504 bytes of VFS data + a 10-byte VFS header + a 12-byte VIPC header.
    pub const MAX_SIZE: usize = 4 + 526;

    /// Creates a frame by taking ownership of the buffer as is.
    pub fn new(buffer: Box<ArrayVec<u8, { Self::MAX_SIZE }>>) -> Self {
        Self(buffer)
    }

    /// Prepares the frame for sending by applying DLE stuffing and adding markers and CRC16.
    pub(super) fn encode(&self) -> EncodedFrame {
        EncodedFrame::encode(self)
    }
}

impl Deref for RawFrame {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A PDL frame ready to send, with delimiters, DLE stuffing, and CRC16.
#[derive(Clone, Default)]
#[repr(transparent)]
pub struct EncodedFrame(Box<ArrayVec<u8, { Self::MAX_SIZE }>>);

impl EncodedFrame {
    // DLE STX + stuffed PDL frame + DLE ETX + CRC16.
    // Each input byte needs at most two output bytes.
    pub(super) const MAX_SIZE: usize = 2 + 2 * RawFrame::MAX_SIZE + 2 + 2;

    fn encode(frame: &RawFrame) -> Self {
        let mut thiz = Self::default();

        // SAFETY: The frame has at most Frame::MAX_SIZE bytes. Each needs at most
        // two output bytes. Delimiters and CRC need six more bytes.
        // The buffer holds this maximum size, so every push fits.
        unsafe {
            thiz.0.push_unchecked(DLE);
            thiz.0.push_unchecked(STX);

            for &byte in frame.iter() {
                // DLE stuffing doubles data DLE bytes to keep them separate from delimiters.
                if byte == DLE {
                    thiz.0.push_unchecked(byte);
                }

                thiz.0.push_unchecked(byte);
            }

            thiz.0.push_unchecked(DLE);
            thiz.0.push_unchecked(ETX);

            let [crc_low, crc_high] = crc16(frame).to_le_bytes();

            thiz.0.push_unchecked(crc_low);
            thiz.0.push_unchecked(crc_high);
        }

        thiz
    }
}

impl AsRef<[u8]> for EncodedFrame {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for EncodedFrame {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
