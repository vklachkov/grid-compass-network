use std::{fmt, io, ops::Deref};

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

pub const MAX_LENGTH: usize = 80;
pub const STORAGE_LENGTH: usize = MAX_LENGTH + 1;

mod sep {
    pub const KIND: u8 = b'~';
    pub const PASS: u8 = b'|';
    pub const PATH: u8 = b'`';
}

#[derive(
    Clone, Copy, Debug, FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, PartialEq, Eq,
)]
#[repr(C)]
pub struct GRiDFileName {
    length: u8,
    bytes: [u8; MAX_LENGTH],
}

const _: () = assert!(size_of::<GRiDFileName>() == STORAGE_LENGTH);

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
        for (slot, &byte) in bytes.iter_mut().zip(value) {
            *slot = byte;
        }

        Ok(Self {
            length: value.len() as u8,
            bytes,
        })
    }

    /// Names decoded straight from the wire skip the constructor, so the
    /// length prefix has to be checked before it is used as an index.
    pub fn validate(&self) -> Result<(), GRiDFileNameError> {
        if usize::from(self.length) > MAX_LENGTH {
            return Err(GRiDFileNameError::TooLong);
        }

        Self::is_valid_name(self.as_bytes())
    }

    pub fn is_valid_name(value: impl AsRef<[u8]>) -> Result<(), GRiDFileNameError> {
        let value = value.as_ref();

        if value.len() > MAX_LENGTH {
            return Err(GRiDFileNameError::TooLong);
        }

        let Some((&last, head)) = value.split_last() else {
            return Err(GRiDFileNameError::InvalidFormat);
        };

        if last != sep::KIND {
            return Err(GRiDFileNameError::InvalidFormat);
        }

        let mut has_separator = false;
        for &byte in head {
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
        self.bytes
            .get(..usize::from(self.length))
            .unwrap_or(&self.bytes)
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
            Self::TooLong => write!(fmt, "file name must be at most {MAX_LENGTH} bytes"),
            Self::InvalidFormat => fmt.write_str("file name must match title~kind~"),
            Self::ForbiddenCharacter(chr) => {
                write!(fmt, "file name contains forbidden character {chr:?}")
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

    fn decode(length: u8, value: &[u8]) -> GRiDFileName {
        let mut bytes = [0xaa; STORAGE_LENGTH];
        bytes[0] = length;
        bytes[1..1 + value.len()].copy_from_slice(value);
        GRiDFileName::read_from_bytes(&bytes).unwrap()
    }

    #[test]
    fn creates_and_dereferences_a_valid_name() {
        let name = GRiDFileName::new(b"Report~Text~").unwrap();

        assert_eq!(&*name, b"Report~Text~");
        assert_eq!(name.as_ref(), name.as_bytes());
    }

    #[test]
    fn the_length_prefix_bounds_the_name_within_its_storage() {
        let name = decode(12, b"Report~Text~");

        assert_eq!(&*name, b"Report~Text~");
    }

    #[test]
    fn validate_rejects_a_decoded_name_the_constructor_would_refuse() {
        assert_eq!(decode(81, b"").validate(), Err(GRiDFileNameError::TooLong));
        assert_eq!(
            decode(5, b"title").validate(),
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
}
