//! UTC calendar dates, and the port through which the log writer asks for today's.

use core::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC calendar day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl CivilDate {
    /// The UTC day containing `seconds` after 1970-01-01T00:00:00Z (negative for earlier).
    #[must_use]
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        // Howard Hinnant's days-from-civil, inverted: shift the epoch to 0000-03-01 so the leap
        // day is the last day of the year, then count 400-year eras of 146,097 days.
        let days = seconds.div_euclid(86_400) + 719_468;
        let era = days.div_euclid(146_097);
        let day_of_era = days.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = if month_index < 10 {
            month_index + 3
        } else {
            month_index - 9
        };
        let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
        Self {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    }

    /// Reads exactly `YYYY-MM-DD`; anything else, or a day that does not exist, is `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let digits = |range: core::ops::Range<usize>| -> Option<u32> {
            bytes[range].iter().try_fold(0u32, |n, &b| {
                b.is_ascii_digit().then(|| n * 10 + u32::from(b - b'0'))
            })
        };
        let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
        let candidate = Self {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        };
        (candidate.exists() && day > 0).then_some(candidate)
    }

    /// Whether the month is 1 to 12 and the day is one of its days.
    const fn exists(&self) -> bool {
        let leap = (self.year % 4 == 0 && self.year % 100 != 0) || self.year % 400 == 0;
        let length = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if leap {
                    29
                } else {
                    28
                }
            }
            _ => return false,
        };
        self.day >= 1 && self.day <= length
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// Answers which UTC day it is now.
pub trait Calendar: Send + Sync {
    fn today(&self) -> CivilDate;
}

/// The wall clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCalendar;

impl Calendar for SystemCalendar {
    fn today(&self) -> CivilDate {
        let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH);
        let seconds = since_epoch.map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
        CivilDate::from_unix_seconds(seconds)
    }
}
