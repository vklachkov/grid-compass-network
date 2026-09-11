mod date;
mod descriptor;
mod name;
mod path;

pub use date::GRiDDate;
pub use descriptor::GRiDFileDescriptor;
pub use name::{GRiDFileName, GRiDFileNameError};
pub use path::{GRiDPath, GRiDPathComponents};

use std::{
    fs::{File, Metadata},
    io::{self, Read, Seek, SeekFrom, Write},
    mem::size_of,
};

use thiserror::Error;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned,
    byteorder::{LE, U32},
};

const HEADER_MAGIC: [u8; 7] = *b"GRiDiRG";
const HEADER_FORMAT_VERSION: u8 = 1;
const HEADER_LENGTH: usize = 16;

pub type Result<T> = std::result::Result<T, GRiDFileError>;

#[derive(Debug, Error)]
pub enum GRiDFileError {
    #[error("I/O: {0}")]
    Io(#[from] io::Error),
    #[error("invalid header magic")]
    InvalidHeaderMagic,
    #[error("unsupported header version {0}")]
    UnsupportedHeaderVersion(u8),
    #[error("data too short: {actual} < {minimum}")]
    DataTooShort {
        actual: u64,
        minimum: u64,
    },
    #[error("data too large")]
    DataTooLarge,
}

impl From<GRiDFileError> for io::Error {
    fn from(error: GRiDFileError) -> Self {
        match error {
            GRiDFileError::Io(error) => error,
            error @ (GRiDFileError::InvalidHeaderMagic
            | GRiDFileError::UnsupportedHeaderVersion(_)
            | GRiDFileError::DataTooShort { .. }) => Self::new(io::ErrorKind::InvalidData, error),
            error => Self::new(io::ErrorKind::InvalidInput, error),
        }
    }
}

impl From<GRiDFileError> for super::Error {
    fn from(error: GRiDFileError) -> Self {
        Self::Io(error.into())
    }
}

#[derive(Debug)]
pub struct GRiDFile {
    file: File,
    name: GRiDFileName,
    header: GRiDFileHeader,
    body_pos: u64,
}

#[derive(Clone, Debug, FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, PartialEq, Eq)]
#[repr(C)]
pub struct GRiDFileHeader {
    pub magic: [u8; 7],
    pub format_version: u8,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub flags: u8,
    pub property_length: U32<LE>,
}

const _: () = assert!(size_of::<GRiDFileHeader>() == HEADER_LENGTH);

impl GRiDFileHeader {
    pub fn new() -> Self {
        Self {
            magic: HEADER_MAGIC,
            format_version: HEADER_FORMAT_VERSION,
            version_major: 0,
            version_minor: 0,
            version_patch: 0,
            flags: 0,
            property_length: U32::ZERO,
        }
    }

    fn from_bytes(bytes: &[u8; HEADER_LENGTH]) -> Result<Self> {
        let header = Self::read_from_bytes(bytes).expect("header size is fixed");
        header.validate()?;
        Ok(header)
    }

    fn to_bytes(&self) -> Result<&[u8]> {
        self.validate()?;
        Ok(self.as_bytes())
    }

    fn validate(&self) -> Result<()> {
        if self.magic != HEADER_MAGIC {
            return Err(GRiDFileError::InvalidHeaderMagic);
        }

        if self.format_version != HEADER_FORMAT_VERSION {
            return Err(GRiDFileError::UnsupportedHeaderVersion(self.format_version));
        }

        Ok(())
    }
}

impl GRiDFile {
    /// Opens a GRiD file and positions it at the start of its logical body.
    pub fn from_file(mut file: File, name: GRiDFileName) -> Result<Self> {
        let mut header_bytes = [0; HEADER_LENGTH];
        file.read_exact(&mut header_bytes)?;
        let header = GRiDFileHeader::from_bytes(&header_bytes)?;

        let physical_length = file.metadata()?.len();
        let minimum_length = HEADER_LENGTH as u64 + u64::from(header.property_length.get());
        if physical_length < minimum_length {
            return Err(GRiDFileError::DataTooShort {
                actual: physical_length,
                minimum: minimum_length,
            });
        }
        if physical_length - minimum_length > u64::from(u32::MAX) {
            return Err(GRiDFileError::DataTooLarge);
        }

        let mut file = Self {
            file,
            name,
            header,
            body_pos: 0,
        };
        file.seek_body(0)?;
        Ok(file)
    }

