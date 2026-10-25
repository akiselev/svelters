//! The value boxes: number and quantity spinners (`drag-value`'s field, `slider`'s and
//! `range-slider`'s value boxes, cadatomic's `quantity-field`), as a [`TextField`] that edits a
//! number. Ported from svelters-egui's `widgets/value_box.rs` (its entry and commit seam),
//! `widgets/numfmt.rs` (display and parsing, the formatter/parser registry) and `interact/step.rs`
//! (the step arithmetic); the interim `Entry` buffer is replaced by quire-text.
//!
//! Signals follow ARCH-WIDGETS §2.4 and FM1 QW7's commit seam: [`NumberSignal::Input`] for every
//! user change of the value, [`NumberSignal::Commit`] at a commit point (Enter or focus lost after
//! typing, every step) when the value differs from where that interaction began, and
//! [`NumberSignal::Submit`] on Enter. In commit mode (Qt `keyboardTracking=false`) typing does not
//! write the value until the commit.
//!
//! The scrub (DragToValue), the step buttons (Pressable auto-repeat) and the hover/focus state bits
//! are the interaction units' (`crate::interact`); the kind composes them with this field.
//!
//! Number display without the `intl` locale is egui 0.37's `emath::format_with_decimals_in_range`
//! (MIT OR Apache-2.0, Copyright (c) 2018-2021 Emil Ernerfeldt), as svelters-egui's `numeric.rs`
//! uses it.

use super::editor::{EditorConfig, FocusReason};
use super::field::{FieldSignal, TextField};
use super::keys::KeyAction;
use super::policy::TextPolicy;
use super::shim::Layouter;
use skelter_base::event::{KeyEvent, KeyState, Modifiers};
use skelter_base::input::InputPurpose;
use skelter_base::key::Key;
use skelter_base::time::Time;
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

/// Registers formatter `name` (`formatter="name"`; Qt's `textFromValue`); replaces an earlier one.
/// Rust-only: Svelte names a function the host registered and cannot add one.
pub fn register_formatter(name: &str, f: impl Fn(f64, std::ops::RangeInclusive<usize>) -> String + Send + Sync + 'static) {
    let mut g = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(Registry::default).formatters.insert(name.to_string(), Arc::new(f));
}

/// Registers parser `name` (`parser="name"`; Qt's `valueFromText`); replaces an earlier one.
pub fn register_parser(name: &str, f: impl Fn(&str) -> Option<f64> + Send + Sync + 'static) {
    let mut g = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(Registry::default).parsers.insert(name.to_string(), Arc::new(f));
}

/// `value` with the fewest decimals in `decimals` that show it (egui's default formatter).
pub fn format_decimals(value: f64, decimals: std::ops::RangeInclusive<usize>) -> String {
    let max = (*decimals.end()).min(16);
    let min = (*decimals.start()).min(max);
    fn almost_equal(a: f32, b: f32, epsilon: f32) -> bool {
        if a == b {
            true
        } else {
            let abs_max = a.abs().max(b.abs());
            abs_max <= epsilon || ((a - b).abs() / abs_max) <= epsilon
        }
    }
    if min < max {
        for d in min..max {
            let text = format!("{value:.d$}");
            if text.parse::<f32>().is_ok_and(|t| almost_equal(t, value as f32, 16.0 * f32::EPSILON)) {
                return text;
            }
        }
    }
    format!("{value:.max$}")
}

/// Reads a typed number: whitespace ignored, `−` (U+2212) as minus, `.` as the decimal point.
pub fn parse_number(text: &str) -> Option<f64> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).map(|c| if c == '\u{2212}' { '-' } else { c }).collect();
    t.parse().ok()
}

/// The decimals of `step` (`0.25` → 2), at most 15.
pub fn decimals_of(step: f64) -> usize {
    if !(step.is_finite() && step > 0.0) {
        return 0;
    }
    (0..=15)
        .find(|&d| {
            let f = 10f64.powi(d as i32);
            ((step * f).round() - step * f).abs() < 1e-9 * f.max(1.0)
        })
        .unwrap_or(15)
}

