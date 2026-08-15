//! Sentry and GRiDMail records differ only in their header, so both dialects
//! share one walk here. Truncated input ends as an error, never as a panic or
//! a silent stop.

use std::io;

use super::{
    error::FrameError,
    utils::{CursorExt, ReadExt},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlvKind {
    /// `<tag><u8 length><value>`, the Sentry admin protocol.
    TagU8,
    /// `<marker><u16 length><tag + value>`, the GRiDMail application framing.
    /// The length counts the tag byte, so a length of zero is malformed.
    MarkerU16 {
        marker: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlvEntry<'a> {
    pub tag: u8,
    pub value: &'a [u8],
}

/// Yields `Result` rather than collecting into a `Vec` to keep the common
/// "find one tag" case allocation free.
#[derive(Clone, Debug)]
pub struct Tlv<'a> {
    cursor: io::Cursor<&'a [u8]>,
    kind: TlvKind,
    failed: bool,
}

impl<'a> Tlv<'a> {
    pub fn new(data: &'a [u8], kind: TlvKind) -> Self {
        Self {
            cursor: io::Cursor::new(data),
            kind,
            failed: false,
        }
    }

    pub fn tag_u8(data: &'a [u8]) -> Self {
        Self::new(data, TlvKind::TagU8)
    }

    pub fn marker_u16(data: &'a [u8], marker: u8) -> Self {
        Self::new(data, TlvKind::MarkerU16 { marker })
    }

    pub fn position(&self) -> usize {
        self.cursor.position() as usize
    }

    pub fn collect_all(self) -> Result<Vec<TlvEntry<'a>>, FrameError> {
        self.collect()
    }

    /// Every record up to the first malformed one, which is as far as a stream
    /// that may carry trailing non-record bytes can be trusted.
    pub fn well_formed_prefix(self) -> Vec<TlvEntry<'a>> {
        self.map_while(Result::ok).collect()
    }

    pub fn find_tag(self, wanted: u8) -> Option<&'a [u8]> {
        self.flatten()
            .find(|entry| entry.tag == wanted)
            .map(|entry| entry.value)
    }

    pub fn all_records_valid(self) -> bool {
        let mut count = 0;
        for entry in self {
            if entry.is_err() {
                return false;
            }
            count += 1;
        }
        count != 0
    }

    fn read_entry(&mut self) -> Result<TlvEntry<'a>, FrameError> {
        match self.kind {
            TlvKind::TagU8 => {
                let tag = self.cursor.read_u8()?;
                let length = self.cursor.read_u8()? as usize;
                let value = self.cursor.read_slice(length)?;
                Ok(TlvEntry { tag, value })
            }
            TlvKind::MarkerU16 { marker } => {
                let found = self.cursor.read_u8()?;
                if found != marker {
                    return Err(FrameError::Validation {
                        reason: format!("expected record marker {marker:#04x}, found {found:#04x}"),
                    });
                }

                let length = self.cursor.read_u16()? as usize;
                // The length covers the tag byte, so an empty record cannot
                // carry one and the walk would not advance.
                if length == 0 {
                    return Err(FrameError::Validation {
                        reason: "record length must cover the tag byte".to_owned(),
                    });
                }

                let record = self.cursor.read_slice(length)?;
                Ok(TlvEntry {
                    tag: record[0],
                    value: &record[1..],
                })
            }
        }
    }
}

impl<'a> Iterator for Tlv<'a> {
    type Item = Result<TlvEntry<'a>, FrameError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.cursor.position() as usize >= self.cursor.get_ref().len() {
            return None;
        }

        let entry = self.read_entry();
        if entry.is_err() {
            // One malformed record makes everything after it meaningless, so
            // the walk stops instead of resynchronising on arbitrary bytes.
            self.failed = true;
        }

        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_tag_u8_records() {
        let entries = Tlv::tag_u8(&[0x07, 0x02, b'h', b'i', 0x09, 0x00])
            .collect_all()
            .unwrap();

        assert_eq!(
            entries,
            [
                TlvEntry {
                    tag: 0x07,
                    value: b"hi"
                },
                TlvEntry {
                    tag: 0x09,
                    value: b""
                },
            ]
        );
    }

    #[test]
    fn walks_marker_u16_records() {
        let entries = Tlv::marker_u16(&[0xfd, 3, 0, b't', b'a', b'b', 0xfd, 1, 0, b'z'], 0xfd)
            .collect_all()
            .unwrap();

        assert_eq!(
            entries,
            [
                TlvEntry {
                    tag: b't',
                    value: b"ab"
                },
                TlvEntry {
                    tag: b'z',
                    value: b""
                },
            ]
        );
    }

    #[test]
    fn rejects_truncated_records() {
        assert!(Tlv::tag_u8(&[0x09, 0x07, b'Z']).collect_all().is_err());
        assert!(Tlv::tag_u8(&[0x09]).collect_all().is_err());
        assert!(
            Tlv::marker_u16(&[0xfd, 9, 0, b't'], 0xfd)
                .collect_all()
                .is_err()
        );
    }

    #[test]
    fn rejects_a_wrong_marker() {
        assert!(
            Tlv::marker_u16(&[0xfe, 1, 0, b'z'], 0xfd)
                .collect_all()
                .is_err()
        );
    }

    #[test]
    fn rejects_a_record_that_cannot_hold_a_tag() {
        assert!(Tlv::marker_u16(&[0xfd, 0, 0], 0xfd).collect_all().is_err());
    }

    #[test]
    fn stops_after_the_first_malformed_record() {
        let mut tlv = Tlv::tag_u8(&[0x07, 0x01, b'a', 0x09, 0x07, b'Z']);

        assert!(tlv.next().unwrap().is_ok());
        assert!(tlv.next().unwrap().is_err());
        assert!(tlv.next().is_none());
    }

    #[test]
    fn keeps_the_records_before_a_trailing_tail() {
        let data = [
            0xfd, 2, 0, b't', b'a', 0xfd, 1, 0, b'z', b'j', b'u', b'n', b'k',
        ];

        assert_eq!(
            Tlv::marker_u16(&data, 0xfd).well_formed_prefix(),
            [
                TlvEntry {
                    tag: b't',
                    value: b"a"
                },
                TlvEntry {
                    tag: b'z',
                    value: b""
                },
            ]
        );
    }

    #[test]
    fn finds_a_tag_without_collecting() {
        let data = [0xfd, 5, 0, b't', b'U', b's', b'e', b'r', 0xfd, 1, 0, b'z'];

        assert_eq!(
            Tlv::marker_u16(&data, 0xfd).find_tag(b't'),
            Some(&b"User"[..])
        );
        assert_eq!(Tlv::marker_u16(&data, 0xfd).find_tag(b'q'), None);
    }
}
