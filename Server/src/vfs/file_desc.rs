use std::io::Cursor;

use anyhow::bail;

use crate::shared::io::ReadExt;

pub const DESCRIPTOR_LENGTH: usize = 198;
pub const MAX_FILE_NAME_LENGTH: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GRiDFileDescriptor {
    pub file_length: u32,
    pub file_name_length: u8,
    pub file_name: [u8; MAX_FILE_NAME_LENGTH],
    pub creation_date: [u8; 11],
    pub dir_file_id: u16,
    pub last_modified_date: [u8; 11],
    pub expiration_date: [u8; 11],
    pub machine_id: u32,
    pub compressed: u8,
    pub encrypted: bool,
    pub protected: bool,
    pub password: [u8; 5],
    pub dir_length: u32,
    pub dir_count: u16,
    pub grid_write1: [u8; 6],
    pub machine_id2: bool,
    pub uses_8087: bool,
    pub version1: u8,
    pub version2: u8,
    pub machine_id3: u32,
    pub grid_write2: [u8; 11],
    pub version3: u8,
    pub property_length: u32,
    pub rom: bool,
    pub rom_id: u16,
    pub mode: u16,
    pub rainy_day_bytes: [u8; 3],
    pub user_defined_bytes: [u8; 20],
    pub grid_central_use: u16,
}

impl Default for GRiDFileDescriptor {
    fn default() -> Self {
        Self {
            file_length: 0,
            file_name_length: 0,
            file_name: [0; _], // TODO(vklachkov): set file name.
            creation_date: [0; _], // TODO(vklachkov): set real date.
            dir_file_id: 0,
            last_modified_date: [0; _],  // TODO(vklachkov): set real date.
            expiration_date: [0; _], // TODO(vklachkov): set real date.
            machine_id: 0,
            compressed: 0,
            encrypted: false,
            protected: false,
            password: [0; _],
            dir_length: 0,
            dir_count: 0,
            grid_write1: [0; _],
            machine_id2: false,
            uses_8087: false,
            version1: 0,
            version2: 0,
            machine_id3: 0,
            grid_write2: [0; _],
            version3: 0,
            property_length: 0,
            rom: false,
            rom_id: 0,
            mode: 0,
            rainy_day_bytes: [0; _],
            user_defined_bytes: [0; _],
            grid_central_use: 0,
        }
    }
}

impl GRiDFileDescriptor {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() != DESCRIPTOR_LENGTH {
            bail!(
                "descriptor must be exactly {DESCRIPTOR_LENGTH} bytes, got {}",
                bytes.len()
            );
        }

        let mut cursor = Cursor::new(bytes);
        let file_length = cursor.read_u32()?;
        let file_name_length = cursor.read_u8()?;
        if file_name_length as usize > MAX_FILE_NAME_LENGTH {
            bail!("fileNameLength must be at most {MAX_FILE_NAME_LENGTH}, got {file_name_length}");
        }
        let file_name = cursor.read_array()?;
        let creation_date = cursor.read_array()?;
        let dir_file_id = cursor.read_u16()?;
        let last_modified_date = cursor.read_array()?;
        let expiration_date = cursor.read_array()?;
        let machine_id = cursor.read_u32()?;
        let compressed = cursor.read_u8()?;
        let encrypted = cursor.read_bool()?;
        let protected = cursor.read_bool()?;
        let password = cursor.read_array()?;
        let dir_length = cursor.read_u32()?;
        let dir_count = cursor.read_u16()?;
        let grid_write1 = cursor.read_array()?;
        let machine_id2 = cursor.read_bool()?;
        let uses_8087 = cursor.read_bool()?;
        let version1 = cursor.read_u8()?;
        let version2 = cursor.read_u8()?;
        let machine_id3 = cursor.read_u32()?;
        let grid_write2 = cursor.read_array()?;
        let version3 = cursor.read_u8()?;
        let property_length = cursor.read_u32()?;
        let rom = cursor.read_bool()?;
        let rom_id = cursor.read_u16()?;
        let mode = cursor.read_u16()?;
        let rainy_day_bytes = cursor.read_array()?;
        let user_defined_bytes = cursor.read_array()?;
        let grid_central_use = cursor.read_u16()?;

