use std::fmt;

use jiff::Zoned;

pub const DATE_LENGTH: usize = 11;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GRiDDate(Option<GRiDDateInner>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GRiDDateInner {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub tenth_of_second: u8,
    pub day_of_week: u8,
    pub day_of_year: u16,
}

impl GRiDDate {
    pub fn today() -> Self {
        let date_time = Zoned::now();
        Self(Some(GRiDDateInner {
            year: date_time.year() as u16,
            month: date_time.month() as u8,
            day: date_time.day() as u8,
            hour: date_time.hour() as u8,
            minute: date_time.minute() as u8,
            second: date_time.second() as u8,
            tenth_of_second: (date_time.subsec_nanosecond() / 100_000_000) as u8,
            day_of_week: date_time.weekday().to_sunday_one_offset() as u8,
            day_of_year: date_time.day_of_year() as u16,
        }))
    }

    pub const fn never() -> Self {
        Self(None)
    }

    pub fn decode(bytes: [u8; DATE_LENGTH]) -> Self {
        if bytes == [0; DATE_LENGTH] {
            return Self::never();
        }

        Self(Some(GRiDDateInner {
            year: u16::from_le_bytes([bytes[0], bytes[1]]),
            month: bytes[2],
            day: bytes[3],
            hour: bytes[4],
            minute: bytes[5],
            second: bytes[6],
            tenth_of_second: bytes[7],
            day_of_week: bytes[8],
            day_of_year: u16::from_le_bytes([bytes[9], bytes[10]]),
        }))
    }

    pub fn encode(self) -> [u8; DATE_LENGTH] {
        let Some(date) = self.0 else {
            return [0; DATE_LENGTH];
        };

        let mut bytes = [0; DATE_LENGTH];
        bytes[..2].copy_from_slice(&date.year.to_le_bytes());
        bytes[2] = date.month;
        bytes[3] = date.day;
        bytes[4] = date.hour;
        bytes[5] = date.minute;
        bytes[6] = date.second;
        bytes[7] = date.tenth_of_second;
        bytes[8] = date.day_of_week;
        bytes[9..].copy_from_slice(&date.day_of_year.to_le_bytes());
        bytes
    }
}

impl fmt::Debug for GRiDDate {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(date) => fmt.debug_tuple("GRiDDate").field(&date).finish(),
            None => fmt.write_str("GRiDDate::never()"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARBITRARY_DATE: GRiDDateInner = GRiDDateInner {
        year: 2024,
        month: 2,
        day: 29,
        hour: 21,
        minute: 30,
        second: 5,
        tenth_of_second: 1,
        day_of_week: 5,
        day_of_year: 60,
    };

    #[test]
    fn never_encodes_as_all_zeroes() {
        assert_eq!(GRiDDate::never().encode(), [0; DATE_LENGTH]);
        assert_eq!(GRiDDate::decode([0; DATE_LENGTH]), GRiDDate::never());
        assert_eq!(GRiDDate::never().0, None);
    }

    #[test]
    fn encodes_each_numeric_field_in_the_grid_layout() {
        assert_eq!(
            GRiDDate(Some(ARBITRARY_DATE)).encode(),
            [0xe8, 0x07, 2, 29, 21, 30, 5, 1, 5, 60, 0]
        );
    }

    #[test]
    fn decode_preserves_arbitrary_unchecked_values() {
        let bytes = [0xff, 0xff, 99, 98, 97, 96, 95, 94, 93, 0xfe, 0xff];
        let date = GRiDDate::decode(bytes);

        assert_eq!(date.encode(), bytes);
        assert_eq!(
            date.0,
            Some(GRiDDateInner {
                year: u16::MAX,
                month: 99,
                day: 98,
                hour: 97,
                minute: 96,
                second: 95,
                tenth_of_second: 94,
                day_of_week: 93,
                day_of_year: 0xfffe,
            })
        );
    }

    #[test]
    fn structured_date_round_trips() {
        let date = GRiDDate(Some(ARBITRARY_DATE));

        assert_eq!(GRiDDate::decode(date.encode()), date);
        assert_eq!(date.0, Some(ARBITRARY_DATE));
    }

    #[test]
    fn today_produces_a_populated_date() {
        let date = GRiDDate::today().0.unwrap();

        assert!(date.year >= 2026);
        assert!((1..=12).contains(&date.month));
        assert!((1..=31).contains(&date.day));
        assert!((1..=7).contains(&date.day_of_week));
        assert!((1..=366).contains(&date.day_of_year));
    }

    #[test]
    fn debug_distinguishes_never_and_populated_dates() {
        assert_eq!(format!("{:?}", GRiDDate::never()), "GRiDDate::never()");
        let debug = format!("{:?}", GRiDDate(Some(ARBITRARY_DATE)));
        assert!(debug.contains("GRiDDate"));
        assert!(debug.contains("year: 2024"));
    }
}
