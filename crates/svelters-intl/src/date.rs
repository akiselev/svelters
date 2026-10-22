//! Dates: `Intl.DateTimeFormat` (`dateStyle`/`timeStyle` and the component options) for UTC and
//! fixed-offset zones (the host's local zone is passed as its offset at the instant).

use crate::data::data;
use crate::parts::{self, Part};
use crate::{IntlError, Locale};
use icu_datetime::fieldbag::field;
use icu_datetime::fieldbag::DateTimeFieldBag;
use icu_datetime::fieldsets::builder::{DateFields, FieldSetBuilder, ZoneStyle};
use icu_datetime::fieldsets::enums::CompositeFieldSet;
use icu_datetime::options::{Length, TimePrecision};
use icu_datetime::{DateTimeFormatter, DateTimeFormatterPreferences};
use icu_locale_core::preferences::extensions::unicode::keywords::{CalendarAlgorithm, HourCycle as IcuHourCycle};
use icu_provider::buf::AsDeserializingBufferProvider;
use icu_time::zone::{TimeZoneInfo, UtcOffset};
use icu_time::{DateTime, TimeZone, ZonedDateTime};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `dateStyle` / `timeStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateStyle {
    Full,
    Long,
    Medium,
    Short,
}

/// `weekday`, `era`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextStyle {
    Narrow,
    Short,
    Long,
}

/// `year`, `day`, `hour`, `minute`, `second`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumericStyle {
    Numeric,
    TwoDigit,
}

/// `month`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonthStyle {
    Numeric,
    TwoDigit,
    Narrow,
    Short,
    Long,
}

/// `timeZoneName`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeZoneNameStyle {
    Short,
    Long,
    ShortOffset,
    LongOffset,
    ShortGeneric,
    LongGeneric,
}

/// `hourCycle` (`hour12: true` is `H12`, false `H23`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HourCycle {
    H11,
    H12,
    H23,
}

/// `Intl.DateTimeFormat` options. With neither styles nor components, year/month/day numeric.
/// Styles and components together are an error, as in ECMA-402.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DateOptions {
    pub date_style: Option<DateStyle>,
    pub time_style: Option<DateStyle>,
    pub weekday: Option<TextStyle>,
    pub era: Option<TextStyle>,
    pub year: Option<NumericStyle>,
    pub month: Option<MonthStyle>,
    pub day: Option<NumericStyle>,
    pub hour: Option<NumericStyle>,
    pub minute: Option<NumericStyle>,
    pub second: Option<NumericStyle>,
    /// 1 to 3.
    pub fractional_second_digits: Option<u8>,
    pub time_zone_name: Option<TimeZoneNameStyle>,
    pub hour_cycle: Option<HourCycle>,
}

impl DateOptions {
    fn has_components(&self) -> bool {
        self.weekday.is_some()
            || self.era.is_some()
            || self.year.is_some()
            || self.month.is_some()
            || self.day.is_some()
            || self.hour.is_some()
            || self.minute.is_some()
            || self.second.is_some()
            || self.fractional_second_digits.is_some()
    }

    fn field_set(&self) -> Result<CompositeFieldSet, IntlError> {
        let styled = self.date_style.is_some() || self.time_style.is_some();
        if styled && (self.has_components() || self.time_zone_name.is_some()) {
            return Err(IntlError::Range("Can't set option dateStyle/timeStyle together with other date-time component options".into()));
        }
        if styled {
            let len = |s: DateStyle| match s {
                DateStyle::Full | DateStyle::Long => Length::Long,
                DateStyle::Medium => Length::Medium,
                DateStyle::Short => Length::Short,
            };
            let mut b = FieldSetBuilder::new();
            b.length = Some(len(self.date_style.or(self.time_style).unwrap_or(DateStyle::Medium)));
            b.date_fields = self.date_style.map(|s| if s == DateStyle::Full { DateFields::YMDE } else { DateFields::YMD });
            b.time_precision = self.time_style.map(|s| if s == DateStyle::Short { TimePrecision::Minute } else { TimePrecision::Second });
            b.zone_style = match self.time_style {
                Some(DateStyle::Full) => Some(ZoneStyle::SpecificLong),
                Some(DateStyle::Long) => Some(ZoneStyle::SpecificShort),
                _ => None,
            };
            return b.build_composite().map_err(|e| IntlError::Range(format!("unsupported date style combination: {e:?}")));
        }
        let mut bag = DateTimeFieldBag::default();
        bag.weekday = self.weekday.map(|w| match w {
            TextStyle::Narrow => field::Weekday::Narrow,
            TextStyle::Short => field::Weekday::Short,
            TextStyle::Long => field::Weekday::Long,
        });
        bag.era = self.era.map(|w| match w {
            TextStyle::Narrow => field::Era::Narrow,
            TextStyle::Short => field::Era::Short,
            TextStyle::Long => field::Era::Long,
        });
        bag.year = self.year.map(|n| match n {
            NumericStyle::Numeric => field::Year::Numeric,
            NumericStyle::TwoDigit => field::Year::TwoDigit,
        });
        bag.month = self.month.map(|m| match m {
            MonthStyle::Numeric => field::Month::Numeric,
            MonthStyle::TwoDigit => field::Month::TwoDigit,
            MonthStyle::Narrow => field::Month::Narrow,
            MonthStyle::Short => field::Month::Short,
            MonthStyle::Long => field::Month::Long,
        });
        bag.day = self.day.map(|n| match n {
            NumericStyle::Numeric => field::Day::Numeric,
            NumericStyle::TwoDigit => field::Day::TwoDigit,
        });
        bag.hour = self.hour.map(|n| match n {
            NumericStyle::Numeric => field::Hour::Numeric,
            NumericStyle::TwoDigit => field::Hour::TwoDigit,
        });
        bag.minute = self.minute.map(|n| match n {
            NumericStyle::Numeric => field::Minute::Numeric,
            NumericStyle::TwoDigit => field::Minute::TwoDigit,
        });
        bag.second = self.second.map(|n| match n {
            NumericStyle::Numeric => field::Second::Numeric,
            NumericStyle::TwoDigit => field::Second::TwoDigit,
        });
        bag.fractional_second_digits = match self.fractional_second_digits {
            None => None,
            Some(1) => Some(field::FractionalSecondDigits::F1),
            Some(2) => Some(field::FractionalSecondDigits::F2),
            Some(3) => Some(field::FractionalSecondDigits::F3),
            Some(_) => return Err(IntlError::Range("fractionalSecondDigits value is out of range.".into())),
        };
        bag.time_zone_name = self.time_zone_name.map(|z| match z {
            TimeZoneNameStyle::Short => field::TimeZoneName::ShortSpecific,
            TimeZoneNameStyle::Long => field::TimeZoneName::LongSpecific,
            TimeZoneNameStyle::ShortOffset => field::TimeZoneName::ShortOffset,
            TimeZoneNameStyle::LongOffset => field::TimeZoneName::LongOffset,
            TimeZoneNameStyle::ShortGeneric => field::TimeZoneName::ShortGeneric,
            TimeZoneNameStyle::LongGeneric => field::TimeZoneName::LongGeneric,
        });
        if !self.has_components() {
            bag.year = Some(field::Year::Numeric);
            bag.month = Some(field::Month::Numeric);
            bag.day = Some(field::Day::Numeric);
        }
        bag.to_field_set_builder().build_composite().map_err(|e| IntlError::Range(format!("unsupported date-time component combination: {e:?}")))
    }
}

/// The time zone to format in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Utc,
    /// A zone known only by its offset from UTC at the instant, in seconds east (the host's local
    /// zone, or `"+05:30"`); its name formats as `GMT+5:30`.
    Offset(i32),
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, DateOptions), Rc<DateTimeFormatter<CompositeFieldSet>>>> = RefCell::new(HashMap::new());
}

fn formatter(locale: &Locale, o: &DateOptions) -> Result<Rc<DateTimeFormatter<CompositeFieldSet>>, IntlError> {
    let key = (locale.tag().to_string(), o.clone());
    if let Some(f) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(f);
    }
    let mut prefs = DateTimeFormatterPreferences::from(locale.icu());
    // The data has the Gregorian calendar only (tools/intl/gen-data.sh): `-u-ca-*` is ignored.
    prefs.calendar_algorithm = Some(CalendarAlgorithm::Gregory);
    if let Some(hc) = o.hour_cycle {
        prefs.hour_cycle = Some(match hc {
            HourCycle::H11 => IcuHourCycle::H11,
            HourCycle::H12 => IcuHourCycle::H12,
            HourCycle::H23 => IcuHourCycle::H23,
        });
    }
    let f = DateTimeFormatter::try_new_unstable(&data().provider.as_deserializing(), prefs, o.field_set()?)
        .map_err(|e| IntlError::Data(e.to_string()))?;
    let f = Rc::new(f);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= 32 {
            c.clear();
        }
        c.insert(key, f.clone());
    });
    Ok(f)
}

/// Milliseconds since the epoch ECMAScript dates can hold.
const MAX_MS: f64 = 8.64e15;

/// `Intl.DateTimeFormat.prototype.formatToParts` for the instant `epoch_ms` in `zone`.
pub fn format_date_to_parts(locale: &Locale, epoch_ms: f64, zone: Zone, o: &DateOptions) -> Result<Vec<Part>, IntlError> {
    if !epoch_ms.is_finite() || epoch_ms.abs() > MAX_MS {
        return Err(IntlError::Range("Invalid time value".into()));
    }
    let f = formatter(locale, o)?;
    let secs = match zone {
        Zone::Utc => 0,
        Zone::Offset(s) => s.clamp(-18 * 3600, 18 * 3600),
    };
    let offset = UtcOffset::from_seconds_unchecked(secs);
    let local = ZonedDateTime::from_epoch_milliseconds_and_utc_offset(epoch_ms.floor() as i64, offset);
    let base = match zone {
        Zone::Utc => TimeZoneInfo::utc(),
        Zone::Offset(_) => TimeZone::UNKNOWN.with_offset(Some(offset)),
    };
    let zone = base.at_date_time(DateTime { date: local.date, time: local.time });
    let zoned = ZonedDateTime { date: local.date, time: local.time, zone };
    Ok(parts::collect(&f.format(&zoned)))
}

/// `Intl.DateTimeFormat.prototype.format`.
pub fn format_date(locale: &Locale, epoch_ms: f64, zone: Zone, o: &DateOptions) -> Result<String, IntlError> {
    format_date_to_parts(locale, epoch_ms, zone, o).map(|p| parts::join(&p))
}
