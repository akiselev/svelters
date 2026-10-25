//! The pickers as presto-core kinds: the `KindSpec`s of `calendar`, `color-editor`, `date-picker` and
//! `color-picker` (built from the rows in [`super::spec`]), and the `Kind` impls of the two inline
//! ones, `calendar` and `color-editor`, over their models ([`Calendar`], [`ColorEditor`]).
//!
//! - Props are parsed into the model in `prop_changed`; a value the model cannot take (a malformed
//!   date or colour) is rejected with [`PropError::Rejected`].
//! - `prepare` runs the model's record code once with [`PrepareText`] so every text is shaped, and
//!   `record` draws it through [`record_with`] (presto-core shapes only in `prepare`).
//! - `event` turns presto-core's pointer and key events into the model's; the model's [`Emit`]s go
//!   out through `EventCx::emit` (several arguments as a `Record` named by the spec rows), and the
//!   bindable props (`value`, `custom-colors`) are written back as native-owned state.
//! - "Today" is [`Date::today_utc`]: presto has no time-zone database yet.
//!
//! Left out: the `Kind` impls of `date-picker` and `color-picker` (their popups are layers in the
//! window's overlay stack, lane WIDGETS-VIEWS' and presto-core's), the colour editor's hex field
//! typing (lane TEXT-INPUT's field), and Tab between the colour editor's inner stops (presto-core's
//! focus model has no sub-node stops; focus lands on the first stop).
//!
//! [`PrepareText`]: crate::table::core_text::PrepareText
//! [`record_with`]: crate::table::core_text::record_with

use super::calendar::{CalCfg, Calendar, FIRST_DAY, WEEKDAY_FORMAT, grid_days};
use super::color::ALPHA;
use super::color_editor::{COLOR_COMMIT_MODE, ColorBody, ColorCfg, ColorEditor};
use super::date::{Date, parse_date_prop};
use super::date_picker::DATE_COMMIT_MODE;
use super::spec::{self as rows, KindRows};
use super::PickerLook;
use crate::table::core_text::{PrepareText, TextCache, record_with};
use crate::table::shim::{self, Emit};
use presto_base::accesskit::{self, Role};
use presto_base::change::ChangeClass::{Layout, Paint};
use presto_base::display::DisplayList;
use presto_base::event::{KeyState, PointerEvent, PointerPhase};
use presto_base::geom::Vec2;
use presto_core::{
    A11yCx, Event, EventCx, FocusReason, Handled, Kind, KindRegistry, KindSpec, MeasureCx, PointerInput, PrepareCx, PropCx, PropDefault, PropError, PropId, PropSpec, RecordCx, SignalFlags, SignalSpec, StateMask,
    Value, ValueType,
};
use presto_core::registry::RegistryError;
use presto_core::taffy;
use std::sync::Arc;

const STR: ValueType = ValueType::Str;

const fn ps(name: &'static str, ty: ValueType, change: presto_base::change::ChangeClass) -> PropSpec {
    PropSpec::new(name, ty, change)
}

const VALUE_DATE: PropSpec = ps("value", STR, Paint).default(PropDefault::Str("")).native();
const CAL_MIN: PropSpec = ps("min", STR, Paint).default(PropDefault::Str(""));
const CAL_MAX: PropSpec = ps("max", STR, Paint).default(PropDefault::Str(""));
const FIRST: PropSpec = ps("first-day", ValueType::Enum, Paint).enum_values(FIRST_DAY).default(PropDefault::Enum(0));
const READ_ONLY: PropSpec = ps("read-only", ValueType::Bool, Paint).default(PropDefault::Bool(false));
const DAY_PARTS: PropSpec = ps("day-parts", ValueType::List, Paint);
const FIELDS: PropSpec = ps("fields", STR, Layout).default(PropDefault::Str("hex rgb"));
const SWATCHES: PropSpec = ps("swatches", ValueType::List, Layout);
const CUSTOM: PropSpec = ps("custom-colors", ValueType::List, Layout).native();
const VALUE_COLOR: PropSpec = ps("value", STR, Paint).default(PropDefault::Str("#000000")).native();

const SIG_CHANGED: SignalSpec = SignalSpec::new("changed", Some(STR), SignalFlags::NONE);
const SIG_COMMIT: SignalSpec = SignalSpec::new("commit", Some(STR), SignalFlags::NONE);
const SIG_PAGE: SignalSpec = SignalSpec::new("page-changed", Some(ValueType::Record), SignalFlags::NONE);

