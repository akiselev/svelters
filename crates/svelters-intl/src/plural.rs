//! Plurals: `Intl.PluralRules` (cardinal and ordinal).

use crate::data::data;
use crate::number::NumberOptions;
use crate::{IntlError, Locale};
use fixed_decimal::{Decimal, FloatPrecision, SignedRoundingMode, UnsignedRoundingMode};
use icu_plurals::{PluralCategory, PluralRules};
use icu_provider::buf::AsDeserializingBufferProvider;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `Intl.PluralRules` options. The digit options shape the operands as `NumberFormat` would
/// (`1.0` with `minimumFractionDigits: 1` is `other` in English).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PluralOptions {
    pub ordinal: bool,
    pub minimum_fraction_digits: Option<u8>,
    pub maximum_fraction_digits: Option<u8>,
}

impl PluralOptions {
    pub fn cardinal() -> PluralOptions {
        PluralOptions::default()
    }

    pub fn ordinal() -> PluralOptions {
        PluralOptions { ordinal: true, ..PluralOptions::default() }
    }
}

thread_local! {
    static RULES: RefCell<HashMap<(String, bool), Rc<PluralRules>>> = RefCell::new(HashMap::new());
}

fn rules(locale: &Locale, ordinal: bool) -> Result<Rc<PluralRules>, IntlError> {
    let key = (locale.tag().to_string(), ordinal);
    if let Some(r) = RULES.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(r);
    }
    let p = &data().provider.as_deserializing();
    let r = Rc::new(if ordinal {
        PluralRules::try_new_ordinal_unstable(p, locale.icu().into())?
    } else {
        PluralRules::try_new_cardinal_unstable(p, locale.icu().into())?
    });
    RULES.with(|c| c.borrow_mut().insert(key, r.clone()));
    Ok(r)
}

fn name(c: PluralCategory) -> &'static str {
    match c {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        _ => "other",
    }
}

/// `Intl.PluralRules.prototype.select`: `"zero" | "one" | "two" | "few" | "many" | "other"`.
pub fn plural_category(locale: &Locale, n: f64, o: &PluralOptions) -> Result<&'static str, IntlError> {
    if !n.is_finite() {
        return Ok("other");
    }
    let r = rules(locale, o.ordinal)?;
    let mut d = Decimal::try_from_f64(n.abs(), FloatPrecision::RoundTrip).map_err(|_| IntlError::Range(format!("cannot select {n}")))?;
    let mut no = NumberOptions::default();
    no.minimum_fraction_digits = o.minimum_fraction_digits;
    no.maximum_fraction_digits = o.maximum_fraction_digits;
    if let Some((min, max)) = no.resolved_fraction_digits()? {
        d.round_with_mode(-(max as i16), SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand));
        d.absolute.trim_end();
        d.absolute.pad_end(-(min as i16));
    }
    Ok(name(r.category_for(&d)))
}

/// `resolvedOptions().pluralCategories`, in ECMA-402's order (zero, one, two, few, many, other).
pub fn plural_categories(locale: &Locale, ordinal: bool) -> Result<Vec<&'static str>, IntlError> {
    let r = rules(locale, ordinal)?;
    let have: Vec<PluralCategory> = r.categories().collect();
    Ok([PluralCategory::Zero, PluralCategory::One, PluralCategory::Two, PluralCategory::Few, PluralCategory::Many, PluralCategory::Other]
        .into_iter()
        .filter(|c| have.contains(c))
        .map(name)
        .collect())
}
