use std::{mem, num::NonZero};

use wide::u8x16;

const CHUNK_SIZE: usize = 16;

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

macro_rules! id_map {
    ($name:ident, $int:ty) => {
        pub struct $name(Box<Align16<[u8; (<$int>::MAX as usize + 1) / 8]>>);

        #[allow(clippy::indexing_slicing)]
        impl $name {
            const SIZE: usize = <$int>::MAX as usize;
            const ALLOC_SIZE: usize = (Self::SIZE + 1) / 8;
            const TOTAL_CHUNKS: usize = Self::ALLOC_SIZE / CHUNK_SIZE;

            pub fn new() -> Self {
                // SAFETY: Zero is valid for AlignedBytes.
                Self(unsafe { Box::new_zeroed().assume_init() })
            }

            pub fn set(&mut self, id: NonZero<$int>) {
                let bit = id.get() - 1;
                let byte = (bit >> 3) as usize;
                let mask = 1u8 << (bit & 0b111);
                self.0[byte] |= mask;
            }

            pub fn is_set(&self, id: NonZero<$int>) -> bool {
                let bit = id.get() - 1;
                let byte = (bit >> 3) as usize;
                let mask = 1u8 << (bit & 0b111);
                self.0[byte] & mask != 0
            }

            pub fn clear(&mut self, id: NonZero<$int>) {
                let bit = id.get() - 1;
                let byte = (bit >> 3) as usize;
                let mask = 1u8 << (bit & 0b111);
                self.0[byte] &= !mask;
            }

            pub fn find_free(&self) -> Option<NonZero<$int>> {
                // SAFETY: AlignedBytes provides 16-byte alignment and equal-sized chunks.
                let chunks = unsafe {
                    mem::transmute::<&[u8; Self::ALLOC_SIZE], &[u8x16; Self::TOTAL_CHUNKS]>(
                        &self.0.0,
                    )
                };

                for (i, chunk) in chunks.iter().enumerate() {
                    let free_bytes = chunk.simd_ne(0xFF).to_bitmask();
                    if free_bytes == 0 {
                        continue;
                    }

                    let byte_offset = free_bytes.trailing_zeros() as usize;
                    let byte = self.0[i * CHUNK_SIZE + byte_offset];
                    let free_bit_offset = byte.trailing_ones() as usize;

                    let candidate = 1 + i * CHUNK_SIZE * 8 + byte_offset * 8 + free_bit_offset;
                    if candidate > Self::SIZE {
                        return None;
                    }

                    // SAFETY: candidate is in 1..=<$int>::MAX.
                    return unsafe { Some(NonZero::new_unchecked(candidate as $int)) };
                }

                None
            }
        }
    };
}

id_map!(IdMap8, u8);
id_map!(IdMap16, u16);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u16) -> NonZero<u16> {
        NonZero::new(value).unwrap()
    }

    fn id8(value: u8) -> NonZero<u8> {
        NonZero::new(value).unwrap()
    }

    #[test]
    fn sets_and_clears_bits_in_the_first_byte() {
        let mut map = IdMap16::new();

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
        let mut map = IdMap16::new();

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
        let mut map = IdMap16::new();

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
        let mut map = IdMap16::new();

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
        let mut map = IdMap16::new();
        map.set(id(1));

        assert_eq!(map.find_free(), Some(id(2)));

        map.clear(id(1));

        assert_eq!(map.find_free(), Some(id(1)));
    }

    #[test]
    fn setting_and_clearing_a_bit_in_the_second_byte_changes_find_free() {
        let mut map = IdMap16::new();
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
        let mut map = IdMap16::new();

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
        let mut map = IdMap16::new();

        map.set(id(17));
        map.set(id(17));
        assert_eq!(map.find_free(), Some(id(1)));

        map.clear(id(17));
        map.clear(id(17));
        assert_eq!(map.find_free(), Some(id(1)));
    }

    #[test]
    fn a_full_map_has_no_free_id_and_clearing_the_last_id_makes_it_free() {
        let mut map = IdMap16::new();

        for value in 1..=u16::MAX {
            map.set(id(value));
        }

        assert_eq!(map.find_free(), None);

        map.clear(id(u16::MAX));

        assert_eq!(map.find_free(), Some(id(u16::MAX)));
    }

    #[test]
    fn the_eight_bit_map_sets_and_clears_bits() {
        let mut map = IdMap8::new();

        map.set(id8(1));
        map.set(id8(8));
        assert_eq!(map.0[0], 0b1000_0001);
        assert!(map.is_set(id8(1)));

        map.clear(id8(1));
        assert_eq!(map.0[0], 0b1000_0000);
        assert!(!map.is_set(id8(1)));

        map.clear(id8(8));
        assert_eq!(map.0[0], 0);
    }

    #[test]
    fn the_eight_bit_map_sets_and_clears_bits_in_its_last_chunk() {
        let mut map = IdMap8::new();

        map.set(id8(129));
        map.set(id8(255));
        assert_eq!(map.0[16], 0b0000_0001);
        assert_eq!(map.0[31], 0b0100_0000);

        map.clear(id8(129));
        map.clear(id8(255));
        assert_eq!(map.0[16], 0);
        assert_eq!(map.0[31], 0);
    }

    #[test]
    fn the_eight_bit_map_finds_the_first_free_id() {
        let mut map = IdMap8::new();

        assert_eq!(map.find_free(), Some(id8(1)));

        for value in 1..=8 {
            map.set(id8(value));
        }

        assert_eq!(map.find_free(), Some(id8(9)));

        map.clear(id8(4));

        assert_eq!(map.find_free(), Some(id8(4)));
    }

    #[test]
    fn a_full_eight_bit_map_has_no_free_id_and_clearing_the_last_id_makes_it_free() {
        let mut map = IdMap8::new();

        for value in 1..=u8::MAX {
            map.set(id8(value));
        }

        assert_eq!(map.find_free(), None);

        map.clear(id8(u8::MAX));

        assert_eq!(map.find_free(), Some(id8(u8::MAX)));
    }
}
