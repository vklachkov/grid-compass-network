use std::io;

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

    let length = u16_len(dst.len() - at - 2, "length-prefixed block")?;
    dst[at..at + 2].copy_from_slice(&length.to_le_bytes());

    Ok(())
}

pub fn u8_len(length: usize, what: &str) -> Result<u8, FrameError> {
    u8::try_from(length).map_err(|_| FrameError::Validation {
        reason: format!("{what} of {length} bytes exceeds the u8 length field"),
    })
}

pub fn u16_len(length: usize, what: &str) -> Result<u16, FrameError> {
    u16::try_from(length).map_err(|_| FrameError::Validation {
        reason: format!("{what} of {length} bytes exceeds the u16 length field"),
    })
}

pub trait ReadExt: io::Read {
    /// Reads a u8 value.
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buffer = [0; 1];
        self.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    /// Reads a little-endian u16 value.
    fn read_u16(&mut self) -> io::Result<u16> {
        let buffer = ReadExt::read_array(self)?;
        Ok(u16::from_le_bytes(buffer))
    }

    /// Reads a little-endian u32 value.
    fn read_u32(&mut self) -> io::Result<u32> {
        let buffer = ReadExt::read_array(self)?;
        Ok(u32::from_le_bytes(buffer))
    }

    /// Reads an array of bytes.
    fn read_array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buffer = [0; N];
        self.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

impl<T: io::Read + ?Sized> ReadExt for T {}

pub trait WriteExt: io::Write {
    /// Writes a u8 value.
    fn write_u8(&mut self, value: u8) -> io::Result<()> {
        self.write_all(&[value])
    }

    /// Writes a little-endian u16 value.
    fn write_u16(&mut self, value: u16) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    /// Writes a little-endian u32 value.
    fn write_u32(&mut self, value: u32) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    /// Writes an array of bytes.
    fn write_array<const N: usize>(&mut self, value: [u8; N]) -> io::Result<()> {
        self.write_all(&value)
    }
}

impl<T: io::Write + ?Sized> WriteExt for T {}

pub trait CursorExt<'a> {
    /// Reads the unread bytes from the current cursor position.
    fn read_remainder(&mut self) -> &'a [u8];

    /// Reads a byte slice with the given length from the current cursor position.
    fn read_slice(&mut self, length: usize) -> io::Result<&'a [u8]>;
}

impl<'a> CursorExt<'a> for io::Cursor<&'a [u8]> {
    fn read_remainder(&mut self) -> &'a [u8] {
        let start = self.position() as usize;
        let end = self.get_ref().len();
        self.set_position(end as u64);
        &self.get_ref()[start..]
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