        Ok(Self {
            file_length,
            file_name_length,
            file_name,
            creation_date,
            dir_file_id,
            last_modified_date,
            expiration_date,
            machine_id,
            compressed,
            encrypted,
            protected,
            password,
            dir_length,
            dir_count,
            grid_write1,
            machine_id2,
            uses_8087,
            version1,
            version2,
            machine_id3,
            grid_write2,
            version3,
            property_length,
            rom,
            rom_id,
            mode,
            rainy_day_bytes,
            user_defined_bytes,
            grid_central_use,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DESCRIPTOR_LENGTH);
        bytes.extend_from_slice(&self.file_length.to_le_bytes());
        bytes.push(self.file_name_length);
        bytes.extend_from_slice(&self.file_name);
        bytes.extend_from_slice(&self.creation_date);
        bytes.extend_from_slice(&self.dir_file_id.to_le_bytes());
        bytes.extend_from_slice(&self.last_modified_date);
        bytes.extend_from_slice(&self.expiration_date);
        bytes.extend_from_slice(&self.machine_id.to_le_bytes());
        bytes.push(self.compressed);
        bytes.push(self.encrypted as u8);
        bytes.push(self.protected as u8);
        bytes.extend_from_slice(&self.password);
        bytes.extend_from_slice(&self.dir_length.to_le_bytes());
        bytes.extend_from_slice(&self.dir_count.to_le_bytes());
        bytes.extend_from_slice(&self.grid_write1);
        bytes.push(self.machine_id2 as u8);
        bytes.push(self.uses_8087 as u8);
        bytes.push(self.version1);
        bytes.push(self.version2);
        bytes.extend_from_slice(&self.machine_id3.to_le_bytes());
        bytes.extend_from_slice(&self.grid_write2);
        bytes.push(self.version3);
        bytes.extend_from_slice(&self.property_length.to_le_bytes());
        bytes.push(self.rom as u8);
        bytes.extend_from_slice(&self.rom_id.to_le_bytes());
        bytes.extend_from_slice(&self.mode.to_le_bytes());
        bytes.extend_from_slice(&self.rainy_day_bytes);
        bytes.extend_from_slice(&self.user_defined_bytes);
        bytes.extend_from_slice(&self.grid_central_use.to_le_bytes());
        debug_assert_eq!(bytes.len(), DESCRIPTOR_LENGTH);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_exactly_198_bytes_and_round_trips_all_fields() {
        let descriptor = GRiDFileDescriptor {
            file_length: 0x0102_0304,
            file_name_length: 3,
            file_name: [7; 80],
            creation_date: [8; 11],
            dir_file_id: 0x1122,
            last_modified_date: [9; 11],
            expiration_date: [10; 11],
            machine_id: 11,
            compressed: 12,
            encrypted: true,
            protected: true,
            password: [13; 5],
            dir_length: 14,
            dir_count: 0x3344,
            grid_write1: [15; 6],
            machine_id2: true,
            uses_8087: true,
            version1: 16,
            version2: 17,
            machine_id3: 18,
            grid_write2: [19; 11],
            version3: 20,
            property_length: 21,
            rom: true,
            rom_id: 0x5566,
            mode: 0x7788,
            rainy_day_bytes: [22; 3],
            user_defined_bytes: [23; 20],
            grid_central_use: 0x99aa,
        };
        let bytes = descriptor.to_bytes();
        assert_eq!(bytes.len(), DESCRIPTOR_LENGTH);
        assert_eq!(GRiDFileDescriptor::from_bytes(&bytes).unwrap(), descriptor);
    }

    #[test]
    fn filename_length_over_80_is_a_format_error() {
        let mut bytes = GRiDFileDescriptor::default().to_bytes();
        bytes[4] = 81;
        assert!(GRiDFileDescriptor::from_bytes(&bytes).is_err());
    }
}
