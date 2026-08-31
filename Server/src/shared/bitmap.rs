use std::{mem, num::NonZeroU16};

use wide::u8x16;

const SIZE: usize = u16::MAX as usize;
const ALLOC_SIZE: usize = (SIZE + 1) / 8;

const CHUNK_SIZE: usize = 16;
const TOTAL_CHUNKS: usize = ALLOC_SIZE / CHUNK_SIZE;

#[repr(align(16))]
struct Align16<T>(T);

impl<T> std::ops::Deref for Align16<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Align16<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct IdMap(Box<Align16<[u8; ALLOC_SIZE]>>);

impl IdMap {
    pub fn new() -> Self {
        // SAFETY: Zero is valid for AlignedBytes.
        Self(unsafe { Box::new_zeroed().assume_init() })
    }

    pub fn set(&mut self, id: NonZeroU16) {
        let bit = id.get() - 1;
        let byte = (bit >> 3) as usize;
        let mask = 1u8 << (bit & 0b111);
        self.0[byte] |= mask;
    }

    pub fn clear(&mut self, id: NonZeroU16) {
        let bit = id.get() - 1;
        let byte = (bit >> 3) as usize;
        let mask = 1u8 << (bit & 0b111);
        self.0[byte] &= !mask;
    }

    pub fn find_free(&self) -> Option<NonZeroU16> {
        // SAFETY: AlignedBytes provides 16-byte alignment and equal-sized chunks.
        let chunks =
            unsafe { mem::transmute::<&[u8; ALLOC_SIZE], &[u8x16; TOTAL_CHUNKS]>(&self.0.0) };

        for i in 0..chunks.len() {
            let chunk = &chunks[i];

            let free_bytes = chunk.simd_ne(0xFF).to_bitmask();
            if free_bytes == 0 {
                continue;
            }

            let byte_offset = free_bytes.trailing_zeros() as usize;
            let byte = self.0[i * CHUNK_SIZE + byte_offset];
            let free_bit_offset = byte.trailing_ones() as usize;

            let candidate = 1 + i * CHUNK_SIZE * 8 + byte_offset * 8 + free_bit_offset;
            if candidate > SIZE {
                return None;
            }

            // SAFETY: candidate is in 1..=u16::MAX.
            return unsafe { Some(NonZeroU16::new_unchecked(candidate as u16)) };
        }
        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u16) -> NonZeroU16 {
        NonZeroU16::new(value).unwrap()
    }

    #[test]
    fn sets_and_clears_bits_in_the_first_byte() {
        let mut map = IdMap::new();

        map.set(id(1));
        map.set(id(8));
        assert_eq!(map.0[0], 0b1000_0001);

        map.clear(id(1));
        assert_eq!(map.0[0], 0b1000_0000);

        map.clear(id(8));
        assert_eq!(map.0[0], 0);
    }

    #[test]
    fn sets_and_clears_bits_in_the_second_byte() {
        let mut map = IdMap::new();

        map.set(id(9));
        map.set(id(16));
        assert_eq!(map.0[1], 0b1000_0001);

        map.clear(id(9));
        assert_eq!(map.0[1], 0b1000_0000);

        map.clear(id(16));
        assert_eq!(map.0[1], 0);
    }

    #[test]
    fn sets_and_clears_bits_in_another_chunk() {
        let mut map = IdMap::new();

        map.set(id(129));
        map.set(id(136));
        assert_eq!(map.0[16], 0b1000_0001);

        map.clear(id(129));
        assert_eq!(map.0[16], 0b1000_0000);

        map.clear(id(136));
        assert_eq!(map.0[16], 0);
    }

    #[test]
    fn finds_the_first_free_id_in_the_first_byte() {
        let mut map = IdMap::new();

        assert_eq!(map.find_free(), Some(id(1)));

        map.set(id(1));

        assert_eq!(map.find_free(), Some(id(2)));

        map.set(id(2));
        map.set(id(3));
        map.set(id(4));
        map.set(id(5));
        map.set(id(6));
        map.set(id(7));

        assert_eq!(map.find_free(), Some(id(8)));

        map.set(id(8));

        assert_eq!(map.find_free(), Some(id(9)));
    }

    #[test]
    fn setting_and_clearing_a_bit_in_the_first_byte_changes_find_free() {
        let mut map = IdMap::new();
        map.set(id(1));

        assert_eq!(map.find_free(), Some(id(2)));

        map.clear(id(1));

        assert_eq!(map.find_free(), Some(id(1)));
    }

    #[test]
    fn setting_and_clearing_a_bit_in_the_second_byte_changes_find_free() {
        let mut map = IdMap::new();
        map.set(id(1));
        map.set(id(2));
        map.set(id(3));
        map.set(id(4));
        map.set(id(5));
        map.set(id(6));
        map.set(id(7));
        map.set(id(8));
        map.set(id(9));

        assert_eq!(map.find_free(), Some(id(10)));

        map.clear(id(9));

        assert_eq!(map.find_free(), Some(id(9)));

        map.set(id(9));

        assert_eq!(map.find_free(), Some(id(10)));
    }

    #[test]
    fn setting_and_clearing_a_bit_in_another_chunk_changes_find_free() {
        const CHUNK_LAST_ID: u16 = 128;
        let mut map = IdMap::new();

        for value in 1..=CHUNK_LAST_ID {
            map.set(id(value));
        }

        assert_eq!(map.find_free(), Some(id(CHUNK_LAST_ID + 1)));

        map.clear(id(CHUNK_LAST_ID));

        assert_eq!(map.find_free(), Some(id(CHUNK_LAST_ID)));

        map.set(id(CHUNK_LAST_ID));

        assert_eq!(map.find_free(), Some(id(CHUNK_LAST_ID + 1)));
    }

    #[test]
    fn setting_an_already_set_bit_and_clearing_an_already_clear_bit_are_idempotent() {
        let mut map = IdMap::new();

        map.set(id(17));
        map.set(id(17));
        assert_eq!(map.find_free(), Some(id(1)));

        map.clear(id(17));
        map.clear(id(17));
        assert_eq!(map.find_free(), Some(id(1)));
    }

    #[test]
    fn a_full_map_has_no_free_id_and_clearing_the_last_id_makes_it_free() {
        let mut map = IdMap::new();

        for value in 1..=u16::MAX {
            map.set(id(value));
        }

        assert_eq!(map.find_free(), None);

        map.clear(id(u16::MAX));

        assert_eq!(map.find_free(), Some(id(u16::MAX)));
    }
}
