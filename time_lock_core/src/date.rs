use core::fmt;

/// The game calendar uses local time, without timezone or DST conversion.
pub const EPOCH_1900_TO_2000_MS: i64 = 3_155_673_600_000;
pub const MAX_MS: i64 = 3_155_759_999_999; // 2099-12-31 23:59:59.999
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateError {
    Format,
    Range,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
}

pub const fn leap_year(year: u16) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

pub fn check_ms(ms: i64) -> Result<i64, DateError> {
    if (0..=MAX_MS).contains(&ms) {
        Ok(ms)
    } else {
        Err(DateError::Range)
    }
}

pub fn from_1900_ms(ms: i64) -> Result<i64, DateError> {
    check_ms(
        ms.checked_sub(EPOCH_1900_TO_2000_MS)
            .ok_or(DateError::Overflow)?,
    )
}

pub fn to_1900_ms(ms: i64) -> Result<i64, DateError> {
    check_ms(ms)?
        .checked_add(EPOCH_1900_TO_2000_MS)
        .ok_or(DateError::Overflow)
}

impl DateTime {
    pub const EPOCH: Self = Self {
        year: 2000,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
    };

    pub fn validate(&self) -> Result<(), DateError> {
        if !(2000..=2099).contains(&self.year)
            || self.day == 0
            || self.day > days_in_month(self.year, self.month)
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
            || self.millisecond > 999
        {
            return Err(DateError::Range);
        }
        Ok(())
    }

    /// Strict ISO local calendar input; omitted milliseconds mean exactly .000.
    pub fn parse(input: &str) -> Result<Self, DateError> {
        let b = input.as_bytes();
        if !(b.len() == 19 || b.len() == 23)
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
            || (b.len() == 23 && b[19] != b'.')
        {
            return Err(DateError::Format);
        }
        fn number(b: &[u8]) -> Result<u16, DateError> {
            b.iter().try_fold(0u16, |n, c| {
                if c.is_ascii_digit() {
                    Ok(n * 10 + u16::from(c - b'0'))
                } else {
                    Err(DateError::Format)
                }
            })
        }
        let value = Self {
            year: number(&b[0..4])?,
            month: number(&b[5..7])? as u8,
            day: number(&b[8..10])? as u8,
            hour: number(&b[11..13])? as u8,
            minute: number(&b[14..16])? as u8,
            second: number(&b[17..19])? as u8,
            millisecond: if b.len() == 23 {
                number(&b[20..23])?
            } else {
                0
            },
        };
        value.validate()?;
        Ok(value)
    }

    pub fn to_epoch_ms(self) -> Result<i64, DateError> {
        self.validate()?;
        let mut days = 0i64;
        for year in 2000..self.year {
            days += if leap_year(year) { 366 } else { 365 };
        }
        for month in 1..self.month {
            days += i64::from(days_in_month(self.year, month));
        }
        days += i64::from(self.day - 1);
        Ok(days * DAY_MS
            + i64::from(self.hour) * 3_600_000
            + i64::from(self.minute) * 60_000
            + i64::from(self.second) * 1000
            + i64::from(self.millisecond))
    }

    pub fn from_epoch_ms(ms: i64) -> Result<Self, DateError> {
        check_ms(ms)?;
        let mut days = ms / DAY_MS;
        let mut year = 2000;
        loop {
            let count = if leap_year(year) { 366 } else { 365 };
            if days < count {
                break;
            }
            days -= count;
            year += 1;
        }
        let mut month = 1;
        while days >= i64::from(days_in_month(year, month)) {
            days -= i64::from(days_in_month(year, month));
            month += 1;
        }
        Ok(Self {
            year,
            month,
            day: days as u8 + 1,
            hour: ((ms / 3_600_000) % 24) as u8,
            minute: ((ms / 60_000) % 60) as u8,
            second: ((ms / 1000) % 60) as u8,
            millisecond: (ms % 1000) as u16,
        })
    }

    /// Reject an invalid destination (e.g. Feb 30); never silently clamp a date.
    pub fn edit(self, field: usize, delta: i16) -> Result<Self, DateError> {
        let mut result = self;
        fn add(n: u16, delta: i16) -> Result<u16, DateError> {
            u16::try_from(i32::from(n) + i32::from(delta)).map_err(|_| DateError::Range)
        }
        fn add8(n: u8, delta: i16) -> Result<u8, DateError> {
            u8::try_from(add(u16::from(n), delta)?).map_err(|_| DateError::Range)
        }
        match field {
            0 => result.year = add(result.year, delta)?,
            1 => result.month = add8(result.month, delta)?,
            2 => result.day = add8(result.day, delta)?,
            3 => result.hour = add8(result.hour, delta)?,
            4 => result.minute = add8(result.minute, delta)?,
            5 => result.second = add8(result.second, delta)?,
            6 => result.millisecond = add(result.millisecond, delta)?,
            _ => return Err(DateError::Range),
        }
        result.validate()?;
        Ok(result)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.millisecond
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_calendar_days_round_trip() {
        for day in 0..36_525 {
            for tail in [0, 86_399_999] {
                let ms = day * DAY_MS + tail;
                assert_eq!(DateTime::from_epoch_ms(ms).unwrap().to_epoch_ms(), Ok(ms));
            }
        }
    }

    #[test]
    fn epoch_and_range() {
        assert_eq!(DateTime::EPOCH.to_epoch_ms(), Ok(0));
        assert_eq!(
            DateTime::parse("2099-12-31T23:59:59.999")
                .unwrap()
                .to_epoch_ms(),
            Ok(MAX_MS)
        );
        assert_eq!(
            DateTime::parse("2000-01-01T00:00:00").unwrap().millisecond,
            0
        );
        assert_eq!(to_1900_ms(0), Ok(EPOCH_1900_TO_2000_MS));
        assert_eq!(from_1900_ms(EPOCH_1900_TO_2000_MS + 1), Ok(1));
        assert!(from_1900_ms(i64::MIN).is_err());
        assert!(DateTime::from_epoch_ms(-1).is_err());
        assert!(DateTime::from_epoch_ms(MAX_MS + 1).is_err());
    }

    #[test]
    fn invalid_dates_are_rejected() {
        for value in [
            "2001-02-29T00:00:00",
            "2100-01-01T00:00:00",
            "1999-12-31T23:59:59",
            "2000-00-01T00:00:00",
            "2000-13-01T00:00:00",
            "2000-04-31T00:00:00",
            "2000-01-00T00:00:00",
            "2000-01-01T24:00:00",
            "2000-01-01T00:00:60",
            "2000-01-01 00:00:00",
            "2000-01-01T00:00:00.1",
            "2000-01-01T00:00:00Z",
            "２０００-01-01T00:00:00",
        ] {
            assert!(DateTime::parse(value).is_err(), "{value}");
        }
        assert!(DateTime::parse("2000-02-29T12:34:56.999").is_ok());
        assert!(!leap_year(2100));
        assert!(leap_year(2000));
        assert!(DateTime::parse("2000-01-31T00:00:00")
            .unwrap()
            .edit(1, 1)
            .is_err());
    }
}