    /// Creates a GRiD file from a header and its complete body, including properties.
    pub fn create(
        mut file: File,
        name: GRiDFileName,
        header: GRiDFileHeader,
        body: &[u8],
    ) -> Result<Self> {
        Self::validate_body_length(&header, body.len() as u64)?;
        Self::write_layout(&mut file, &header, body)?;

        let mut file = Self {
            file,
            name,
            header,
            body_pos: 0,
        };
        file.seek_body(0)?;
        Ok(file)
    }

    /// Returns the parsed header currently stored by the file.
    pub fn header(&self) -> &GRiDFileHeader {
        &self.header
    }

    /// Returns the GRiD name associated with the file.
    pub fn name(&self) -> GRiDFileName {
        self.name
    }

    /// Returns metadata for the underlying physical file.
    pub fn metadata(&self) -> Result<Metadata> {
        Ok(self.file.metadata()?)
    }

    /// Returns the current position relative to the start of the logical body.
    pub fn position(&self) -> u64 {
        self.body_pos
    }

    pub fn truncate(&mut self) -> Result<()> {
        Self::validate_body_length(&self.header, self.body_pos)?;
        Ok(self.file.set_len(self.body_offset(self.body_pos))?)
    }

    /// Replaces the header without changing the body or its logical position.
    pub fn set_header(&mut self, new_header: GRiDFileHeader) -> Result<()> {
        Self::validate_body_length(&new_header, self.body_length()?)?;
        new_header.to_bytes()?;
        self.header = new_header;
        self.sync_header()
    }

    /// Writes the header followed by the complete body, including properties.
    fn write_layout(file: &mut File, header: &GRiDFileHeader, body: &[u8]) -> Result<()> {
        file.set_len(HEADER_LENGTH as u64 + body.len() as u64)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(header.to_bytes()?)?;
        file.write_all(body)?;
        Ok(())
    }

    /// Writes the in-memory header without changing the body or its logical position.
    fn sync_header(&mut self) -> Result<()> {
        let physical_position = self.body_offset(self.body_pos);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(self.header.to_bytes()?)?;
        self.file.seek(SeekFrom::Start(physical_position))?;
        Ok(())
    }

    fn validate_body_length(header: &GRiDFileHeader, body_length: u64) -> Result<()> {
        let property_length = u64::from(header.property_length.get());
        if body_length < property_length {
            return Err(GRiDFileError::DataTooShort {
                actual: body_length,
                minimum: property_length,
            });
        }
        if body_length - property_length > u64::from(u32::MAX) {
            return Err(GRiDFileError::DataTooLarge);
        }
        Ok(())
    }

    /// Returns the length of the logical body, including properties.
    pub fn body_length(&self) -> io::Result<u64> {
        self.file
            .metadata()?
            .len()
            .checked_sub(HEADER_LENGTH as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated header"))
    }

    /// Applies a signed seek offset and rejects positions before the logical body.
    fn seek_target(start: u64, offset: i64) -> Option<u64> {
        if offset >= 0 {
            start.checked_add(offset as u64)
        } else {
            start.checked_sub(offset.unsigned_abs())
        }
    }

    /// Moves the physical cursor to a position relative to the logical body.
    fn seek_body(&mut self, position: u64) -> io::Result<u64> {
        self.file.seek(SeekFrom::Start(self.body_offset(position)))
    }

    fn body_offset(&self, position: u64) -> u64 {
        HEADER_LENGTH as u64 + position
    }
}

impl Read for GRiDFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let body_length = self.body_length()?;
        if self.body_pos >= body_length || buffer.is_empty() {
            return Ok(0);
        }

