use std::io;

use zerocopy::{FromBytes, Immutable, IntoBytes};

use super::FrameError;

pub fn read_small_slice<'a>(cursor: &mut io::Cursor<&'a [u8]>) -> Result<&'a [u8], FrameError> {
    let length = cursor.read_u8()?;
    cursor.read_slice(length as usize).map_err(Into::into)
}

/// Patching the length in after the body is written makes the prefix and the
/// bytes it describes impossible to disagree, which a separate pre-pass over
/// the same data cannot guarantee.
pub fn with_u16_len(
    dst: &mut Vec<u8>,
    f: impl FnOnce(&mut Vec<u8>) -> Result<(), FrameError>,
) -> Result<(), FrameError> {
    let at = dst.len();
    dst.extend([0, 0]);

    f(dst)?;

    let body_length = dst.len().saturating_sub(at + 2);
    let Ok(length) = u16::try_from(body_length) else {
        return Err(FrameError::Validation {
            reason: format!("a block of {body_length} bytes overruns its u16 length prefix"),
        });
    };

    let Some(prefix) = dst.get_mut(at..at + 2) else {
        return Err(FrameError::Validation {
            reason: "the block body consumed its own length prefix".to_owned(),
        });
    };
    prefix.copy_from_slice(&length.to_le_bytes());

    Ok(())
}

pub trait ReadExt: io::Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buffer = [0; 1];
        self.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        let buffer = ReadExt::read_array(self)?;
        Ok(u16::from_le_bytes(buffer))
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        let buffer = ReadExt::read_array(self)?;
        Ok(u32::from_le_bytes(buffer))
    }

    fn read_array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buffer = [0; N];
        self.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn read_struct<T: FromBytes + IntoBytes>(&mut self) -> io::Result<T> {
        let mut value = T::new_zeroed();
        self.read_exact(value.as_mut_bytes())?;
        Ok(value)
    }
}

impl<T: io::Read + ?Sized> ReadExt for T {}

pub trait WriteExt: io::Write {
    fn write_u8(&mut self, value: u8) -> io::Result<()> {
        self.write_all(&[value])
    }

    fn write_u16(&mut self, value: u16) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_struct<T: IntoBytes + Immutable + ?Sized>(&mut self, value: &T) -> io::Result<()> {
        self.write_all(value.as_bytes())
    }

    /// The length is checked rather than truncated: a silently shortened prefix
    /// would desynchronize the client's parser instead of failing here.
    fn write_u8_slice(&mut self, value: &[u8]) -> Result<(), FrameError> {
        let Ok(length) = u8::try_from(value.len()) else {
            return Err(FrameError::Validation {
                reason: format!(
                    "a slice of {} bytes overruns its u8 length prefix",
                    value.len()
                ),
            });
        };

        self.write_u8(length)?;
        self.write_all(value)?;

        Ok(())
    }
}

impl<T: io::Write + ?Sized> WriteExt for T {}

pub trait CursorExt<'a> {
    fn read_remainder(&mut self) -> &'a [u8];

    fn read_slice(&mut self, length: usize) -> io::Result<&'a [u8]>;
}

impl<'a> CursorExt<'a> for io::Cursor<&'a [u8]> {
    fn read_remainder(&mut self) -> &'a [u8] {
        let start = self.position() as usize;
        let end = self.get_ref().len();
        self.set_position(end as u64);
        self.get_ref().get(start..).unwrap_or_default()
    }

    fn read_slice(&mut self, length: usize) -> io::Result<&'a [u8]> {
        let start = self.position() as usize;

        let Some(end) = start.checked_add(length) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "slice length overflow",
            ));
        };

        let data = self.get_ref();

        let Some(value) = data.get(start..end) else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("not enough bytes to read {length} bytes at offset {start}"),
            ));
        };

        self.set_position(end as u64);

        Ok(value)
    }
}