/// How a box shows and reads its number.
#[derive(Clone, Debug, PartialEq)]
pub struct NumFormat {
    pub prefix: String,
    pub suffix: String,
    /// Decimals shown: the fewest in `min..=max` that show the value.
    pub decimals: (usize, usize),
    /// Shown instead of the number at `min` (Qt `specialValueText`); empty for none.
    pub special: String,
    pub min: f64,
    /// Registered formatter and parser names; empty for the default notation.
    pub formatter: String,
    pub parser: String,
}

impl Default for NumFormat {
    fn default() -> Self {
        NumFormat { prefix: String::new(), suffix: String::new(), decimals: (0, 6), special: String::new(), min: f64::NEG_INFINITY, formatter: String::new(), parser: String::new() }
    }
}

impl NumFormat {
    /// The number's text (no affixes, no special text): what the field edits.
    pub fn number(&self, v: f64) -> String {
        let (lo, hi) = self.decimals;
        let hi = hi.max(lo);
        if !self.formatter.is_empty()
            && let Some(f) = with(|r| r.formatters.get(&self.formatter).cloned())
        {
            return f(v, lo..=hi);
        }
        format_decimals(v, lo..=hi)
    }

    /// The special text applies at `v`.
    pub fn is_special(&self, v: f64) -> bool {
        !self.special.is_empty() && v == self.min
    }

    /// What the box shows while not editing: the special text at `min`, else prefix, number, suffix.
    pub fn display(&self, v: f64) -> String {
        if self.is_special(v) {
            return self.special.clone();
        }
        format!("{}{}{}", self.prefix, self.number(v), self.suffix)
    }

    /// Reads typed text: the special text is `min`; the affixes may be typed or left out; then the
    /// registered parser, else [`parse_number`].
    pub fn parse(&self, text: &str) -> Option<f64> {
        let t = text.trim();
        if !self.special.is_empty() && t == self.special.trim() {
            return Some(self.min);
        }
        let t = t.strip_prefix(self.prefix.trim()).unwrap_or(t).trim();
        let t = t.strip_suffix(self.suffix.trim()).unwrap_or(t).trim();
        if !self.parser.is_empty()
            && let Some(p) = with(|r| r.parsers.get(&self.parser).cloned())
        {
            return p(t).filter(|v| !v.is_nan());
        }
        parse_number(t).filter(|v| !v.is_nan())
    }
}

/// What an out-of-range typed value commits to (Qt `correctionMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Correction {
    /// The nearest bound (Qt CorrectToNearestValue).
    #[default]
    Nearest,
    /// The value before the edit (Qt CorrectToPreviousValue).
    Previous,
}

/// The value's bounds and steps (the arithmetic of svelters-egui's `StepParams`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steps {
    /// Bounds; infinite (or NaN) for none.
    pub min: f64,
    pub max: f64,
    /// One step (Up/Down, a wheel notch); ≤ 0 is 1% of a finite range, else 1.
    pub step: f64,
    /// PageUp/PageDown in value units; ≤ 0 is ten steps.
    pub page: f64,
    /// Stepping past one end goes to the other (Qt `wrapping`).
    pub wrap: bool,
    /// Qt AdaptiveDecimalStepType: a step is one decade below the value's magnitude.
    pub adaptive: bool,
    /// Rounding of results: fixed decimals, else 10 places (`0.1 + 0.2` is `0.3`).
    pub decimals: Option<usize>,
}

impl Default for Steps {
    fn default() -> Self {
        Steps { min: f64::NEG_INFINITY, max: f64::INFINITY, step: 1.0, page: 0.0, wrap: false, adaptive: false, decimals: None }
    }
}

impl Steps {
    pub fn step_at(&self, v: f64, up: bool) -> f64 {
        let base = if self.step > 0.0 && self.step.is_finite() {
            self.step
        } else if (self.max - self.min).is_finite() && self.max > self.min {
            (self.max - self.min) / 100.0
        } else {
            1.0
        };
        if !self.adaptive || !v.is_finite() || v == 0.0 {
            return base;
        }
        let a = v.abs();
        let mut e = a.log10().floor();
        if !up && (a - 10f64.powf(e)).abs() < 1e-12 * a.max(1.0) {
            e -= 1.0;
        }
        10f64.powf(e - 1.0).max(base)
    }

    pub fn page_size(&self, v: f64, up: bool) -> f64 {
        if self.page > 0.0 && self.page.is_finite() { self.page } else { 10.0 * self.step_at(v, up) }
    }

