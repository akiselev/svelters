//! Numbers: `Intl.NumberFormat` (decimal, percent, currency; standard and compact notation),
//! and the inverse parse the numeric widgets use.

use crate::data::data;
use crate::parts::{self, Part};
use crate::{IntlError, Locale};
use fixed_decimal::{Decimal, FloatPrecision, SignedRoundingMode, UnsignedRoundingMode};
use icu_decimal::options::{CompactDecimalFormatterOptions, DecimalFormatterOptions, GroupingStrategy};
use icu_decimal::{CompactDecimalFormatter, DecimalFormatter};
use icu_experimental::dimension::currency::formatter::{CurrencyFormatter, CurrencyFormatterPreferences};
use icu_experimental::dimension::currency::options::CurrencyFormatterOptions;
use icu_experimental::dimension::percent::formatter::{PercentFormatter, PercentFormatterPreferences};
use icu_locale_core::preferences::extensions::unicode::keywords::CurrencyType;
use icu_provider::buf::AsDeserializingBufferProvider;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumberStyle {
    #[default]
    Decimal,
    Percent,
    Currency,
}

/// `currencyDisplay`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CurrencyDisplay {
    #[default]
    Symbol,
    NarrowSymbol,
    Code,
    Name,
}

/// `useGrouping`: `"auto"` (true), `"always"`, `"min2"`, false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Grouping {
    #[default]
    Auto,
    Always,
    Min2,
    Off,
}

/// `signDisplay`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SignDisplay {
    #[default]
    Auto,
    Never,
    Always,
    ExceptZero,
    Negative,
}

/// `notation` (`compactDisplay` folded in). Scientific and engineering are not supported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Notation {
    #[default]
    Standard,
    CompactShort,
    CompactLong,
}

/// `Intl.NumberFormat` options. Digit options left `None` take ECMA-402's defaults for the style.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct NumberOptions {
    pub style: NumberStyle,
    /// ISO 4217 code; required for [`NumberStyle::Currency`].
    pub currency: Option<String>,
    pub currency_display: CurrencyDisplay,
    pub minimum_integer_digits: u8,
    pub minimum_fraction_digits: Option<u8>,
    pub maximum_fraction_digits: Option<u8>,
    pub minimum_significant_digits: Option<u8>,
    pub maximum_significant_digits: Option<u8>,
    pub use_grouping: Option<Grouping>,
    pub sign_display: SignDisplay,
    pub notation: Notation,
}

impl Default for NumberOptions {
    fn default() -> Self {
        NumberOptions {
            style: NumberStyle::Decimal,
            currency: None,
            currency_display: CurrencyDisplay::Symbol,
            minimum_integer_digits: 1,
            minimum_fraction_digits: None,
            maximum_fraction_digits: None,
            minimum_significant_digits: None,
            maximum_significant_digits: None,
            use_grouping: None,
            sign_display: SignDisplay::Auto,
            notation: Notation::Standard,
        }
    }
}

impl NumberOptions {
    /// Fixed fraction digits (`min = max = n`), the shape numeric widgets use.
    pub fn fraction_digits(n: u8) -> NumberOptions {
        NumberOptions { minimum_fraction_digits: Some(n), maximum_fraction_digits: Some(n), ..NumberOptions::default() }
    }

    /// The fraction digits ECMA-402 resolves: `(min, max)`, or `None` for compact notation's own
    /// rounding (no digit option given) and for significant digits.
    pub fn resolved_fraction_digits(&self) -> Result<Option<(u8, u8)>, IntlError> {
        if self.minimum_significant_digits.is_some() || self.maximum_significant_digits.is_some() {
            return Ok(None);
        }
        if self.notation != Notation::Standard && self.minimum_fraction_digits.is_none() && self.maximum_fraction_digits.is_none() {
            return Ok(None);
        }
        let (dmin, dmax) = match self.style {
            NumberStyle::Decimal => (0, 3),
            NumberStyle::Percent => (0, 0),
            NumberStyle::Currency => {
                let d = currency_digits(self.currency.as_deref().unwrap_or(""));
                (d, d)
            }
        };
        let (min, max) = match (self.minimum_fraction_digits, self.maximum_fraction_digits) {
            (None, None) => (dmin, dmax),
            (Some(min), None) => (min, min.max(dmax)),
            (None, Some(max)) => (dmin.min(max), max),
            (Some(min), Some(max)) if min > max => return Err(IntlError::Range("maximumFractionDigits value is out of range.".into())),
            (Some(min), Some(max)) => (min, max),
        };
        if max > 100 {
            return Err(IntlError::Range("maximumFractionDigits value is out of range.".into()));
        }
        Ok(Some((min, max)))
    }