/// `calendar`: an inline month calendar.
pub static CALENDAR: KindSpec = KindSpec {
    props: &[
        VALUE_DATE,
        ps("calendar-week", ValueType::Bool, Layout).default(PropDefault::Bool(true)),
        CAL_MIN,
        CAL_MAX,
        FIRST,
        ps("weekday-format", ValueType::Enum, Layout).enum_values(WEEKDAY_FORMAT).default(PropDefault::Enum(1)),
        ps("navigation", ValueType::Bool, Layout).default(PropDefault::Bool(true)),
        READ_ONLY,
        DAY_PARTS,
    ],
    signals: &[SIG_CHANGED, SIG_COMMIT, SIG_PAGE],
    role: Role::Grid,
    focusable: true,
    ..KindSpec::new("calendar")
};

/// `date-picker`: a drop button with a calendar popup (spec only; see the module doc).
pub static DATE_PICKER: KindSpec = KindSpec {
    props: &[
        ps("value", STR, Layout).default(PropDefault::Str("")).native(),
        ps("format", STR, Layout).default(PropDefault::Str("%Y-%m-%d")),
        ps("calendar-week", ValueType::Bool, Paint).default(PropDefault::Bool(true)),
        CAL_MIN,
        CAL_MAX,
        FIRST,
        ps("weekday-format", ValueType::Enum, Paint).enum_values(WEEKDAY_FORMAT).default(PropDefault::Enum(1)),
        ps("navigation", ValueType::Bool, Paint).default(PropDefault::Bool(true)),
        READ_ONLY,
        DAY_PARTS,
        ps("commit-mode", ValueType::Enum, Paint).enum_values(DATE_COMMIT_MODE).default(PropDefault::Enum(0)),
    ],
    signals: &[SIG_CHANGED, SIG_COMMIT, SIG_PAGE],
    role: Role::DateInput,
    focusable: true,
    ..KindSpec::new("date-picker")
};

/// `color-editor`: an inline colour editor.
pub static COLOR_EDITOR: KindSpec = KindSpec {
    props: &[VALUE_COLOR, ps("alpha", ValueType::Enum, Layout).enum_values(ALPHA).default(PropDefault::Enum(0)), FIELDS, SWATCHES, CUSTOM],
    signals: &[SIG_CHANGED, SIG_COMMIT],
    role: Role::Group,
    focusable: true,
    ..KindSpec::new("color-editor")
};

/// `color-picker`: a drop button with a colour editor popup (spec only; see the module doc).
pub static COLOR_PICKER: KindSpec = KindSpec {
    props: &[
        VALUE_COLOR,
        ps("alpha", ValueType::Enum, Paint).enum_values(ALPHA).default(PropDefault::Enum(0)),
        FIELDS,
        SWATCHES,
        CUSTOM,
        ps("commit-mode", ValueType::Enum, Paint).enum_values(COLOR_COMMIT_MODE).default(PropDefault::Enum(0)),
    ],
    signals: &[SIG_CHANGED, SIG_COMMIT],
    role: Role::ColorWell,
    focusable: true,
    ..KindSpec::new("color-picker")
};

/// Registers the kinds whose `Kind` impls exist: `calendar` and `color-editor`.
pub fn register(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    reg.register::<CalendarKind>()?;
    reg.register::<ColorEditorKind>()?;
    Ok(())
}

/// A shim value as a presto-core value.
pub fn to_value(v: &shim::Value) -> Value {
    match v {
        shim::Value::Bool(b) => Value::Bool(*b),
        shim::Value::I64(i) => Value::I64(*i),
        shim::Value::F64(f) => Value::F64(*f),
        shim::Value::Str(s) => Value::Str(Arc::from(s.as_str())),
        shim::Value::List(l) => Value::List(l.iter().map(to_value).collect()),
    }
}

/// The argument of `emit` as its signal carries it: none, the one value, or a `Record` with the
/// argument names of the kind's spec rows.
pub fn emit_arg(kind: &KindRows, emit: &Emit) -> Option<Value> {
    match emit.args.as_slice() {
        [] => None,
        [one] => Some(to_value(one)),
        many => {
            let names = kind.signals.iter().find(|s| s.name == emit.signal).map(|s| s.args).unwrap_or(&[]);
            let fields: Vec<(Arc<str>, Value)> = many.iter().enumerate().map(|(k, v)| (Arc::from(names.get(k).copied().unwrap_or("arg")), to_value(v))).collect();
            Some(Value::Record(fields.into()))
        }
    }
}

