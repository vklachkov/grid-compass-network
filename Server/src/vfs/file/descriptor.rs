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

    fn date(value: u8) -> GRiDDate {
        GRiDDate::read_from_bytes(&[value; 11]).unwrap()
    }

    #[test]
    fn descriptor_is_exactly_198_bytes_and_round_trips_all_fields() {
        let descriptor = GRiDFileDescriptor {
            file_length: U32::new(0x0102_0304),
            file_name: GRiDFileName::new(b"Name~Data~").unwrap(),
            creation_date: date(8),
            dir_file_id: U16::new(0x1122),
            last_modified_date: date(9),
            expiration_date: date(10),
            machine_id: U32::new(11),
            compressed: 12,
            encrypted: 1,
            protected: 1,
            password: [3, b'K', b'E', b'Y', 0],
            dir_length: U32::new(14),
            dir_count: U16::new(0x3344),
            grid_write1: [15; 6],
            machine_id2: 1,
            uses_8087: 1,
            version1: 16,
            version2: 17,
            machine_id3: U32::new(18),
            grid_write2: [19; 11],
            version3: 20,
            property_length: U32::new(21),
            rom: 1,
            rom_id: U16::new(0x5566),
            mode: U16::new(0x7788),
            rainy_day_bytes: [22; 3],
            user_defined_bytes: [23; 20],
            grid_central_use: U16::new(0x99aa),
        };
        let bytes = descriptor.to_bytes();
        assert_eq!(bytes.len(), DESCRIPTOR_LENGTH);
        assert_eq!(GRiDFileDescriptor::from_bytes(&bytes).unwrap(), descriptor);
    }

    #[test]
    fn filename_length_over_80_is_a_format_error() {
        let descriptor = GRiDFileDescriptor {
            file_name: GRiDFileName::new(b"Name~Data~").unwrap(),
            ..Default::default()
        };
        let mut bytes = descriptor.to_bytes();
        bytes[4] = 81;
        assert!(GRiDFileDescriptor::from_bytes(&bytes).is_err());
    }

    #[test]
    fn default_descriptor_is_zeroed() {
        let name = GRiDFileName::new(b"Name~Data~").unwrap();
        let descriptor = GRiDFileDescriptor {
            file_name: name,
            ..Default::default()
        };

        assert_eq!(descriptor.file_name, name);
        assert_eq!(descriptor.creation_date, GRiDDate::never());
        assert_eq!(descriptor.last_modified_date, GRiDDate::never());
        assert_eq!(descriptor.expiration_date, GRiDDate::never());
        assert_eq!(descriptor.password, [0; 5]);
    }
}