    fn grouping(&self) -> GroupingStrategy {
        let g = self.use_grouping.unwrap_or(if self.notation == Notation::Standard { Grouping::Auto } else { Grouping::Min2 });
        match g {
            Grouping::Auto => GroupingStrategy::Auto,
            Grouping::Always => GroupingStrategy::Always,
            Grouping::Min2 => GroupingStrategy::Min2,
            Grouping::Off => GroupingStrategy::Never,
        }
    }
}

/// ISO 4217 minor units for the currencies that do not use 2.
fn currency_digits(code: &str) -> u8 {
    match code.to_ascii_uppercase().as_str() {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX" | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        _ => 2,
    }
}

enum Fmt {
    Decimal(DecimalFormatter),
    Percent(PercentFormatter<DecimalFormatter>),
    Currency(CurrencyFormatter<DecimalFormatter>),
    Compact(CompactDecimalFormatter),
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, NumberOptions), Rc<Fmt>>> = RefCell::new(HashMap::new());
}

const CACHE_MAX: usize = 64;

fn formatter(locale: &Locale, o: &NumberOptions) -> Result<Rc<Fmt>, IntlError> {
    let key = (locale.tag().to_string(), o.clone());
    if let Some(f) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(f);
    }
    let p = &data().provider.as_deserializing();
    let loc = locale.icu();
    let mut dopts = DecimalFormatterOptions::default();
    dopts.grouping_strategy = Some(o.grouping());
    let f = match (o.style, o.notation) {
        (NumberStyle::Currency, _) => {
            let code = o.currency.as_deref().ok_or_else(|| IntlError::Range("Currency code is required with currency style.".into()))?;
            let cur = CurrencyType::try_from_str(code).map_err(|_| IntlError::Range(format!("Invalid currency code : {code}")))?;
            let prefs = CurrencyFormatterPreferences::from(loc);
            let copts = CurrencyFormatterOptions::default();
            Fmt::Currency(match o.currency_display {
                CurrencyDisplay::Symbol => CurrencyFormatter::try_new_symbol_unstable(p, prefs, cur, copts)?,
                CurrencyDisplay::NarrowSymbol => CurrencyFormatter::try_new_symbol_narrow_unstable(p, prefs, cur, copts)?,
                CurrencyDisplay::Code => CurrencyFormatter::try_new_code_unstable(p, prefs, cur, copts)?,
                CurrencyDisplay::Name => CurrencyFormatter::try_new_name_unstable(p, prefs, cur)?,
            })
        }
        (NumberStyle::Percent, _) => {
            let df = DecimalFormatter::try_new_unstable(p, loc.into(), dopts)?;
            Fmt::Percent(PercentFormatter::try_new_with_decimal_formatter_unstable(p, PercentFormatterPreferences::from(loc), df, Default::default())?)
        }
        (NumberStyle::Decimal, Notation::Standard) => Fmt::Decimal(DecimalFormatter::try_new_unstable(p, loc.into(), dopts)?),
        (NumberStyle::Decimal, n) => {
            let mut copts = CompactDecimalFormatterOptions::default();
            copts.grouping_strategy = Some(o.grouping());
            Fmt::Compact(if n == Notation::CompactLong {
                CompactDecimalFormatter::try_new_long_unstable(p, loc.into(), copts)?
            } else {
                CompactDecimalFormatter::try_new_short_unstable(p, loc.into(), copts)?
            })
        }
    };
    let f = Rc::new(f);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= CACHE_MAX {
            c.clear();
        }
        c.insert(key, f.clone());
    });
    Ok(f)
}

