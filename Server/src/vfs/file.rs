use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

use super::file_desc::{DESCRIPTOR_LENGTH, GRiDFileDescriptor, MAX_FILE_NAME_LENGTH};

pub struct GRiDFile {
    file: File,
    descriptor: GRiDFileDescriptor,
    body_pos: u64,
}

impl GRiDFile {
    /// Opens a GRiD file and positions it at the start of its logical body.
    /// Physical bytes after the length declared by the descriptor are ignored.
    pub fn open(mut file: File) -> io::Result<Self> {
        let mut descriptor_bytes = [0; DESCRIPTOR_LENGTH];
        file.read_exact(&mut descriptor_bytes)?;

        let descriptor = GRiDFileDescriptor::from_bytes(&descriptor_bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        let physical_length = file.metadata()?.len();
        let declared_length = Self::file_total_length(&descriptor);

        if physical_length < declared_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "physical file is shorter than declared size: {physical_length} < {declared_length}"
                ),
            ));
        }

        let mut file = Self {
            file,
            descriptor,
            body_pos: 0,
        };
        file.seek_body(0)?;
        Ok(file)
    }

    /// Creates a GRiD file from a descriptor and its property stream.
    /// The body is allocated to `file_length` and starts with zero-filled bytes.
    pub fn create(
        mut file: File,
        descriptor: GRiDFileDescriptor,
        properties: &[u8],
    ) -> io::Result<Self> {
        if descriptor.file_name_length as usize > MAX_FILE_NAME_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fileNameLength must be at most {MAX_FILE_NAME_LENGTH}, got {}",
                    descriptor.file_name_length
                ),
            ));
        }

        if descriptor.property_length != properties.len() as u32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "propertyLength is {}, but properties contain {} bytes",
                    descriptor.property_length,
                    properties.len()
                ),
            ));
        }

        Self::write_layout(&mut file, &descriptor, properties, &[])?;

        let mut file = Self {
            file,
            descriptor,
            body_pos: 0,
        };
        file.seek_body(0)?;
        Ok(file)
    }

    /// Returns the parsed descriptor currently stored by the file.
    pub fn descriptor(&self) -> &GRiDFileDescriptor {
        &self.descriptor
    }

    /// Returns the current position relative to the start of the logical body.
    pub fn position(&self) -> u64 {
        self.body_pos
    }

    /// Replaces the descriptor while preserving the properties and body.
    /// Shrinking a section truncates its tail; growing it appends zero-filled bytes.
    /// The logical body position is clamped when the new body becomes shorter.
    pub fn set_descriptor(&mut self, new_descriptor: GRiDFileDescriptor) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(DESCRIPTOR_LENGTH as u64))?;

        let mut properties = vec![0; self.descriptor.property_length as usize];
        self.file.read_exact(&mut properties)?;
        properties.resize(new_descriptor.property_length as usize, 0);

        let mut body = vec![0; self.descriptor.file_length as usize];
        self.file.read_exact(&mut body)?;
        body.resize(new_descriptor.file_length as usize, 0);

        Self::write_layout(&mut self.file, &new_descriptor, &properties, &body)?;

        self.descriptor = new_descriptor;
        self.body_pos = self.body_pos.min(u64::from(self.descriptor.file_length));

        self.seek_body(self.body_pos)?;
        Ok(())
    }

    /// Rewrites the physical descriptor-properties-body layout at the exact declared size.
    fn write_layout(
        file: &mut File,
        descriptor: &GRiDFileDescriptor,
        properties: &[u8],
        body: &[u8],
    ) -> io::Result<()> {
        file.set_len(Self::file_total_length(descriptor))?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&descriptor.to_bytes())?;
        file.write_all(properties)?;
        file.write_all(body)?;
        Ok(())
    }

    /// Writes the in-memory descriptor without changing properties, body, or body position.
    fn sync_descriptor(&mut self) -> io::Result<()> {
        let physical_position = self.body_offset(self.body_pos);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&self.descriptor.to_bytes())?;
        self.file.seek(SeekFrom::Start(physical_position))?;
        Ok(())
    }

    /// Applies a signed seek offset and rejects positions before the logical body.
    fn seek_target(start: u64, offset: i64) -> Option<u64> {
        if offset >= 0 {
            Some(start + offset as u64)
        } else {
            start.checked_sub(offset.unsigned_abs())
        }
    }

    /// Moves the physical cursor to a position relative to the logical body.
    fn seek_body(&mut self, position: u64) -> io::Result<u64> {
        self.file.seek(SeekFrom::Start(self.body_offset(position)))
    }

    /// Returns the physical size declared by the descriptor.
    fn file_total_length(descriptor: &GRiDFileDescriptor) -> u64 {
        DESCRIPTOR_LENGTH as u64
            + u64::from(descriptor.property_length)
            + u64::from(descriptor.file_length)
    }

    /// Translates a logical body position into a physical file offset.
    fn body_offset(&self, position: u64) -> u64 {
        DESCRIPTOR_LENGTH as u64 + u64::from(self.descriptor.property_length) + position
    }
}

