//! Civil (proleptic Gregorian) dates for the date kinds, without a time-zone library: day numbers
//! from Howard Hinnant's `days_from_civil`/`civil_from_days` (public domain, "chrono-Compatible
//! Low-Level Date Algorithms"), ISO 8601 `YYYY-MM-DD` parsing and printing, ISO week numbers, month
//! arithmetic that clamps the day (Jan 31 + 1 month = Feb 28/29, as jiff's `Span::months`), and the
//! `strftime` subset the `format` prop documents.
//!
//! Years are 1..=9999 (svelters-egui used jiff, whose range is wider; a date prop outside four-digit
//! years is not ISO `YYYY-MM-DD` anyway). Arithmetic that would leave that range returns the date
//! unchanged, as jiff's `checked_add(..).unwrap_or(d)` did.

use std::fmt;

/// A calendar date. Ordered chronologically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

pub const MIN_YEAR: i32 = 1;
pub const MAX_YEAR: i32 = 9999;

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
/// Monday first.
pub const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Whether `y` is a Gregorian leap year.
pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// The number of days in month `m` (1..=12) of year `y`.
pub fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 of `y-m-d`.
fn days_from_civil(y: i32, m: u8, d: u8) -> i64 {
    let y = if m <= 2 { y as i64 - 1 } else { y as i64 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of day `z` since 1970-01-01.
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

impl Date {
    /// `y-m-d` when it is a real date within [`MIN_YEAR`]..=[`MAX_YEAR`].
    pub fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        ((MIN_YEAR..=MAX_YEAR).contains(&year) && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)).then_some(Date { year, month, day })
    }
    pub fn year(self) -> i32 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01.
    pub fn to_days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// The date `days` after 1970-01-01, if within range.
    pub fn from_days(days: i64) -> Option<Date> {
        let (y, m, d) = civil_from_days(days);
        Date::new(y, m, d)
    }

    /// Today in UTC (the platform clock; svelters-egui took the local zone's date through jiff,
    /// which presto has no time-zone database for yet).
    pub fn today_utc() -> Date {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        Date::from_days(secs.div_euclid(86_400)).unwrap_or(Date { year: 1970, month: 1, day: 1 })
    }

    /// Monday = 0 … Sunday = 6.
    pub fn weekday(self) -> u8 {
        // 1970-01-01 was a Thursday (3)
        (self.to_days() + 3).rem_euclid(7) as u8
    }

    /// The day of the year, 1..=366.
    pub fn ordinal(self) -> u16 {
        (self.to_days() - days_from_civil(self.year, 1, 1) + 1) as u16
    }

    /// `n` days later (earlier when negative); unchanged when that leaves the range.
    pub fn add_days(self, n: i64) -> Date {
        Date::from_days(self.to_days() + n).unwrap_or(self)
    }

    /// `n` months later, the day clamped to the target month's length; unchanged out of range.
    pub fn add_months(self, n: i64) -> Date {
        let idx = self.year as i64 * 12 + (self.month as i64 - 1) + n;
        let (y, m) = (idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u8);
        if !(MIN_YEAR as i64..=MAX_YEAR as i64).contains(&y) {
            return self;
        }
        let y = y as i32;
        Date::new(y, m, self.day.min(days_in_month(y, m))).unwrap_or(self)
    }

    /// The first day of this date's month.
    pub fn first_of_month(self) -> Date {
        Date { day: 1, ..self }
    }

    /// The last day of this date's month.
    pub fn last_of_month(self) -> Date {
        Date { day: days_in_month(self.year, self.month), ..self }
    }

    /// The ISO 8601 week-numbering year and week (1..=53): the week holding this date's Thursday.
    pub fn iso_week(self) -> (i32, u8) {
        let thursday = Date::from_days(self.to_days() + 3 - self.weekday() as i64).unwrap_or(self);
        (thursday.year, ((thursday.ordinal() - 1) / 7 + 1) as u8)
    }

    /// Parses ISO 8601 `YYYY-MM-DD`.
    pub fn parse_iso(s: &str) -> Result<Date, String> {
        let err = || format!("`{s}` is not an ISO 8601 date (YYYY-MM-DD)");
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return Err(err());
        }
        let num = |r: std::ops::Range<usize>| -> Option<u32> { s.get(r).filter(|t| t.bytes().all(|c| c.is_ascii_digit())).and_then(|t| t.parse().ok()) };
        let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else { return Err(err()) };
        Date::new(y as i32, m as u8, d as u8).ok_or_else(err)
    }

    /// Formats with `fmt`, `strftime` style: `%Y %y %m %d %e %B %b %h %A %a %j %u %w %V %G %F %D
    /// %%`. An unknown directive is an error (the date picker then shows ISO, as with jiff).
    pub fn strftime(self, fmt: &str) -> Result<String, String> {
        let mut out = String::new();
        let mut it = fmt.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let Some(d) = it.next() else { return Err("`%` at the end of the format".into()) };
            use std::fmt::Write;
            let (iy, iw) = self.iso_week();
            let _ = match d {
                'Y' => write!(out, "{:04}", self.year),
                'y' => write!(out, "{:02}", self.year % 100),
                'm' => write!(out, "{:02}", self.month),
                'd' => write!(out, "{:02}", self.day),
                'e' => write!(out, "{:>2}", self.day),
                'B' => write!(out, "{}", MONTHS[self.month as usize - 1]),
                'b' | 'h' => write!(out, "{}", &MONTHS[self.month as usize - 1][..3]),
                'A' => write!(out, "{}", WEEKDAYS[self.weekday() as usize]),
                'a' => write!(out, "{}", &WEEKDAYS[self.weekday() as usize][..3]),
                'j' => write!(out, "{:03}", self.ordinal()),
                'u' => write!(out, "{}", self.weekday() + 1),
                'w' => write!(out, "{}", (self.weekday() + 1) % 7),
                'V' => write!(out, "{iw:02}"),
                'G' => write!(out, "{iy:04}"),
                'F' => write!(out, "{self}"),
                'D' => write!(out, "{:02}/{:02}/{:02}", self.month, self.day, self.year % 100),
                '%' => write!(out, "%"),
                other => return Err(format!("unknown directive `%{other}`")),
            };
        }
        Ok(out)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A date prop: ISO 8601 or `""` (none). A write that is neither is refused.