        let available = usize::try_from(body_length - self.body_pos).unwrap_or(usize::MAX);
        let read_length = buffer.len().min(available);
        self.seek_body(self.body_pos)?;
        let count = self.file.read(&mut buffer[..read_length])?;
        self.body_pos += count as u64;
        Ok(count)
    }
}

impl Write for GRiDFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        let end = self
            .body_pos
            .checked_add(buffer.len() as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "length overflow"))?;
        Self::validate_body_length(&self.header, end.max(self.body_length()?))?;

        self.seek_body(self.body_pos)?;
        let count = self.file.write(buffer)?;
        self.body_pos += count as u64;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for GRiDFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let target = match position {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(offset) => Self::seek_target(self.body_pos, offset),
            SeekFrom::End(offset) => Self::seek_target(self.body_length()?, offset),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;

        self.seek_body(target)?;
        self.body_pos = target;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use tempfile::tempfile;

    fn read_physical_file(file: &mut File) -> Vec<u8> {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        bytes
    }

    fn name() -> GRiDFileName {
        GRiDFileName::new(b"test~Data~").unwrap()
    }

    fn header(property_length: u32) -> GRiDFileHeader {
        let mut header = GRiDFileHeader::new();
        header.property_length = U32::new(property_length);
        header
    }

    #[test]
    fn header_is_exactly_16_bytes_and_round_trips_all_fields() {
        let mut header = header(0x0102_0304);
        header.version_major = 1;
        header.version_minor = 2;
        header.version_patch = 3;
        header.flags = 0x12;

        let bytes = header.to_bytes().unwrap();

        assert_eq!(HEADER_LENGTH, 16);
        assert_eq!(&bytes[..8], b"GRiDiRG\x01");
        assert_eq!(
            GRiDFileHeader::from_bytes(bytes.try_into().unwrap()).unwrap(),
            header
        );
    }

    #[test]
    fn open_rejects_invalid_magic_and_format_version() {
        for offset in [0, HEADER_MAGIC.len()] {
            let mut file = tempfile().unwrap();
            let mut bytes = header(0).to_bytes().unwrap().to_vec();
            bytes[offset] ^= 0xff;
            file.write_all(&bytes).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();

            assert!(matches!(
                GRiDFile::from_file(file, name()).unwrap_err(),
                GRiDFileError::InvalidHeaderMagic | GRiDFileError::UnsupportedHeaderVersion(_)
            ));
        }
    }

    #[test]
    fn create_writes_header_and_complete_body() {
        let mut physical_file = tempfile().unwrap();
        let header = header(3);
        let file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            name(),
            header.clone(),
            b"propbody",
        )
        .unwrap();

        assert_eq!(file.header(), &header);
        assert_eq!(file.name(), name());
        assert_eq!(file.position(), 0);
        assert_eq!(physical_file.metadata().unwrap().len(), 24);
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        assert_eq!(&bytes[..HEADER_LENGTH], header.to_bytes().unwrap());
        assert_eq!(&bytes[HEADER_LENGTH..], b"propbody");
    }

    #[test]
    fn open_rejects_a_physically_short_property_section() {
        let mut file = tempfile().unwrap();
        file.write_all(&header(5).to_bytes().unwrap()).unwrap();
        file.write_all(b"abc").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        assert!(matches!(
            GRiDFile::from_file(file, name()).unwrap_err(),
            GRiDFileError::DataTooShort { .. }
        ));
    }

    #[test]
    fn open_rejects_a_truncated_header() {
        let mut file = tempfile().unwrap();
        file.write_all(&[0; 10]).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        assert!(matches!(
            GRiDFile::from_file(file, name()).unwrap_err(),
            GRiDFileError::Io(error) if error.kind() == io::ErrorKind::UnexpectedEof
        ));
    }

    #[test]
    fn create_rejects_a_body_shorter_than_properties() {
        let error = GRiDFile::create(tempfile().unwrap(), name(), header(3), b"ab").unwrap_err();
        assert!(matches!(error, GRiDFileError::DataTooShort { .. }));
    }

    #[test]
    fn properties_are_the_start_of_the_logical_body() {
        let mut physical_file = tempfile().unwrap();
        let mut file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            name(),
            header(4),
            b"metaabc",
        )
        .unwrap();

        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert_eq!(body, b"metaabc");

        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"META").unwrap();
        let bytes = read_physical_file(&mut physical_file);
        assert_eq!(&bytes[HEADER_LENGTH..HEADER_LENGTH + 4], b"META");
    }

    #[test]
    fn body_write_grows_the_physical_file_without_changing_the_header() {
        let mut physical_file = tempfile().unwrap();
        let header = header(2);
        let mut file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            name(),
            header.clone(),
            b"xy\0",
        )
        .unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(b"new").unwrap();
        assert_eq!(file.header(), &header);
        assert_eq!(file.position(), 6);
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        let stored_header =
            GRiDFileHeader::from_bytes(bytes[..HEADER_LENGTH].try_into().unwrap()).unwrap();
        assert_eq!(stored_header, header);
        assert_eq!(&bytes[HEADER_LENGTH..HEADER_LENGTH + 2], b"xy");
        assert_eq!(physical_file.metadata().unwrap().len(), 22);
    }

    #[test]
    fn header_update_may_redistribute_the_body_length() {
        let mut physical_file = tempfile().unwrap();
        let mut file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            name(),
            header(3),
            b"content",
        )
        .unwrap();

        let mut replacement = file.header().clone();
        replacement.property_length = U32::new(5);
        replacement.flags = 42;
        file.set_header(replacement.clone()).unwrap();
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        let stored_header =
            GRiDFileHeader::from_bytes(bytes[..HEADER_LENGTH].try_into().unwrap()).unwrap();
        assert_eq!(stored_header, replacement);
        assert_eq!(&bytes[HEADER_LENGTH..], b"content");
    }

    #[test]
    fn header_update_rejects_properties_longer_than_the_body() {
        let mut file = GRiDFile::create(tempfile().unwrap(), name(), header(5), &[0; 11]).unwrap();
        file.seek(SeekFrom::Start(5)).unwrap();

        let original = file.header().clone();
        let mut replacement = original.clone();
        replacement.property_length = U32::new(12);
        let error = file.set_header(replacement).unwrap_err();

        assert!(matches!(error, GRiDFileError::DataTooShort { .. }));
        assert_eq!(file.position(), 5);
        assert_eq!(file.header(), &original);
    }

    #[test]
    fn truncate_cannot_remove_properties() {
        let mut file = GRiDFile::create(tempfile().unwrap(), name(), header(2), b"xybody").unwrap();
        file.seek(SeekFrom::Start(1)).unwrap();
        assert!(matches!(
            file.truncate().unwrap_err(),
            GRiDFileError::DataTooShort { .. }
        ));

        file.seek(SeekFrom::Start(4)).unwrap();
        file.truncate().unwrap();
        assert_eq!(file.metadata().unwrap().len(), HEADER_LENGTH as u64 + 4);
    }

    #[test]
    fn seek_is_relative_to_the_logical_body() {
        let mut file = GRiDFile::create(tempfile().unwrap(), name(), header(2), &[0; 4]).unwrap();
        assert_eq!(file.seek(SeekFrom::Start(1)).unwrap(), 1);
        assert_eq!(file.seek(SeekFrom::Current(1)).unwrap(), 2);
        assert_eq!(file.seek(SeekFrom::End(-1)).unwrap(), 3);
        assert_eq!(file.seek(SeekFrom::End(4)).unwrap(), 8);
        assert!(file.seek(SeekFrom::Current(-9)).is_err());
        assert_eq!(file.position(), 8);
    }

    #[test]
    fn empty_properties_and_body_are_supported() {
        let physical_file = tempfile().unwrap();
        let mut file =
            GRiDFile::create(physical_file.try_clone().unwrap(), name(), header(0), &[]).unwrap();
        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
        assert_eq!(
            physical_file.metadata().unwrap().len(),
            HEADER_LENGTH as u64
        );
    }
}