impl Read for GRiDFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let file_length = u64::from(self.descriptor.file_length);
        if self.body_pos >= file_length || buffer.is_empty() {
            return Ok(0);
        }

        let available = (file_length - self.body_pos) as usize;
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

        self.seek_body(self.body_pos)?;
        let count = self.file.write(buffer)?;
        self.body_pos += count as u64;
        if self.body_pos > u64::from(self.descriptor.file_length) {
            self.descriptor.file_length = self.body_pos as u32;
            self.sync_descriptor()?;
        }

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
            SeekFrom::End(offset) => {
                Self::seek_target(u64::from(self.descriptor.file_length), offset)
            }
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "logical seek before zero"))?;

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

    fn descriptor(file_length: u32, property_length: u32) -> GRiDFileDescriptor {
        GRiDFileDescriptor {
            file_length,
            file_name_length: 4,
            file_name: {
                let mut value = [0; 80];
                value[..4].copy_from_slice(b"test");
                value
            },
            property_length,
            ..GRiDFileDescriptor::default()
        }
    }

    #[test]
    fn create_writes_descriptor_properties_and_declared_body_size() {
        let mut physical_file = tempfile().unwrap();
        let descriptor = descriptor(5, 3);
        let file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            descriptor.clone(),
            b"abc",
        )
        .unwrap();
        assert_eq!(file.descriptor(), &descriptor);
        assert_eq!(file.position(), 0);
        assert_eq!(physical_file.metadata().unwrap().len(), 206);
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        assert_eq!(&bytes[..DESCRIPTOR_LENGTH], descriptor.to_bytes());
        assert_eq!(&bytes[DESCRIPTOR_LENGTH..DESCRIPTOR_LENGTH + 3], b"abc");
    }

    #[test]
    fn open_rejects_a_physically_short_declared_file() {
        let mut file = tempfile().unwrap();
        let descriptor = descriptor(5, 3);
        file.write_all(&descriptor.to_bytes()).unwrap();
        file.write_all(b"abc").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let error = GRiDFile::open(file).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn open_rejects_a_truncated_descriptor() {
        let mut file = tempfile().unwrap();
        file.write_all(&[0; 10]).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let error = GRiDFile::open(file).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn open_allows_trailing_physical_bytes_but_reads_only_declared_body() {
        let mut physical_file = tempfile().unwrap();
        let descriptor = descriptor(3, 0);
        physical_file.write_all(&descriptor.to_bytes()).unwrap();
        physical_file.write_all(b"abcTRAILING").unwrap();
        physical_file.seek(SeekFrom::Start(0)).unwrap();

        let mut file = GRiDFile::open(physical_file).unwrap();
        assert_eq!(file.position(), 0);

        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert_eq!(body, b"abc");
    }

    #[test]
    fn create_rejects_invalid_descriptor_lengths() {
        let mut invalid_name = descriptor(0, 0);
        invalid_name.file_name_length = (MAX_FILE_NAME_LENGTH + 1) as u8;
        let error = GRiDFile::create(tempfile().unwrap(), invalid_name, &[])
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let invalid_properties = descriptor(0, 1);
        let error = GRiDFile::create(tempfile().unwrap(), invalid_properties, &[])
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn descriptor_access_does_not_change_body_position() {
        let descriptor = descriptor(3, 0);
        let mut file = GRiDFile::create(tempfile().unwrap(), descriptor.clone(), &[]).unwrap();
        file.seek(SeekFrom::Start(1)).unwrap();

        assert_eq!(file.descriptor(), &descriptor);
        assert_eq!(file.position(), 1);
    }

    #[test]
    fn properties_are_not_part_of_the_body() {
        let mut physical_file = tempfile().unwrap();
        let descriptor = descriptor(3, 4);
        let mut file =
            GRiDFile::create(physical_file.try_clone().unwrap(), descriptor, b"meta").unwrap();
        file.write_all(b"abc").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert_eq!(body, b"abc");
        let bytes = read_physical_file(&mut physical_file);
        assert_eq!(&bytes[DESCRIPTOR_LENGTH..DESCRIPTOR_LENGTH + 4], b"meta");
    }

    #[test]
    fn body_read_write_and_seek_are_logical() {
        let descriptor = descriptor(3, 2);
        let mut file = GRiDFile::create(tempfile().unwrap(), descriptor, b"xy").unwrap();
        let mut body = [0; 3];
        file.read_exact(&mut body).unwrap();
        assert_eq!(&body, b"\0\0\0");
        file.seek(SeekFrom::Start(1)).unwrap();
        file.write_all(b"Q").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_exact(&mut body).unwrap();
        assert_eq!(&body, b"\0Q\0");
    }

    #[test]
    fn body_write_grows_length_without_changing_properties() {
        let mut physical_file = tempfile().unwrap();
        let descriptor = descriptor(1, 2);
        let mut file =
            GRiDFile::create(physical_file.try_clone().unwrap(), descriptor, b"xy").unwrap();
        file.seek(SeekFrom::Start(3)).unwrap();
        file.write_all(b"new").unwrap();
        assert_eq!(file.descriptor().file_length, 6);
        assert_eq!(file.position(), 6);
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        let stored_descriptor =
            GRiDFileDescriptor::from_bytes(&bytes[..DESCRIPTOR_LENGTH]).unwrap();
        assert_eq!(stored_descriptor.file_length, 6);
        assert_eq!(&bytes[DESCRIPTOR_LENGTH..DESCRIPTOR_LENGTH + 2], b"xy");
        assert_eq!(physical_file.metadata().unwrap().len(), 206);
    }

    #[test]
    fn descriptor_update_resizes_properties_and_body() {
        let mut physical_file = tempfile().unwrap();
        let mut file =
            GRiDFile::create(physical_file.try_clone().unwrap(), descriptor(4, 3), b"abc").unwrap();
        file.write_all(b"body").unwrap();

        let mut replacement = file.descriptor().clone();
        replacement.file_length = 6;
        replacement.property_length = 5;
        replacement.mode = 42;
        file.set_descriptor(replacement.clone()).unwrap();
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        let stored_descriptor =
            GRiDFileDescriptor::from_bytes(&bytes[..DESCRIPTOR_LENGTH]).unwrap();
        assert_eq!(stored_descriptor, replacement);
        assert_eq!(physical_file.metadata().unwrap().len(), 209);
        assert_eq!(&bytes[DESCRIPTOR_LENGTH..DESCRIPTOR_LENGTH + 5], b"abc\0\0");
        assert_eq!(&bytes[DESCRIPTOR_LENGTH + 5..], b"body\0\0");
    }

    #[test]
    fn descriptor_update_truncates_sections_and_clamps_position() {
        let mut physical_file = tempfile().unwrap();
        let mut file = GRiDFile::create(
            physical_file.try_clone().unwrap(),
            descriptor(6, 5),
            b"props",
        )
        .unwrap();
        file.write_all(b"body!!").unwrap();
        file.seek(SeekFrom::Start(5)).unwrap();

        let mut replacement = file.descriptor().clone();
        replacement.file_length = 3;
        replacement.property_length = 2;
        file.set_descriptor(replacement.clone()).unwrap();

        assert_eq!(file.position(), 3);
        assert_eq!(file.descriptor(), &replacement);
        drop(file);

        let bytes = read_physical_file(&mut physical_file);
        assert_eq!(bytes.len(), DESCRIPTOR_LENGTH + 2 + 3);
        assert_eq!(&bytes[DESCRIPTOR_LENGTH..DESCRIPTOR_LENGTH + 2], b"pr");
        assert_eq!(&bytes[DESCRIPTOR_LENGTH + 2..], b"bod");
    }

    #[test]
    fn seek_is_relative_to_the_logical_body() {
        let mut file = GRiDFile::create(tempfile().unwrap(), descriptor(2, 0), &[]).unwrap();
        assert_eq!(file.seek(SeekFrom::Start(1)).unwrap(), 1);
        assert_eq!(file.seek(SeekFrom::Current(1)).unwrap(), 2);
        assert_eq!(file.seek(SeekFrom::End(-1)).unwrap(), 1);
        assert_eq!(file.seek(SeekFrom::End(4)).unwrap(), 6);
        assert!(file.seek(SeekFrom::Current(-7)).is_err());
        assert_eq!(file.position(), 6);
    }

    #[test]
    fn empty_properties_and_body_are_supported() {
        let physical_file = tempfile().unwrap();
        let mut file =
            GRiDFile::create(physical_file.try_clone().unwrap(), descriptor(0, 0), &[]).unwrap();
        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
        assert_eq!(
            physical_file.metadata().unwrap().len(),
            DESCRIPTOR_LENGTH as u64
        );
    }
}