pub fn parse_date_prop(s: &str) -> Result<Option<Date>, String> {
    let s = s.trim();
    if s.is_empty() { Ok(None) } else { Date::parse_iso(s).map(Some) }
}

/// A date read leniently (a bad string is none).
pub fn date_or_none(s: &str) -> Option<Date> {
    parse_date_prop(s).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse_iso(s).unwrap()
    }

    #[test]
    fn day_numbers_round_trip() {
        assert_eq!(d("1970-01-01").to_days(), 0);
        assert_eq!(d("2000-03-01").to_days(), 11_017);
        for n in [-719_162i64, -1, 0, 1, 11_016, 20_000, 2_932_896] {
            let x = Date::from_days(n).unwrap();
            assert_eq!(x.to_days(), n);
        }
        assert_eq!(Date::from_days(-719_163), None, "before 0001-01-01");
    }

    #[test]
    fn weekdays_and_iso_weeks() {
        assert_eq!(d("2026-10-01").weekday(), 3, "a Thursday");
        assert_eq!(d("2026-10-25").weekday(), 6, "a Sunday");
        assert_eq!(d("2021-01-03").iso_week(), (2020, 53));
        assert_eq!(d("2021-01-04").iso_week(), (2021, 1));
        assert_eq!(d("2026-12-31").iso_week(), (2026, 53));
        assert_eq!(d("2024-12-30").iso_week(), (2025, 1));
    }

    #[test]
    fn month_arithmetic_clamps_the_day() {
        assert_eq!(d("2026-01-31").add_months(1), d("2026-02-28"));
        assert_eq!(d("2024-01-31").add_months(1), d("2024-02-29"));
        assert_eq!(d("2026-03-15").add_months(-3), d("2025-12-15"));
        assert_eq!(d("2026-10-22").add_months(12), d("2027-10-22"));
        assert_eq!(d("9999-12-01").add_months(1), d("9999-12-01"), "out of range: unchanged");
        assert_eq!(d("2026-02-28").add_days(1), d("2026-03-01"));
        assert_eq!(d("2024-02-28").add_days(1), d("2024-02-29"));
    }

    #[test]
    fn iso_parse_and_format() {
        assert_eq!(d("2026-10-22").to_string(), "2026-10-22");
        assert!(Date::parse_iso("2026-02-30").is_err());
        assert!(Date::parse_iso("2026-1-01").is_err());
        assert!(Date::parse_iso("20261001").is_err());
        assert!(Date::parse_iso("+026-10-01").is_err());
        assert_eq!(parse_date_prop(""), Ok(None));
        assert_eq!(date_or_none("nope"), None);
    }

    #[test]
    fn strftime_subset() {
        let x = d("2026-10-02");
        assert_eq!(x.strftime("%Y-%m-%d").unwrap(), "2026-10-02");
        assert_eq!(x.strftime("%a %e %b %Y").unwrap(), "Fri  2 Oct 2026");
        assert_eq!(x.strftime("%A, %B %d (%j) W%V %%").unwrap(), "Friday, October 02 (275) W40 %");
        assert!(x.strftime("%Q").is_err());
    }
}
