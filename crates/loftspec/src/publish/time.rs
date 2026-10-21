//! The publish time: whole seconds since the Unix epoch, written in the RFC 3339 form in UTC to
//! the second, ending in `Z`.

use std::fmt;
use std::str::FromStr;

/// The last second a four-digit year can name: 9999-12-31T23:59:59Z.
const LAST: u64 = 253_402_300_799;

const SECONDS_PER_DAY: u64 = 86_400;

/// When a publish happened, ordered by the instant it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublishedAt(u64);

/// A time that is not one the lock can carry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    #[error("{0} seconds since 1970 lies past the year 9999")]
    TooLate(u64),
    #[error("`{0}` is not a UTC time of the form YYYY-MM-DDTHH:MM:SSZ")]
    NotRfc3339(String),
}

impl PublishedAt {
    /// The time `seconds` after 1970-01-01T00:00:00Z.
    ///
    /// # Errors
    ///
    /// The time lies past the year 9999.
    pub fn from_unix_seconds(seconds: u64) -> Result<Self, TimeError> {
        if seconds > LAST {
            return Err(TimeError::TooLate(seconds));
        }
        Ok(Self(seconds))
    }
}

impl fmt::Display for PublishedAt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil_from_days(self.0 / SECONDS_PER_DAY);
        let second = self.0 % SECONDS_PER_DAY;
        write!(
            formatter,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            second / 3600,
            second / 60 % 60,
            second % 60
        )
    }
}

impl FromStr for PublishedAt {
    type Err = TimeError;

    fn from_str(text: &str) -> Result<Self, TimeError> {
        let error = || TimeError::NotRfc3339(text.to_owned());
        let bytes = text.as_bytes();
        let shaped = bytes.len() == 20
            && bytes.iter().enumerate().all(|(index, byte)| match index {
                4 | 7 => *byte == b'-',
                10 => *byte == b'T',
                13 | 16 => *byte == b':',
                19 => *byte == b'Z',
                _ => byte.is_ascii_digit(),
            });
        if !shaped {
            return Err(error());
        }
        let number =
            |range: std::ops::Range<usize>| -> u64 { text[range].parse().expect("checked digits") };
        let (year, month, day) = (number(0..4), number(5..7), number(8..10));
        let (hour, minute, second) = (number(11..13), number(14..16), number(17..19));
        let valid = year >= 1970
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day)
            && hour < 24
            && minute < 60
            && second < 60;
        if !valid {
            return Err(error());
        }
        let days = days_from_civil(year, month, day);
        Ok(Self(
            days * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second,
        ))
    }
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The days from 1970-01-01 to a date on or after it, in the proleptic Gregorian calendar.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date `days` after 1970-01-01, in the proleptic Gregorian calendar.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = (month_from_march + 2) % 12 + 1;
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}
