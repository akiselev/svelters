//! OW1b: how the numeric widgets (`slider`'s and `range-slider`'s value boxes, `drag-value`) show
//! and read numbers: the current locale (IN1, `crate::numeric`), decimals, group separators,
//! `prefix`/`suffix`, Qt's `specialValueText`, and the `formatter`/`parser` registry (Qt's
//! `textFromValue`/`valueFromText`: named Rust functions an app registers, so cadatomic's
//! quantity fields format units and read expressions without JS per keystroke).
//!
//! Security (deny by default, ARCH-SECURITY): the registry is Rust-only; Svelte names a function
//! the host registered and cannot add one. Formatters and parsers are pure text functions.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A formatter: the value and the decimals the widget would show (`min..=max`) to its text
/// (without `prefix`/`suffix`).
pub type Formatter = Arc<dyn Fn(f64, std::ops::RangeInclusive<usize>) -> String + Send + Sync>;
/// A parser: typed text (without `prefix`/`suffix`) to a value, `None` if it is not one.
pub type Parser = Arc<dyn Fn(&str) -> Option<f64> + Send + Sync>;

#[derive(Default)]
struct Registry {
    formatters: HashMap<String, Formatter>,
    parsers: HashMap<String, Parser>,
}

static REGISTRY: RwLock<Option<Registry>> = RwLock::new(None);

fn with<R>(f: impl FnOnce(&Registry) -> R) -> R {
    let g = REGISTRY.read().unwrap_or_else(|e| e.into_inner());
    match g.as_ref() {
        Some(r) => f(r),
        None => f(&Registry::default()),
    }
}

/// Registers formatter `name` (`<drag-value formatter="name">`); replaces an earlier one.
pub fn register_formatter(name: &str, f: impl Fn(f64, std::ops::RangeInclusive<usize>) -> String + Send + Sync + 'static) {
    let mut g = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(Registry::default).formatters.insert(name.to_string(), Arc::new(f));
}

/// Registers parser `name` (`<drag-value parser="name">`); replaces an earlier one.
pub fn register_parser(name: &str, f: impl Fn(&str) -> Option<f64> + Send + Sync + 'static) {
    let mut g = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(Registry::default).parsers.insert(name.to_string(), Arc::new(f));
}

/// The number format of one widget.
#[derive(Clone, Copy, Debug)]
pub struct NumFormat<'a> {
    pub prefix: &'a str,
    pub suffix: &'a str,
    /// Decimals shown: fewest in `min..=max` that show the value.
    pub decimals: (usize, usize),
    /// The locale's group separators.
    pub grouping: bool,
    /// Shown instead of the number at `min` (Qt `specialValueText`); empty for none.
    pub special: &'a str,
    pub min: f64,
    /// Registered formatter and parser names; empty for the locale's.
    pub formatter: &'a str,
    pub parser: &'a str,
}

impl Default for NumFormat<'_> {
    fn default() -> Self {
        NumFormat { prefix: "", suffix: "", decimals: (0, 6), grouping: true, special: "", min: f64::NEG_INFINITY, formatter: "", parser: "" }
    }
}

/// The decimals of `step` (`0.25` → 2), at most 15.
pub fn decimals_of(step: f64) -> usize {
    if !(step.is_finite() && step > 0.0) {
        return 0;
    }
    (0..=15).find(|&d| {
        let f = 10f64.powi(d as i32);
        ((step * f).round() - step * f).abs() < 1e-9 * f.max(1.0)
    })
    .unwrap_or(15)
}

/// Decimals so that one `per_point` of value (a point dragged, a pixel of track) shows (egui's
/// choice: `ceil(log10(0.5 / per_point))`).
pub fn auto_decimals(per_point: f64) -> usize {
    if !(per_point.is_finite() && per_point > 0.0) {
        return 0;
    }
    (0.5 / per_point).log10().ceil().clamp(0.0, 15.0) as usize
}

impl NumFormat<'_> {
    /// The number's text (no `prefix`/`suffix`, no special text).
    pub fn number(&self, v: f64) -> String {
        let (lo, hi) = self.decimals;
        let hi = hi.max(lo);
        if !self.formatter.is_empty()
            && let Some(f) = with(|r| r.formatters.get(self.formatter).cloned())
        {
            return f(v, lo..=hi);
        }
        crate::numeric::format_grouped(v, lo..=hi, self.grouping)
    }

    /// The special text applies at `v`.
    pub fn is_special(&self, v: f64) -> bool {
        !self.special.is_empty() && v == self.min
    }

    /// What the widget shows: the special text at `min`, else `prefix` + number + `suffix`.
    pub fn display(&self, v: f64) -> String {
        if self.is_special(v) {
            return self.special.to_string();
        }
        format!("{}{}{}", self.prefix, self.number(v), self.suffix)
    }

    /// Reads typed text: the special text is `min`; `prefix`/`suffix` may be typed or left out;
    /// then the registered parser, else the locale's notation (IN1), else egui's.
    pub fn parse(&self, text: &str) -> Option<f64> {
        let t = text.trim();
        if !self.special.is_empty() && t == self.special.trim() {
            return Some(self.min);
        }
        let t = t.strip_prefix(self.prefix.trim()).unwrap_or(t).trim();
        let t = t.strip_suffix(self.suffix.trim()).unwrap_or(t).trim();
        if !self.parser.is_empty()
            && let Some(p) = with(|r| r.parsers.get(self.parser).cloned())
        {
            return p(t).filter(|v| !v.is_nan());
        }
        crate::numeric::parse(t).filter(|v| !v.is_nan())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ow1b_decimals_special_text_affixes_and_registry() {
        assert_eq!((decimals_of(0.25), decimals_of(1.0), decimals_of(0.1)), (2, 0, 1));
        assert_eq!(auto_decimals(1.0), 0);
        assert_eq!(auto_decimals(0.01), 2);
        let f = NumFormat { prefix: "x: ", suffix: " mm", special: "Auto", min: 0.0, decimals: (1, 1), grouping: false, ..NumFormat::default() };
        assert_eq!(f.display(0.0), "Auto");
        assert_eq!(f.parse("Auto"), Some(0.0));
        assert_eq!(f.parse("x: 2.5 mm"), Some(2.5));
        assert_eq!(f.parse("2.5"), Some(2.5));
        register_formatter("ow1b-test-hex", |v, _| format!("{:x}", v as i64));
        register_parser("ow1b-test-hex", |t| i64::from_str_radix(t, 16).ok().map(|v| v as f64));
        let h = NumFormat { formatter: "ow1b-test-hex", parser: "ow1b-test-hex", ..NumFormat::default() };
        assert_eq!(h.display(255.0), "ff");
        assert_eq!(h.parse("1f"), Some(31.0));
    }
}
