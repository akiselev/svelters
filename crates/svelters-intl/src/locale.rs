//! Locales: parsing, canonical tags, negotiation against the data's locales, the current locale.

use crate::IntlError;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::sync::{Arc, RwLock};

/// A parsed BCP 47 locale (cheap to clone).
#[derive(Clone, PartialEq, Eq)]
pub struct Locale(Arc<Inner>);

#[derive(PartialEq, Eq)]
struct Inner {
    icu: icu_locale_core::Locale,
    tag: String,
}

impl Locale {
    /// Parses a well-formed BCP 47 tag (`de-DE`, `ja-JP-u-ca-japanese`) into its canonical form.
    pub fn parse(tag: &str) -> Result<Locale, IntlError> {
        let icu = icu_locale_core::Locale::try_from_str(tag).map_err(|_| IntlError::Tag(tag.to_string()))?;
        if icu.id.language.is_unknown() && !tag.eq_ignore_ascii_case("und") && !tag.to_ascii_lowercase().starts_with("und-") {
            return Err(IntlError::Tag(tag.to_string()));
        }
        Ok(Locale::from_icu(icu))
    }

    /// Like [`Locale::parse`], but also takes POSIX names (`de_DE.UTF-8`, `de_DE@euro`); for
    /// host settings, not for JS.
    pub fn parse_lenient(tag: &str) -> Result<Locale, IntlError> {
        let base = tag.split(['.', '@']).next().unwrap_or(tag).replace('_', "-");
        Locale::parse(&base).map_err(|_| IntlError::Tag(tag.to_string()))
    }

    pub(crate) fn from_icu(icu: icu_locale_core::Locale) -> Locale {
        let tag = icu.to_string();
        Locale(Arc::new(Inner { icu, tag }))
    }

    /// The canonical tag (`de-DE`).
    pub fn tag(&self) -> &str {
        &self.0.tag
    }

    /// The language subtag (`de`).
    pub fn language(&self) -> &str {
        self.0.icu.id.language.as_str()
    }

    pub(crate) fn icu(&self) -> &icu_locale_core::Locale {
        &self.0.icu
    }

    /// Whether the data has this locale's language (other regions and scripts of it fall back).
    pub fn is_supported(&self) -> bool {
        crate::data::data().locales.iter().any(|d| d.split('-').next() == Some(self.language()))
    }
}

impl fmt::Debug for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Locale({})", self.tag())
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// `Intl.getCanonicalLocales` for one tag: the canonical form, or `Tag` if malformed.
///
/// Case and subtag order are canonicalized; CLDR aliases (`iw` -> `he`) are not.
pub fn canonicalize(tag: &str) -> Result<String, IntlError> {
    Locale::parse(tag).map(|l| l.tag().to_string())
}

/// ECMA-402 lookup negotiation: the first requested locale the data supports, else the current
/// locale if supported, else `en`. A malformed tag is an error, as in `Intl` constructors.
pub fn resolve(requested: &[&str]) -> Result<Locale, IntlError> {
    let mut parsed = Vec::with_capacity(requested.len());
    for t in requested {
        parsed.push(Locale::parse(t)?);
    }
    if let Some(l) = parsed.into_iter().find(Locale::is_supported) {
        return Ok(l);
    }
    let cur = current();
    if cur.is_supported() {
        return Ok(cur);
    }
    Locale::parse("en")
}

thread_local! {
    static CURRENT: RefCell<Option<Locale>> = const { RefCell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

static PROCESS_DEFAULT: RwLock<Option<Locale>> = RwLock::new(None);

/// The current locale of this thread: the one [`set_current`] set, else the process default,
/// else [`system_locale`].
pub fn current() -> Locale {
    if let Some(l) = CURRENT.with(|c| c.borrow().clone()) {
        return l;
    }
    if let Some(l) = PROCESS_DEFAULT.read().ok().and_then(|g| g.clone()) {
        return l;
    }
    system_locale()
}

/// Sets this thread's current locale (the UI thread's: `Host::set_locale` calls it). Takes BCP 47
/// tags and POSIX names. Returns the parsed locale; bumps [`current_generation`].
pub fn set_current(tag: &str) -> Result<Locale, IntlError> {
    let l = Locale::parse_lenient(tag)?;
    CURRENT.with(|c| *c.borrow_mut() = Some(l.clone()));
    GENERATION.with(|g| g.set(g.get() + 1));
    Ok(l)
}

/// Counts [`set_current`] calls on this thread, so caches of formatted text can tell a change.
pub fn current_generation() -> u64 {
    GENERATION.with(Cell::get)
}

/// Sets the locale of threads that never called [`set_current`].
pub fn set_process_default(tag: &str) -> Result<Locale, IntlError> {
    let l = Locale::parse_lenient(tag)?;
    if let Ok(mut g) = PROCESS_DEFAULT.write() {
        *g = Some(l.clone());
    }
    Ok(l)
}

/// The locale of the environment: `LC_ALL`, then `LC_NUMERIC`, then `LANG` (POSIX names
/// accepted); `C`/`POSIX` and anything unset or malformed are `en-US`. On Windows and macOS GUI
/// launches these are usually unset, so embedders should pass their own locale.
pub fn system_locale() -> Locale {
    for var in ["LC_ALL", "LC_NUMERIC", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            if v == "C" || v == "POSIX" || v.starts_with("C.") {
                break;
            }
            if let Ok(l) = Locale::parse_lenient(v) {
                return l;
            }
        }
    }
    Locale::parse("en-US").expect("en-US parses")
}