fn pointer_event(phase: PointerPhase, p: &PointerInput) -> PointerEvent {
    PointerEvent {
        phase,
        pointer: p.pointer,
        kind: p.kind,
        button: p.button,
        buttons: p.buttons,
        pos: p.local,
        modifiers: p.modifiers,
        pressure: p.pressure,
        tilt: Vec2::ZERO,
        history: Default::default(),
    }
}

/// The pointer phase of a presto-core event, capturing the pointer on a press and releasing it at
/// the end; `None` for other events.
fn pointer_phase(cx: &mut EventCx, ev: &Event) -> Option<(PointerPhase, Option<PointerEvent>)> {
    Some(match ev {
        Event::PointerDown(p) => {
            cx.capture_pointer();
            cx.focus(FocusReason::Pointer);
            (PointerPhase::Down, Some(pointer_event(PointerPhase::Down, p)))
        }
        Event::PointerMove(p) => (PointerPhase::Move, Some(pointer_event(PointerPhase::Move, p))),
        Event::PointerUp(p) => {
            cx.release_pointer();
            (PointerPhase::Up, Some(pointer_event(PointerPhase::Up, p)))
        }
        Event::PointerCancel => {
            cx.release_pointer();
            (PointerPhase::Cancel, None)
        }
        _ => return None,
    })
}

fn cancel_event() -> PointerEvent {
    PointerEvent {
        phase: PointerPhase::Cancel,
        pointer: 0,
        kind: presto_base::event::PointerType::Mouse,
        button: None,
        buttons: 0,
        pos: Default::default(),
        modifiers: Default::default(),
        pressure: 0.0,
        tilt: Vec2::ZERO,
        history: Default::default(),
    }
}

fn enabled(cx: &EventCx) -> bool {
    cx.view(cx.node()).is_some_and(|v| !v.disabled())
}

fn focus_visible(state: StateMask) -> bool {
    state.contains(StateMask::FOCUS_VISIBLE)
}

/// Sends the model's emits: bind write-backs become native-owned writes, the rest signals.
fn deliver(cx: &mut EventCx, spec: &'static KindSpec, kind: &KindRows, emits: Vec<Emit>) -> bool {
    let any = !emits.is_empty();
    for e in emits {
        if e.signal == "custom-colors-changed" {
            if let (Some(p), Some(v)) = (spec.prop_named("custom-colors"), emit_arg(kind, &e)) {
                let _ = cx.set_native(p, v);
            }
            continue;
        }
        if e.signal == "changed"
            && let (Some(p), Some(v)) = (spec.prop_named("value"), emit_arg(kind, &e))
        {
            let _ = cx.set_native(p, v);
        }
        if let Some(s) = spec.signal_named(e.signal) {
            let arg = emit_arg(kind, &e);
            cx.emit(s, arg);
        }
    }
    if any {
        cx.request_paint();
    }
    any
}

/// The prop of `spec` that `id` is.
fn prop_name(spec: &KindSpec, id: PropId) -> Option<&'static str> {
    (0..spec.props.len()).find(|&i| spec.prop_id(i) == id).map(|i| spec.props[i].name)
}

fn str_of(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str).unwrap_or("")
}

fn bool_of(v: Option<&Value>, default: bool) -> bool {
    v.and_then(Value::as_bool).unwrap_or(default)
}

/// An enum prop's value name (`Enum(i)` into `names`, or the name itself).
fn enum_of(v: Option<&Value>, names: &'static [&'static str], default: &'static str) -> &'static str {
    match v {
        Some(Value::Enum(i)) => names.get(*i as usize).copied().unwrap_or(default),
        Some(Value::Str(s)) => names.iter().copied().find(|n| **n == **s).unwrap_or(default),
        _ => default,
    }
}

fn strings_of(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_list).map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// The `calendar` kind.
#[derive(Default)]
pub struct CalendarKind {
    pub model: Calendar,
    pub look: PickerLook,
    text: TextCache,
}

