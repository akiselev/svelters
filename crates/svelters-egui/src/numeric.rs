//! IN1: locale-aware number display and entry for egui's numeric widgets (`slider`'s value box,
//! `drag-value`, `range-slider`'s value boxes): the decimal separator and grouping of the current locale
//! (`svelters_intl::current`, set by `Host::set_locale`), and parsing of what users type in it
//! (`1,5` is 1.5 under de-DE).
//!
//! Without the `intl` feature these are identity wrappers: egui's own `1234.5` notation.
//!
//! Writer lane: IN1 (the widget files call [`slider`] / [`drag_value`] and nothing else).

use egui::{DragValue, RangeSlider, Slider};
use std::ops::RangeInclusive;

/// Formats `value` as egui's default formatter would (the fewest decimals in `decimals` that show
/// it), in the current locale's notation.
pub fn format(value: f64, decimals: RangeInclusive<usize>) -> String {
    format_grouped(value, decimals, true)
}

/// [`format`] with the locale's group separators on or off (OW1b: `drag-value`'s
/// `group-separator`, Qt's `showGroupSeparator`).
pub fn format_grouped(value: f64, decimals: RangeInclusive<usize>, grouping: bool) -> String {
    let ascii = egui::emath::format_with_decimals_in_range(value, decimals);
    #[cfg(feature = "intl")]
    {
        let d = ascii.split_once('.').map_or(0, |(_, f)| f.len()).min(20) as u8;
        let mut o = svelters_intl::NumberOptions::fraction_digits(d);
        if !grouping {
            o.use_grouping = Some(svelters_intl::Grouping::Off);
        }
        if value.is_finite()
            && let Ok(s) = svelters_intl::format_number(&svelters_intl::current(), value, &o)
        {
            return s;
        }
    }
    let _ = grouping;
    ascii
}

/// Parses a number typed in the current locale's notation; when that fails, egui's own reading
/// (whitespace ignored, `−` as minus, `.` as the decimal point).
pub fn parse(text: &str) -> Option<f64> {
    #[cfg(feature = "intl")]
    if let Some(v) = svelters_intl::parse_number(&svelters_intl::current(), text) {
        return Some(v);
    }
    let t: String = text.chars().filter(|c| !c.is_whitespace()).map(|c| if c == '\u{2212}' { '-' } else { c }).collect();
    t.parse().ok()
}

/// `slider` with locale-aware value text and entry.
pub fn slider(s: Slider<'_>) -> Slider<'_> {
    if cfg!(feature = "intl") { s.custom_formatter(format).custom_parser(parse) } else { s }
}

/// `drag-value` (and spin fields) with locale-aware text and entry.
pub fn drag_value(d: DragValue<'_>) -> DragValue<'_> {
    if cfg!(feature = "intl") { d.custom_formatter(format).custom_parser(parse) } else { d }
}

/// `range-slider` with locale-aware value boxes.
pub fn range_slider(s: RangeSlider<'_>) -> RangeSlider<'_> {
    if cfg!(feature = "intl") { s.custom_formatter(format).custom_parser(parse) } else { s }
}
