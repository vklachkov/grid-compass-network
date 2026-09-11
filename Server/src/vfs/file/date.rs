use std::{fmt, time::SystemTime};

use jiff::{Timestamp, Zoned, tz::TimeZone};
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned,
    byteorder::{LE, U16},
};

pub const DATE_LENGTH: usize = 11;

#[derive(Clone, Copy, FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, PartialEq, Eq)]
#[repr(C)]
pub struct GRiDDate {
    year: U16<LE>,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    tenth_of_second: u8,
    day_of_week: u8,
    day_of_year: U16<LE>,
}

const _: () = assert!(size_of::<GRiDDate>() == DATE_LENGTH);

const NEVER: GRiDDate = GRiDDate {
    year: U16::ZERO,
    month: 0,
    day: 0,
    hour: 0,
    minute: 0,
    second: 0,
    tenth_of_second: 0,
    day_of_week: 0,
    day_of_year: U16::ZERO,
};

impl GRiDDate {
    pub fn today() -> Self {
        Self::from_zoned(&Zoned::now())
    }

    fn from_zoned(date_time: &Zoned) -> Self {
        Self {
            year: U16::new(date_time.year() as u16),
            month: date_time.month() as u8,
            day: date_time.day() as u8,
            hour: date_time.hour() as u8,
            minute: date_time.minute() as u8,
            second: date_time.second() as u8,
            tenth_of_second: (date_time.subsec_nanosecond() / 100_000_000) as u8,
            day_of_week: date_time.weekday().to_sunday_one_offset() as u8,
            day_of_year: U16::new(date_time.day_of_year() as u16),
        }
    }

    pub const fn never() -> Self {
        NEVER
    }
}

/// Timestamps outside the range GRiD can express degrade to `never` rather
/// than failing the request that carries them.
impl From<SystemTime> for GRiDDate {
    fn from(time: SystemTime) -> Self {
        match Timestamp::try_from(time) {
            Ok(timestamp) => Self::from_zoned(&timestamp.to_zoned(TimeZone::system())),
            Err(_) => Self::never(),
        }
    }
}

/// Hand-written so the operator log keeps showing plain numbers instead of the
/// `U16(2024)` wrapper the byte-order types print.
impl fmt::Debug for GRiDDate {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == NEVER {
            return fmt.write_str("GRiDDate::never()");
        }

        fmt.debug_struct("GRiDDate")
            .field("year", &self.year.get())
            .field("month", &self.month)
            .field("day", &self.day)
            .field("hour", &self.hour)
            .field("minute", &self.minute)
            .field("second", &self.second)
            .field("tenth_of_second", &self.tenth_of_second)
            .field("day_of_week", &self.day_of_week)
            .field("day_of_year", &self.day_of_year.get())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARBITRARY_DATE: GRiDDate = GRiDDate {
        year: U16::new(2024),
        month: 2,
        day: 29,
        hour: 21,
        minute: 30,
        second: 5,
        tenth_of_second: 1,
        day_of_week: 5,
        day_of_year: U16::new(60),
    };

    #[test]
    fn never_encodes_as_all_zeroes() {
        assert_eq!(GRiDDate::never().as_bytes(), [0; DATE_LENGTH]);
        assert_eq!(
            GRiDDate::read_from_bytes(&[0; DATE_LENGTH]).unwrap(),
            GRiDDate::never()
        );
    }

    #[test]
    fn encodes_each_numeric_field_in_the_grid_layout() {
        assert_eq!(
            ARBITRARY_DATE.as_bytes(),
            [0xe8, 0x07, 2, 29, 21, 30, 5, 1, 5, 60, 0]
        );
    }

    #[test]
    fn decode_preserves_arbitrary_unchecked_values() {
        let bytes = [0xff, 0xff, 99, 98, 97, 96, 95, 94, 93, 0xfe, 0xff];
        let date = GRiDDate::read_from_bytes(&bytes).unwrap();

        assert_eq!(date.as_bytes(), bytes);
        assert_eq!(
            date,
            GRiDDate {
                year: U16::new(u16::MAX),
                month: 99,
                day: 98,
                hour: 97,
                minute: 96,
                second: 95,
                tenth_of_second: 94,
                day_of_week: 93,
                day_of_year: U16::new(0xfffe),
            }
        );
    }

    #[test]
    fn structured_date_round_trips() {
        assert_eq!(
            GRiDDate::read_from_bytes(ARBITRARY_DATE.as_bytes()).unwrap(),
            ARBITRARY_DATE
        );
    }

    #[test]
    fn today_produces_a_populated_date() {
        let date = GRiDDate::today();

        assert!(date.year.get() >= 2026);
        assert!((1..=12).contains(&date.month));
        assert!((1..=31).contains(&date.day));
        assert!((1..=7).contains(&date.day_of_week));
        assert!((1..=366).contains(&date.day_of_year.get()));
    }

    #[test]
    fn debug_distinguishes_never_and_populated_dates() {
        assert_eq!(format!("{:?}", GRiDDate::never()), "GRiDDate::never()");
        let debug = format!("{:?}", ARBITRARY_DATE);
        assert!(debug.contains("GRiDDate"));
        assert!(debug.contains("year: 2024"));
    }
}