impl Kind for CalendarKind {
    fn spec() -> &'static KindSpec {
        &CALENDAR
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let date = |v: Option<&Value>| parse_date_prop(str_of(v)).map_err(PropError::Rejected);
        let cfg = &mut self.model.cfg;
        match prop_name(&CALENDAR, prop) {
            Some("value") => {
                let d = date(value)?;
                self.model.set_value(d);
            }
            Some("calendar-week") => cfg.week_numbers = bool_of(value, true),
            Some("min") => cfg.min = date(value)?,
            Some("max") => cfg.max = date(value)?,
            Some("first-day") => cfg.first_day = CalCfg::first_day_of(enum_of(value, FIRST_DAY, "auto")),
            Some("weekday-format") => cfg.weekday_format = CalCfg::weekday_format_of(enum_of(value, WEEKDAY_FORMAT, "short")),
            Some("navigation") => cfg.navigation = bool_of(value, true),
            Some("read-only") => cfg.read_only = bool_of(value, false),
            Some("day-parts") => cfg.day_parts = strings_of(value),
            _ => return Ok(()),
        }
        cx.request_paint();
        Ok(())
    }

    fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.model.size(&self.look);
        taffy::Size { width: s.w, height: s.h }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let (on, fv) = (!cx.disabled(), focus_visible(cx.state()));
        self.text.clear();
        let mut scratch = DisplayList::new();
        let mut painter = PrepareText { cx, cache: &mut self.text };
        self.model.record(&mut scratch, &mut painter, &self.look, Date::today_utc(), on, fv);
    }

    fn record(&self, cx: &mut RecordCx) {
        let (on, fv) = (!cx.disabled(), focus_visible(cx.state()));
        let today = Date::today_utc();
        record_with(cx, &self.text, |t, list| self.model.record(list, t, &self.look, today, on, fv));
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = enabled(cx);
        let today = Date::today_utc();
        if let Some((phase, p)) = pointer_phase(cx, ev) {
            let p = p.unwrap_or_else(cancel_event);
            debug_assert_eq!(p.phase, phase);
            let emits = self.model.pointer(&p, &self.look, today, on);
            deliver(cx, &CALENDAR, &rows::CALENDAR, emits);
            cx.request_paint();
            return Handled::Yes;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key.filter(|key| super::calendar::CalBody::claims(*key)) else { return Handled::No };
                let emits = self.model.key(key, k.modifiers, today, on);
                deliver(cx, &CALENDAR, &rows::CALENDAR, emits);
                cx.request_paint();
                Handled::Yes
            }
            Event::HoverLeave | Event::FocusIn(_) | Event::FocusOut => {
                cx.request_paint();
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        let today = Date::today_utc();
        if let Some(v) = self.model.value {
            cx.out.set_value(v.to_string());
        }
        let cursor = self.model.body.cursor(self.model.value, today);
        let (y, m) = self.model.body.page(self.model.value, today);
        if let Some(k) = grid_days(y, m, self.model.cfg.first_day).iter().position(|d| *d == cursor) {
            let (_, desc) = self.model.body.describe(k, &self.model.cfg, self.model.value, today);
            cx.out.set_description(desc);
        }
        if !cx.disabled() && !self.model.cfg.read_only {
            cx.out.add_action(accesskit::Action::Click);
        }
    }
}

/// The `color-editor` kind.
#[derive(Default)]
pub struct ColorEditorKind {
    pub model: ColorEditor,
    pub look: PickerLook,
    text: TextCache,
}

