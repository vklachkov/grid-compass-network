use std::{fmt, io, ops::Deref};

pub const MAX_LENGTH: usize = 80;

mod sep {
    pub const KIND: u8 = b'~';
    pub const PASS: u8 = b'|';
    pub const PATH: u8 = b'`';
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GRiDFileName {
    length: u8,
    bytes: [u8; MAX_LENGTH],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GRiDFileNameError {
    TooLong,
    InvalidFormat,
    ForbiddenCharacter(char),
}

impl GRiDFileName {
    pub fn new(value: impl AsRef<[u8]>) -> Result<Self, GRiDFileNameError> {
        let value = value.as_ref();
        Self::is_valid_name(value)?;

        let mut bytes = [0; MAX_LENGTH];
        bytes[..value.len()].copy_from_slice(value);

        Ok(Self {
            length: value.len() as u8,
            bytes,
        })
    }

    pub fn from_bytes(length: u8, bytes: [u8; MAX_LENGTH]) -> Result<Self, GRiDFileNameError> {
        if usize::from(length) > MAX_LENGTH {
            return Err(GRiDFileNameError::TooLong);
        }
    
        Self::is_valid_name(&bytes)?;

        Ok(Self {
            length,
            bytes,
        })
    }

    pub fn is_valid_name(value: impl AsRef<[u8]>) -> Result<(), GRiDFileNameError> {
        let value = value.as_ref();

        if value.is_empty() {
            return Err(GRiDFileNameError::InvalidFormat);
        }
        if value.len() > MAX_LENGTH {
            return Err(GRiDFileNameError::TooLong);
        }

        if value[value.len() - 1] != sep::KIND {
            return Err(GRiDFileNameError::InvalidFormat);
        }

        let mut has_separator = false;
        for &byte in &value[..value.len() - 1] {
            match byte {
                sep::KIND if !has_separator => {
                    has_separator = true;
                }
                sep::KIND | sep::PASS | sep::PATH => {
                    return Err(GRiDFileNameError::ForbiddenCharacter(char::from(byte)));
                }
                _ => {}
            }
        }

        if !has_separator {
            return Err(GRiDFileNameError::InvalidFormat);
        }

        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }

    pub const fn len(&self) -> u8 {
        self.length
    }

    pub const fn storage(&self) -> &[u8; MAX_LENGTH] {
        &self.bytes
    }
}

impl Deref for GRiDFileName {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for GRiDFileName {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Display for GRiDFileNameError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => {
                write!(fmt, "file name must be at most {MAX_LENGTH} bytes") //
            }
            Self::InvalidFormat => {
                fmt.write_str("file name must match title~kind~") //
            }
            Self::ForbiddenCharacter(chr) => {
                write!(fmt, "file name contains forbidden character {chr:?}") //
            }
        }
    }
}

impl std::error::Error for GRiDFileNameError {}

impl From<GRiDFileNameError> for io::Error {
    fn from(error: GRiDFileNameError) -> Self {
        Self::new(io::ErrorKind::InvalidInput, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_dereferences_a_valid_name() {
        let name = GRiDFileName::new(b"Report~Text~").unwrap();

        assert_eq!(&*name, b"Report~Text~");
        assert_eq!(name.as_ref(), name.as_bytes());
    }

    #[test]
    fn decodes_length_prefixed_storage() {
        let mut bytes = [0xaa; MAX_LENGTH + 1];
        bytes[0] = 12;
        bytes[1..13].copy_from_slice(b"Report~Text~");

        let name = GRiDFileName::from_bytes(bytes).unwrap();

        assert_eq!(&*name, b"Report~Text~");
        assert_eq!(&name.storage()[12..], &[0xaa; 68]);
    }

    #[test]
    fn rejects_an_invalid_length_prefixed_name() {
        let mut too_long = [0; MAX_LENGTH + 1];
        too_long[0] = 81;
        assert_eq!(
            GRiDFileName::from_bytes(too_long),
            Err(GRiDFileNameError::TooLong)
        );

        let mut malformed = [0; MAX_LENGTH + 1];
        malformed[0] = 5;
        malformed[1..6].copy_from_slice(b"title");
        assert_eq!(
            GRiDFileName::from_bytes(malformed),
            Err(GRiDFileNameError::InvalidFormat)
        );
    }

    #[test]
    fn accepts_a_name_of_exactly_eighty_bytes() {
        let mut value = vec![b'a'; 78];
        value.extend_from_slice(b"~~");

        let name = GRiDFileName::new(&value).unwrap();

        assert_eq!(name.as_bytes(), value.as_slice());
    }

    #[test]
    fn rejects_an_overlong_name() {
        let mut value = vec![b'a'; 79];
        value.extend_from_slice(b"~~");

        assert_eq!(GRiDFileName::new(value), Err(GRiDFileNameError::TooLong));
    }

    #[test]
    fn rejects_names_outside_the_title_kind_template() {
        for value in [b"title".as_slice(), b"title~kind".as_slice()] {
            assert_eq!(
                GRiDFileName::new(value),
                Err(GRiDFileNameError::InvalidFormat)
            );
        }
    }

    #[test]
    fn rejects_forbidden_component_characters() {
        for (value, chr) in [
            (b"ti|tle~kind~".as_slice(), '|'),
            (b"title~ki`nd~".as_slice(), '`'),
            (b"title~~kind~".as_slice(), '~'),
        ] {
            assert_eq!(
                GRiDFileName::new(value),
                Err(GRiDFileNameError::ForbiddenCharacter(chr))
            );
        }
    }

    #[test]
    fn validates_without_constructing_a_name() {
        assert_eq!(GRiDFileName::is_valid_name(b"Report~Text~"), Ok(()));
        assert_eq!(
            GRiDFileName::is_valid_name(b"Report|Draft~Text~"),
            Err(GRiDFileNameError::ForbiddenCharacter('|'))
        );
    }

    #[test]
    fn converts_its_error_to_an_io_error() {
        let error: io::Error = GRiDFileNameError::InvalidFormat.into();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
