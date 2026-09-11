use anyhow::bail;
use zerocopy::{
    FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout, Unaligned,
    byteorder::{LE, U16, U32},
};

use super::{GRiDDate, GRiDFileName};

pub const DESCRIPTOR_LENGTH: usize = 198;

/// The flag fields are `u8` rather than `bool` because the wire carries
/// whatever the client sent, and only 0 and 1 are valid `bool` bit patterns.
#[derive(Clone, Debug, FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, PartialEq, Eq)]
#[repr(C)]
pub struct GRiDFileDescriptor {
    pub file_length: U32<LE>,
    pub file_name: GRiDFileName,
    pub creation_date: GRiDDate,
    pub dir_file_id: U16<LE>,
    pub last_modified_date: GRiDDate,
    pub expiration_date: GRiDDate,
    pub machine_id: U32<LE>,
    pub compressed: u8,
    pub encrypted: u8,
    pub protected: u8,
    pub password: [u8; 5],
    pub dir_length: U32<LE>,
    pub dir_count: U16<LE>,
    pub grid_write1: [u8; 6],
    pub machine_id2: u8,
    pub uses_8087: u8,
    pub version1: u8,
    pub version2: u8,
    pub machine_id3: U32<LE>,
    pub grid_write2: [u8; 11],
    pub version3: u8,
    pub property_length: U32<LE>,
    pub rom: u8,
    pub rom_id: U16<LE>,
    pub mode: U16<LE>,
    pub rainy_day_bytes: [u8; 3],
    pub user_defined_bytes: [u8; 20],
    pub grid_central_use: U16<LE>,
}

const _: () = assert!(size_of::<GRiDFileDescriptor>() == DESCRIPTOR_LENGTH);

impl GRiDFileDescriptor {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let Ok(descriptor) = Self::read_from_bytes(bytes) else {
            bail!(
                "descriptor must be exactly {DESCRIPTOR_LENGTH} bytes, got {}",
                bytes.len()
            );
        };

        descriptor.file_name.validate()?;

        Ok(descriptor)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl Default for GRiDFileDescriptor {
    fn default() -> Self {
        Self::new_zeroed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded() -> Vec<u8> {
        GRiDFileDescriptor {
            file_length: U32::new(0x0102_0304),
            file_name: GRiDFileName::new(b"Name~Data~").unwrap(),
            ..Default::default()
        }
        .to_bytes()
    }

    #[test]
    fn the_name_follows_the_file_length_on_the_wire() {
        let bytes = encoded();

        assert_eq!(&bytes[..4], 0x0102_0304u32.to_le_bytes());
        assert_eq!(bytes[4], 10);
        assert_eq!(&bytes[5..15], b"Name~Data~");
    }

    #[test]
    fn a_name_length_over_the_maximum_is_rejected() {
        let mut bytes = encoded();
        bytes[4] = 81;

        assert!(GRiDFileDescriptor::from_bytes(&bytes).is_err());
    }
}
