//! Collation: `Intl.Collator` and `String.prototype.localeCompare`.

use crate::data::data;
use crate::{IntlError, Locale};
use icu_collator::options::{AlternateHandling, CaseLevel, CollatorOptions as IcuOptions, Strength};
use icu_collator::preferences::{CollationCaseFirst, CollationNumericOrdering};
use icu_collator::{Collator, CollatorPreferences};
use icu_provider::buf::AsDeserializingBufferProvider;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

/// `sensitivity`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sensitivity {
    /// a = á = A
    Base,
    /// a = A, a ≠ á
    Accent,
    /// a = á, a ≠ A
    Case,
    /// a ≠ á ≠ A
    #[default]
    Variant,
}

/// `caseFirst`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaseFirst {
    Upper,
    Lower,
    Off,
}

/// `Intl.Collator` options (`usage: "sort"`; `"search"` collates the same here).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CollatorOptions {
    pub sensitivity: Sensitivity,
    pub numeric: bool,
    pub ignore_punctuation: bool,
    pub case_first: Option<CaseFirst>,
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, CollatorOptions), Rc<Collator>>> = RefCell::new(HashMap::new());
}

fn collator(locale: &Locale, o: &CollatorOptions) -> Result<Rc<Collator>, IntlError> {
    let key = (locale.tag().to_string(), o.clone());
    if let Some(c) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(c);
    }
    let mut prefs = CollatorPreferences::from(locale.icu());
    if o.numeric {
        prefs.numeric_ordering = Some(CollationNumericOrdering::True);
    }
    if let Some(cf) = o.case_first {
        prefs.case_first = Some(match cf {
            CaseFirst::Upper => CollationCaseFirst::Upper,
            CaseFirst::Lower => CollationCaseFirst::Lower,
            CaseFirst::Off => CollationCaseFirst::False,
        });
    }
    let mut io = IcuOptions::default();
    let (strength, case_level) = match o.sensitivity {
        Sensitivity::Base => (Strength::Primary, CaseLevel::Off),
        Sensitivity::Accent => (Strength::Secondary, CaseLevel::Off),
        Sensitivity::Case => (Strength::Primary, CaseLevel::On),
        Sensitivity::Variant => (Strength::Tertiary, CaseLevel::Off),
    };
    io.strength = Some(strength);
    io.case_level = Some(case_level);
    if o.ignore_punctuation {
        io.alternate_handling = Some(AlternateHandling::Shifted);
    }
    let c = Rc::new(Collator::try_new_unstable(&data().provider.as_deserializing(), prefs, io)?);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, c.clone());
    });
    Ok(c)
}

/// `Intl.Collator.prototype.compare`.
pub fn compare(locale: &Locale, a: &str, b: &str, o: &CollatorOptions) -> Result<Ordering, IntlError> {
    Ok(collator(locale, o)?.as_borrowed().compare(a, b))
}