impl Kind for ColorEditorKind {
    fn spec() -> &'static KindSpec {
        &COLOR_EDITOR
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let cfg = &mut self.model.cfg;
        match prop_name(&COLOR_EDITOR, prop) {
            Some("value") => {
                let s = value.and_then(Value::as_str).unwrap_or("#000000");
                let rgba = super::color::parse_hex(s).map_err(PropError::Rejected)?;
                self.model.value = ColorBody::hex_of(super::color::Hsv::from_srgba(rgba), cfg);
            }
            Some("alpha") => cfg.alpha = ColorCfg::alpha_of(enum_of(value, ALPHA, "opaque")),
            Some("fields") => (cfg.hex, cfg.rgb, cfg.hsv) = ColorCfg::fields_of(value.and_then(Value::as_str).unwrap_or("hex rgb")),
            Some("swatches") => cfg.swatches = strings_of(value),
            Some("custom-colors") => cfg.custom = strings_of(value),
            _ => return Ok(()),
        }
        cx.request_paint();
        Ok(())
    }

    fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.model.size(&self.look);
        taffy::Size { width: s.w, height: s.h }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let (on, fv) = (!cx.disabled(), focus_visible(cx.state()));
        self.text.clear();
        let mut scratch = DisplayList::new();
        let mut painter = PrepareText { cx, cache: &mut self.text };
        self.model.record(&mut scratch, &mut painter, &self.look, on, fv);
    }

    fn record(&self, cx: &mut RecordCx) {
        let (on, fv) = (!cx.disabled(), focus_visible(cx.state()));
        record_with(cx, &self.text, |t, list| self.model.record(list, t, &self.look, on, fv));
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = enabled(cx);
        if let Some((_, p)) = pointer_phase(cx, ev) {
            let p = p.unwrap_or_else(cancel_event);
            let emits = self.model.pointer(&p, &self.look, on);
            deliver(cx, &COLOR_EDITOR, &rows::COLOR_EDITOR, emits);
            cx.request_paint();
            return Handled::Yes;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key.filter(|key| self.model.body.claims(*key)) else { return Handled::No };
                let emits = self.model.key(key, k.modifiers, on);
                deliver(cx, &COLOR_EDITOR, &rows::COLOR_EDITOR, emits);
                cx.request_paint();
                Handled::Yes
            }
            Event::FocusIn(_) => {
                if self.model.body.focus.is_none() {
                    self.model.body.focus = ColorBody::stops(&self.model.geom(&self.look)).first().copied();
                }
                cx.request_paint();
                Handled::No
            }
            Event::FocusOut => {
                self.model.body.focus = None;
                cx.request_paint();
                Handled::No
            }
            Event::HoverLeave => {
                cx.request_paint();
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        cx.out.set_value(self.model.value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(spec: &KindSpec, rows: &KindRows) {
        assert_eq!(spec.name, rows.tag);
        let names: Vec<&str> = spec.props.iter().map(|p| p.name).collect();
        let row_names: Vec<&str> = rows.props.iter().map(|p| p.name).collect();
        assert_eq!(names, row_names, "{}", spec.name);
        for (p, r) in spec.props.iter().zip(rows.props) {
            assert_eq!(p.change, r.change, "{}.{}", spec.name, p.name);
            assert_eq!(p.native_owned, r.bind, "{}.{}", spec.name, p.name);
            let ty = match r.ty {
                rows::Ty::Str => ValueType::Str,
                rows::Ty::Bool => ValueType::Bool,
                rows::Ty::StrList => ValueType::List,
                rows::Ty::Enum(v) => {
                    assert_eq!(p.enum_values, v, "{}.{}", spec.name, p.name);
                    ValueType::Enum
                }
            };
            assert_eq!(p.ty, ty, "{}.{}", spec.name, p.name);
        }
        let sigs: Vec<&str> = spec.signals.iter().map(|s| s.name).collect();
        let row_sigs: Vec<&str> = rows.signals.iter().map(|s| s.name).collect();
        assert_eq!(sigs, row_sigs);
        for (s, r) in spec.signals.iter().zip(rows.signals) {
            let want = match r.args.len() {
                0 => None,
                1 => Some(ValueType::Str),
                _ => Some(ValueType::Record),
            };
            assert_eq!(s.arg, want, "{}.{}", spec.name, s.name);
        }
    }

    #[test]
    fn specs_match_the_rows_and_ids_are_distinct() {
        check(&CALENDAR, &rows::CALENDAR);
        check(&DATE_PICKER, &rows::DATE_PICKER);
        check(&COLOR_EDITOR, &rows::COLOR_EDITOR);
        check(&COLOR_PICKER, &rows::COLOR_PICKER);
        let ids = [CALENDAR.id(), DATE_PICKER.id(), COLOR_EDITOR.id(), COLOR_PICKER.id()];
        for (i, a) in ids.iter().enumerate() {
            assert!(ids[i + 1..].iter().all(|b| b != a));
        }
        let mut reg = KindRegistry::new();
        register(&mut reg).expect("the inline pickers register");
        assert!(reg.by_name("calendar").is_some() && reg.by_name("color-editor").is_some());
    }

    #[test]
    fn emits_become_values_and_records() {
        let e = Emit::new("changed", vec![shim::Value::Str("2026-10-22".into())]);
        assert_eq!(emit_arg(&rows::CALENDAR, &e), Some(Value::Str(Arc::from("2026-10-22"))));
        let page = Emit::new("page-changed", vec![shim::Value::I64(2026), shim::Value::I64(11)]);
        let Some(Value::Record(fields)) = emit_arg(&rows::CALENDAR, &page) else { panic!("a record") };
        assert_eq!(fields.iter().map(|(k, _)| &**k).collect::<Vec<_>>(), ["year", "month"]);
        assert_eq!(fields[1].1, Value::I64(11));
        assert_eq!(emit_arg(&rows::CALENDAR, &Emit::new("commit", vec![])), None);
    }

    #[test]
    fn a_calendar_key_changes_the_value() {
        let mut k = CalendarKind::default();
        let today = Date::today_utc();
        k.model.set_value(Some(today));
        let emits = k.model.key(presto_base::key::Key::Enter, Default::default(), today, true);
        assert_eq!(emits.first().map(|e| emit_arg(&rows::CALENDAR, e)), Some(Some(Value::Str(Arc::from(today.to_string().as_str())))));
        assert_eq!(emits[0].signal, "changed");
    }
}