    pub fn round(&self, v: f64) -> f64 {
        let d = self.decimals.unwrap_or(10).min(15) as i32;
        let f = 10f64.powi(d);
        let r = (v * f).round() / f;
        if r.is_finite() { r } else { v }
    }

    pub fn clamp(&self, v: f64) -> f64 {
        let mut v = v;
        if !self.min.is_nan() && v < self.min {
            v = self.min;
        }
        if !self.max.is_nan() && v > self.max {
            v = self.max;
        }
        v
    }

    pub fn out_of_range(&self, v: f64) -> bool {
        (!self.min.is_nan() && v < self.min) || (!self.max.is_nan() && v > self.max)
    }

    /// `n` steps (or pages) from `v`: rounded, then wrapped or clamped.
    pub fn by(&self, v: f64, n: i32, page: bool) -> f64 {
        let up = n > 0;
        let mut x = v;
        for _ in 0..n.unsigned_abs().min(10_000) {
            let d = if page { self.page_size(x, up) } else { self.step_at(x, up) };
            x = self.round(if up { x + d } else { x - d });
        }
        if self.wrap && self.min.is_finite() && self.max.is_finite() {
            if x > self.max {
                x = if v >= self.max { self.min } else { self.max };
            } else if x < self.min {
                x = if v <= self.min { self.max } else { self.min };
            }
        }
        self.clamp(self.round(x))
    }
}

/// What the box tells its kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumberSignal {
    Input(f64),
    Commit(f64),
    Submit(f64),
}

/// A number field. See the module docs.
pub struct NumberField {
    pub field: TextField,
    pub fmt: NumFormat,
    pub steps: Steps,
    pub correction: Correction,
    /// Qt `keyboardTracking=false`: typing writes the value only at the commit.
    pub commit_mode: bool,
    value: f64,
    /// The value when the edit began (a commit compares with it).
    from: f64,
    editing: bool,
    signals: Vec<NumberSignal>,
}

