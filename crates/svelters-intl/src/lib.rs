//! svelters-intl (IN1): locale-aware number, plural, collation and date formatting over ICU4X.
//!
//! It backs `Intl` in the QuickJS host (`svelters-host`, feature `intl`) and the numeric widgets
//! of `svelters-egui`. svelters ships no translation catalog: the embedder owns the locale and its
//! strings, and sets the locale with [`set_current`] (through `Host::set_locale`).
//!
//! ```
//! use svelters_intl::{Locale, NumberOptions};
//! let de = Locale::parse("de-DE").unwrap();
//! assert_eq!(svelters_intl::format_number(&de, 1234.5, &NumberOptions::default()).unwrap(), "1.234,5");
//! assert_eq!(svelters_intl::parse_number(&de, "1,5"), Some(1.5));
//! ```
//!
//! **Data.** ICU4X data comes from one postcard blob for a locale subset ([`data_locales`]),
//! built in (`data/icu4x.postcard`, made by `tools/intl/gen-data.sh`), or chosen by the embedder
//! with [`use_data`] before the first call. A locale outside the subset resolves to the default
//! locale (as `Intl`'s locale negotiation does), never to missing data.
//!
//! **Threads.** The current locale is per thread: [`set_current`] sets it for the UI thread (the
//! one the host and the egui backend run on); other threads see [`set_process_default`], else the
//! system locale. Formatters are cached per thread.

mod collate;
mod data;
mod date;
mod locale;
mod number;
mod parts;
mod plural;

pub use collate::{compare, CaseFirst, CollatorOptions, Sensitivity};
pub use data::{data_locales, use_data};
pub use date::{format_date, format_date_to_parts, DateOptions, DateStyle, HourCycle, MonthStyle, NumericStyle, TextStyle, TimeZoneNameStyle, Zone};
pub use locale::{canonicalize, current, current_generation, resolve, set_current, set_process_default, system_locale, Locale};
pub use number::{
    format_number, format_number_to_parts, parse_number, CurrencyDisplay, Grouping, Notation, NumberOptions, NumberStyle, SignDisplay,
};
pub use parts::Part;
pub use plural::{plural_categories, plural_category, PluralOptions};

use std::fmt;

/// An error from this crate. JS maps `Tag` and `Range` to `RangeError`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntlError {
    /// Not a well-formed BCP 47 language tag.
    Tag(String),
    /// An option out of range, or a value that cannot be formatted.
    Range(String),
    /// ICU4X data missing or invalid.
    Data(String),
}

impl fmt::Display for IntlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntlError::Tag(t) => write!(f, "Incorrect locale information provided: {t}"),
            IntlError::Range(m) => f.write_str(m),
            IntlError::Data(m) => write!(f, "ICU4X data: {m}"),
        }
    }
}

impl std::error::Error for IntlError {}

impl From<icu_provider::DataError> for IntlError {
    fn from(e: icu_provider::DataError) -> Self {
        IntlError::Data(e.to_string())
    }
}

#[cfg(test)]
mod tests;