/// Rounds and pads `x` (already scaled for percent) as the options say.
fn to_decimal(x: f64, o: &NumberOptions) -> Result<Decimal, IntlError> {
    let mut d = Decimal::try_from_f64(x, FloatPrecision::RoundTrip).map_err(|_| IntlError::Range(format!("cannot format {x}")))?;
    if o.style == NumberStyle::Percent {
        d.multiply_pow10(2);
    }
    let half = SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand);
    if o.minimum_significant_digits.is_some() || o.maximum_significant_digits.is_some() {
        let min = o.minimum_significant_digits.unwrap_or(1).clamp(1, 21) as i16;
        let max = o.maximum_significant_digits.unwrap_or(21).clamp(1, 21) as i16;
        if min > max {
            return Err(IntlError::Range("maximumSignificantDigits value is out of range.".into()));
        }
        if !d.absolute.is_zero() {
            let top = d.absolute.nonzero_magnitude_start();
            d.round_with_mode(top - max + 1, half);
            d.absolute.trim_end();
            let top = d.absolute.nonzero_magnitude_start();
            d.absolute.pad_end(top - min + 1);
        } else {
            d.absolute.pad_end(-(min - 1));
        }
    } else if let Some((min, max)) = o.resolved_fraction_digits()? {
        d.round_with_mode(-(max as i16), half);
        d.absolute.trim_end();
        d.absolute.pad_end(-(min as i16));
    }
    d.absolute.pad_start(o.minimum_integer_digits.clamp(1, 21) as i16);
    d.apply_sign_display(match o.sign_display {
        SignDisplay::Auto => fixed_decimal::SignDisplay::Auto,
        SignDisplay::Never => fixed_decimal::SignDisplay::Never,
        SignDisplay::Always => fixed_decimal::SignDisplay::Always,
        SignDisplay::ExceptZero => fixed_decimal::SignDisplay::ExceptZero,
        SignDisplay::Negative => fixed_decimal::SignDisplay::Negative,
    });
    Ok(d)
}

/// `Intl.NumberFormat.prototype.format`.
pub fn format_number(locale: &Locale, x: f64, o: &NumberOptions) -> Result<String, IntlError> {
    format_number_to_parts(locale, x, o).map(|p| parts::join(&p))
}

/// `Intl.NumberFormat.prototype.formatToParts`.
pub fn format_number_to_parts(locale: &Locale, x: f64, o: &NumberOptions) -> Result<Vec<Part>, IntlError> {
    if !x.is_finite() {
        return Ok(non_finite(x, o));
    }
    let f = formatter(locale, o)?;
    let d = to_decimal(x, o)?;
    let raw = match &*f {
        Fmt::Decimal(f) => parts::collect(&f.format(&d)),
        Fmt::Compact(f) => parts::split_literals(parts::collect(&f.format(&d)), "compact"),
        Fmt::Percent(f) => parts::split_literals(parts::collect(&f.format(&d)), "percentSign"),
        Fmt::Currency(f) => parts::split_literals(parts::collect(&f.format_fixed_decimal(&d)), "currency"),
    };
    Ok(raw)
}

fn non_finite(x: f64, o: &NumberOptions) -> Vec<Part> {
    let mut out = Vec::new();
    let neg = x.is_sign_negative() && !x.is_nan();
    let sign = match o.sign_display {
        SignDisplay::Never => None,
        SignDisplay::Always | SignDisplay::ExceptZero if !x.is_nan() => Some(if neg { "-" } else { "+" }),
        SignDisplay::Always => Some("+"),
        _ if neg => Some("-"),
        _ => None,
    };
    if let Some(s) = sign {
        out.push(Part { kind: if s == "-" { "minusSign" } else { "plusSign" }.into(), value: s.into() });
    }
    if x.is_nan() {
        out.push(Part { kind: "nan".into(), value: "NaN".into() });
    } else {
        out.push(Part { kind: "infinity".into(), value: "∞".into() });
    }
    out
}

/// The symbols of a locale's standard decimal format, read back from formatted text.
#[derive(Clone, Debug)]
struct Symbols {
    decimal: String,
    group: String,
    minus: String,
    /// The locale's digits 0-9 (ASCII for Latin numbering).
    digits: [char; 10],
}