impl NumberField {
    pub fn new(value: f64, policy: TextPolicy) -> Self {
        let fmt = NumFormat::default();
        let cfg = EditorConfig { purpose: InputPurpose::Number, ..EditorConfig::default() };
        let field = TextField::new(&fmt.display(value), cfg, policy);
        NumberField { field, fmt, steps: Steps::default(), correction: Correction::Nearest, commit_mode: false, value, from: value, editing: false, signals: Vec::new() }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn editing(&self) -> bool {
        self.editing
    }

    pub fn take_signals(&mut self) -> Vec<NumberSignal> {
        std::mem::take(&mut self.signals)
    }

    /// The `value` prop. While editing an untouched text follows it; a typed text is kept.
    pub fn set_value(&mut self, v: f64) {
        if v == self.value || v.is_nan() {
            return;
        }
        self.value = v;
        if !self.editing {
            self.field.set_text(&self.fmt.display(v));
        } else if !self.dirty() {
            self.restart_edit(v);
        }
    }

    /// Shows the current value again (after a format change).
    pub fn refresh(&mut self) {
        if self.editing {
            self.restart_edit(self.value);
        } else {
            self.field.set_text(&self.fmt.display(self.value));
        }
    }

    fn dirty(&self) -> bool {
        self.editing && self.field.text() != self.fmt.number(self.from)
    }

    /// Replaces the text with `v`'s number, all selected (a step or Enter while editing).
    fn restart_edit(&mut self, v: f64) {
        self.field.set_text(&self.fmt.number(v));
        let len = self.field.text().len();
        self.field.editor.set_selection(0, len);
        self.from = v;
    }

    /// A typed text's value: rounded and clamped, or `None` (not a number, or out of range under
    /// `Correction::Previous`).
    fn parse(&self, text: &str) -> Option<f64> {
        let v = self.fmt.parse(text)?;
        if self.steps.out_of_range(v) && self.correction == Correction::Previous {
            return None;
        }
        Some(self.steps.clamp(self.steps.round(v)))
    }

    fn write(&mut self, v: f64) {
        if v != self.value {
            self.value = v;
            self.signals.push(NumberSignal::Input(v));
        }
    }

    fn commit_point(&mut self, v: f64) {
        self.write(v);
        if v != self.from {
            self.signals.push(NumberSignal::Commit(v));
        }
        self.from = v;
    }

    pub fn focus_in(&mut self, reason: FocusReason) {
        self.editing = !self.field.editor.config().read_only;
        self.field.focus_in(reason);
        if self.editing {
            self.restart_edit(self.value);
        }
        self.field.take_signals();
    }

    /// Focus left: an edited text commits; the box shows the formatted value again.
    pub fn focus_out(&mut self) {
        if self.dirty()
            && let Some(v) = self.parse(&self.field.text())
        {
            self.commit_point(v);
        }
        self.editing = false;
        self.field.focus_out();
        self.field.take_signals();
        self.field.set_text(&self.fmt.display(self.value));
    }

    /// `n` steps (pages): a commit point. While the text is edited, steps go from the typed value.
    pub fn step(&mut self, n: i32, page: bool) {
        if self.field.editor.config().read_only {
            return;
        }
        let base = if self.dirty() { self.parse(&self.field.text()).unwrap_or(self.value) } else { self.value };
        let v = self.steps.by(base, n, page);
        self.commit_point(v);
        if self.editing {
            self.restart_edit(v);
        } else {
            self.field.set_text(&self.fmt.display(v));
        }
    }

    /// A wheel notch (positive away from the user steps up); the kind calls it only under its wheel
    /// rule (FM1: while focused).
    pub fn wheel(&mut self, notches: i32) {
        if notches != 0 {
            self.step(notches, false);
        }
    }

    /// A key while focused: steps on Up/Down/PageUp/PageDown, applies on Enter, restores on Escape,
    /// and edits the text otherwise.
    pub fn key(&mut self, ev: &KeyEvent, now: Time, lay: &mut dyn Layouter) -> KeyAction {
        if ev.state == KeyState::Pressed && !self.field.editor.composing() {
            let m = ev.modifiers;
            let plain = !(m.contains(Modifiers::CTRL) || m.contains(Modifiers::ALT) || m.contains(Modifiers::SUPER));
            let step = match ev.key {
                Some(Key::ArrowUp) if plain => Some((1, false)),
                Some(Key::ArrowDown) if plain => Some((-1, false)),
                Some(Key::PageUp) if plain => Some((1, true)),
                Some(Key::PageDown) if plain => Some((-1, true)),
                _ => None,
            };
            if let Some((n, page)) = step {
                self.step(n, page);
                return KeyAction::Consumed;
            }
        }
        let action = self.field.key(ev, now, lay);
        let edited = self.field.take_signals().contains(&FieldSignal::Changed);
        match action {
            KeyAction::Submit => {
                let v = if self.dirty() { self.parse(&self.field.text()).unwrap_or(self.value) } else { self.value };
                self.commit_point(v);
                self.signals.push(NumberSignal::Submit(v));
                if self.editing {
                    self.restart_edit(v);
                }
            }
            KeyAction::Cancel => {
                // the text and the value from before the edit
                let from = self.from;
                self.write(from);
                if self.editing {
                    self.restart_edit(from);
                }
            }
            _ if edited && !self.commit_mode => {
                if let Some(v) = self.parse(&self.field.text()) {
                    self.write(v);
                }
            }
            _ => {}
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::policy::Platform;
    use crate::text::shim::MonoLayouter;
    use skelter_base::event::KeyLocation;

    fn ev(k: Key, text: Option<&str>) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() }
    }

    fn typ(n: &mut NumberField, lay: &mut MonoLayouter, s: &str) {
        for c in s.chars() {
            let t = c.to_string();
            n.key(&ev(Key::A, Some(&t)), Time(0), lay);
        }
    }

    #[test]
    fn format_and_parse() {
        assert_eq!(format_decimals(2.5, 0..=6), "2.5");
        assert_eq!(format_decimals(2.0, 0..=6), "2");
        assert_eq!(format_decimals(1.0 / 3.0, 0..=3), "0.333");
        assert_eq!(parse_number(" \u{2212}1 234.5 "), Some(-1234.5));
        assert_eq!((decimals_of(0.25), decimals_of(1.0), decimals_of(0.1)), (2, 0, 1));
        let f = NumFormat { prefix: "x: ".into(), suffix: " mm".into(), special: "Auto".into(), min: 0.0, decimals: (1, 1), ..NumFormat::default() };
        assert_eq!(f.display(0.0), "Auto");
        assert_eq!(f.display(2.0), "x: 2.0 mm");
        assert_eq!(f.parse("Auto"), Some(0.0));
        assert_eq!(f.parse("x: 2.5 mm"), Some(2.5));
        assert_eq!(f.parse("2.5"), Some(2.5));
        register_formatter("text-input-test-hex", |v, _| format!("{:x}", v as i64));
        register_parser("text-input-test-hex", |t| i64::from_str_radix(t, 16).ok().map(|v| v as f64));
        let h = NumFormat { formatter: "text-input-test-hex".into(), parser: "text-input-test-hex".into(), ..NumFormat::default() };
        assert_eq!(h.display(255.0), "ff");
        assert_eq!(h.parse("1f"), Some(31.0));
    }

    #[test]
    fn step_arithmetic() {
        let s = Steps { min: 0.0, max: 1.0, step: 0.1, ..Steps::default() };
        assert_eq!(s.by(0.2, 1, false), 0.3);
        assert_eq!(s.by(0.95, 1, false), 1.0);
        let w = Steps { wrap: true, ..s };
        assert_eq!(w.by(1.0, 1, false), 0.0);
        assert_eq!(s.by(0.5, -1, true), 0.0);
        let a = Steps { adaptive: true, step: 0.01, ..Steps::default() };
        assert_eq!(a.step_at(250.0, true), 10.0);
        assert_eq!(a.step_at(100.0, false), 1.0);
    }

    #[test]
    fn typing_steps_and_commit_seam() {
        let mut lay = MonoLayouter { advance: 8.0, line_height: 16.0 };
        let mut n = NumberField::new(5.0, TextPolicy::of(Platform::Qt));
        n.steps = Steps { min: 0.0, max: 10.0, ..Steps::default() };
        n.fmt.suffix = " mm".into();
        n.refresh();
        assert_eq!(n.field.text(), "5 mm");
        n.focus_in(FocusReason::Keyboard);
        assert_eq!(n.field.text(), "5", "editing shows the bare number, all selected");
        typ(&mut n, &mut lay, "7");
        assert_eq!(n.value(), 7.0);
        assert_eq!(n.take_signals(), vec![NumberSignal::Input(7.0)]);
        n.key(&ev(Key::ArrowUp, None), Time(0), &mut lay);
        assert_eq!(n.value(), 8.0);
        assert_eq!(n.take_signals(), vec![NumberSignal::Input(8.0), NumberSignal::Commit(8.0)]);
        assert_eq!(n.field.text(), "8");
        typ(&mut n, &mut lay, "42");
        assert_eq!(n.value(), 10.0, "clamped to max (Correction::Nearest)");
        n.key(&ev(Key::Escape, None), Time(0), &mut lay);
        assert_eq!((n.value(), n.field.text().as_str()), (8.0, "8"), "Escape restores");
        n.take_signals();
        typ(&mut n, &mut lay, "3");
        n.key(&ev(Key::Enter, Some("\r")), Time(0), &mut lay);
        assert_eq!(n.take_signals(), vec![NumberSignal::Input(3.0), NumberSignal::Commit(3.0), NumberSignal::Submit(3.0)]);
        n.focus_out();
        assert_eq!(n.field.text(), "3 mm");
        assert!(n.take_signals().is_empty(), "nothing new to commit on blur");
    }

    #[test]
    fn commit_mode_writes_only_at_the_commit() {
        let mut lay = MonoLayouter { advance: 8.0, line_height: 16.0 };
        let mut n = NumberField::new(1.0, TextPolicy::of(Platform::Gtk));
        n.commit_mode = true;
        n.focus_in(FocusReason::Keyboard);
        typ(&mut n, &mut lay, "9");
        assert_eq!(n.value(), 1.0);
        assert!(n.take_signals().is_empty());
        n.focus_out();
        assert_eq!(n.take_signals(), vec![NumberSignal::Input(9.0), NumberSignal::Commit(9.0)]);
        n.correction = Correction::Previous;
        n.steps.max = 5.0;
        n.focus_in(FocusReason::Keyboard);
        typ(&mut n, &mut lay, "7");
        n.focus_out();
        assert_eq!(n.value(), 9.0, "out of range under Correction::Previous keeps the value");
    }
}