thread_local! {
    static SYMBOLS: RefCell<HashMap<String, Rc<Symbols>>> = RefCell::new(HashMap::new());
}

fn symbols(locale: &Locale) -> Result<Rc<Symbols>, IntlError> {
    if let Some(s) = SYMBOLS.with(|c| c.borrow().get(locale.tag()).cloned()) {
        return Ok(s);
    }
    let mut o = NumberOptions::fraction_digits(1);
    o.use_grouping = Some(Grouping::Always);
    let ps = format_number_to_parts(locale, -1234567.5, &o)?;
    let pick = |k: &str| ps.iter().find(|p| p.kind == k).map(|p| p.value.clone()).unwrap_or_default();
    let mut digits = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
    let mut o = NumberOptions::default();
    o.use_grouping = Some(Grouping::Off);
    let ds: Vec<char> = format_number(locale, 1234567890.0, &o)?.chars().collect();
    if ds.len() == 10 {
        // "1234567890": digit n is at index n-1, zero last
        for (n, slot) in digits.iter_mut().enumerate() {
            *slot = ds[(n + 9) % 10];
        }
    }
    let s = Rc::new(Symbols { decimal: pick("decimal"), group: pick("group"), minus: pick("minusSign"), digits });
    SYMBOLS.with(|c| c.borrow_mut().insert(locale.tag().to_string(), s.clone()));
    Ok(s)
}

/// Parses a number typed in `locale`'s notation (`"1.234,5"` and `"1,5"` in de-DE).
///
/// Accepts the locale's digits, decimal separator, grouping separator (and any space where the
/// locale groups with a space), its minus sign as well as `-` and `+`, and an exponent `e`. When
/// the locale's decimal separator is not `.` and the text has none, a single `.` not followed by
/// exactly three digits is read as the decimal point (`1.5` is 1.5 in de-DE, `1.500` is 1500).
/// Returns `None` for anything else.
pub fn parse_number(locale: &Locale, text: &str) -> Option<f64> {
    let s = symbols(locale).ok()?;
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let space_group = s.group.chars().all(|c| c.is_whitespace() || c == '\u{202f}' || c == '\u{a0}');
    let has_decimal = !s.decimal.is_empty() && t.contains(s.decimal.as_str());
    let dot_is_decimal = !has_decimal && s.decimal != "." && {
        let dots: Vec<usize> = t.match_indices('.').map(|(i, _)| i).collect();
        dots.len() == 1 && t[dots[0] + 1..].chars().take_while(|c| !c.is_ascii_punctuation()).filter(|c| c.is_ascii_digit()).count() != 3
    };
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(ch) = rest.chars().next() {
        if !s.decimal.is_empty() && rest.starts_with(s.decimal.as_str()) {
            out.push('.');
            rest = &rest[s.decimal.len()..];
            continue;
        }
        if dot_is_decimal && rest.starts_with('.') {
            out.push('.');
            rest = &rest[1..];
            continue;
        }
        if !s.group.is_empty() && rest.starts_with(s.group.as_str()) {
            rest = &rest[s.group.len()..];
            continue;
        }
        if !s.minus.is_empty() && rest.starts_with(s.minus.as_str()) {
            out.push('-');
            rest = &rest[s.minus.len()..];
            continue;
        }
        rest = &rest[ch.len_utf8()..];
        if let Some(n) = s.digits.iter().position(|&d| d == ch) {
            out.push((b'0' + n as u8) as char);
        } else if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == 'e' || ch == 'E' {
            out.push(ch);
        } else if ch == '\u{2212}' {
            out.push('-');
        } else if ch == '.' && s.group == "." {
            // a grouping dot the locale writes as "." (de-DE)
        } else if (ch.is_whitespace() || ch == '\u{202f}' || ch == '\u{a0}') && (space_group || out.is_empty() || rest.is_empty()) {
        } else if (ch == '\'' || ch == '\u{2019}') && (s.group == "\u{2019}" || s.group == "'") {
        } else {
            return None;
        }
    }
    if !out.bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    out.parse::<f64>().ok()
}
