//! The pickers as kinds of this crate's kind interface ([`crate::shim::core::Kind`], hosted on
//! presto-core by [`crate::host::Hosted`]): the specs of `calendar`, `color-editor`, `date-picker` and
//! `color-picker` (the rows of [`super::spec`], declared with [`kind_props!`](crate::kind_props)), and the
//! kinds of the two inline ones, [`CalendarKind`] and [`ColorEditorKind`], over their models
//! ([`Calendar`], [`ColorEditor`]).
//!
//! - A prop write is checked against its spec, then parsed into the model; a malformed date or colour
//!   is rejected (the stored value stays).
//! - Events reach the model in the kind's local points; a press asks for focus (the host captures the
//!   pointer). The model's [`Emit`]s go out as signals (several arguments become a record named by the
//!   spec), and `value` / `custom-colors` changed by the user are reported as native changes, which
//!   the host writes back for `bind:`.
//! - Text goes through [`ShaperPainter`] (the thread's text system); the look is
//!   [`PickerLook::from_theme`] of the built-in theme for the node's scheme.
//! - "Today" is [`Date::today_utc`]: presto has no time-zone database yet.
//!
//! Left out: the kinds of `date-picker` and `color-picker` (their popups are layers in the window's
//! overlay stack, lane WIDGETS-VIEWS' and presto-core's: only their specs are here), the colour
//! editor's hex field typing (lane TEXT-INPUT's field), and Tab between the colour editor's inner
//! stops (the focus model has no sub-node stops; focus lands on the first stop).

use super::calendar::{CalBody, Calendar, CalCfg, FIRST_DAY, WEEKDAY_FORMAT, grid_days};
use super::color::{ALPHA, Hsv, parse_hex};
use super::color_editor::{COLOR_COMMIT_MODE, ColorBody, ColorCfg, ColorEditor};
use super::date::{Date, parse_date_prop};
use super::date_picker::DATE_COMMIT_MODE;
use super::PickerLook;
use crate::host;
use crate::interact::StateBits;
use crate::shim::core::{self as w, A11yCx, Event, EventCx, Handled, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, ValueType};
use crate::table::shim::{self as tshim, Emit};
use crate::table::text_painter::ShaperPainter;
use crate::theme::Theme;
use presto_base::accesskit::{self, Role};
use presto_base::event::{KeyState, PointerEvent, PointerPhase, PointerType};
use presto_base::geom::{Point, Vec2};
use presto_base::key::Key;
use presto_core::{KindRegistry, RegistryError};
use std::sync::{Arc, OnceLock};

const SIG_CHANGED: SignalSpec = SignalSpec::new("changed", &[("value", ValueType::Str)], "The user changed the value; carries it.");
const SIG_COMMIT: SignalSpec = SignalSpec::new("commit", &[("value", ValueType::Str)], "The user committed the value (a pick, Enter, the end of a drag).");
const SIG_PAGE: SignalSpec = SignalSpec::new("page-changed", &[("year", ValueType::I64), ("month", ValueType::I64)], "The shown month changed.");

/// `calendar`'s and `date-picker`'s signals.
pub const DATE_SIGNALS: &[SignalSpec] = &[SIG_CHANGED, SIG_COMMIT, SIG_PAGE];
/// `color-editor`'s and `color-picker`'s signals.
pub const COLOR_SIGNALS: &[SignalSpec] = &[SIG_CHANGED, SIG_COMMIT];

pub mod calendar_props {
    use super::{FIRST_DAY, WEEKDAY_FORMAT};
    use crate::shim::core::Value;
    crate::kind_props! {
        /// `calendar`'s props.
        pub struct CalendarProps;
        /// The picked date, ISO 8601 (`""` for none).
        value "value": String = Str("") ; Str, Paint, bindable;
        /// ISO week numbers in front of each row.
        calendar_week "calendar-week": bool = Bool(true) ; Bool, Layout;
        min "min": String = Str("") ; Str, Paint;
        max "max": String = Str("") ; Str, Paint;
        first_day "first-day": String = Enum("auto") ; Enum(FIRST_DAY), Paint;
        weekday_format "weekday-format": String = Enum("short") ; Enum(WEEKDAY_FORMAT), Layout;
        /// The month header with its previous and next buttons.
        navigation "navigation": bool = Bool(true) ; Bool, Layout;
        read_only "read-only": bool = Bool(false) ; Bool, Paint;
        /// Per-day part names (`::part(name)`), as `YYYY-MM-DD name` strings.
        day_parts "day-parts": Vec<Value> = None ; List, Paint;
        disabled "disabled": bool = Bool(false) ; Bool, Paint;
    }
}

pub mod date_picker_props {
    use super::{DATE_COMMIT_MODE, FIRST_DAY, WEEKDAY_FORMAT};
    use crate::shim::core::Value;
    crate::kind_props! {
        /// `date-picker`'s props.
        pub struct DatePickerProps;
        value "value": String = Str("") ; Str, Layout, bindable;
        /// The closed button's text (`strftime` subset).
        format "format": String = Str("%Y-%m-%d") ; Str, Layout;
        calendar_week "calendar-week": bool = Bool(true) ; Bool, Paint;
        min "min": String = Str("") ; Str, Paint;
        max "max": String = Str("") ; Str, Paint;
        first_day "first-day": String = Enum("auto") ; Enum(FIRST_DAY), Paint;
        weekday_format "weekday-format": String = Enum("short") ; Enum(WEEKDAY_FORMAT), Paint;
        navigation "navigation": bool = Bool(true) ; Bool, Paint;
        read_only "read-only": bool = Bool(false) ; Bool, Paint;
        day_parts "day-parts": Vec<Value> = None ; List, Paint;
        commit_mode "commit-mode": String = Enum("confirm") ; Enum(DATE_COMMIT_MODE), Paint;
        disabled "disabled": bool = Bool(false) ; Bool, Paint;
    }
}

pub mod color_editor_props {
    use super::ALPHA;
    use crate::shim::core::Value;
    crate::kind_props! {
        /// `color-editor`'s props.
        pub struct ColorEditorProps;
        /// The colour, CSS hex (`#rrggbb`, `#rrggbbaa`).
        value "value": String = Str("#000000") ; Str, Paint, bindable;
        alpha "alpha": String = Enum("opaque") ; Enum(ALPHA), Layout;
        /// The channel boxes shown: `hex`, `rgb`, `hsv`, space-separated.
        fields "fields": String = Str("hex rgb") ; Str, Layout;
        swatches "swatches": Vec<Value> = None ; List, Layout;
        custom_colors "custom-colors": Vec<Value> = None ; List, Layout, bindable;
        disabled "disabled": bool = Bool(false) ; Bool, Paint;
    }
}

pub mod color_picker_props {
    use super::{ALPHA, COLOR_COMMIT_MODE};
    use crate::shim::core::Value;
    crate::kind_props! {
        /// `color-picker`'s props.
        pub struct ColorPickerProps;
        value "value": String = Str("#000000") ; Str, Paint, bindable;
        alpha "alpha": String = Enum("opaque") ; Enum(ALPHA), Paint;
        fields "fields": String = Str("hex rgb") ; Str, Layout;
        swatches "swatches": Vec<Value> = None ; List, Layout;
        custom_colors "custom-colors": Vec<Value> = None ; List, Layout, bindable;
        commit_mode "commit-mode": String = Enum("live") ; Enum(COLOR_COMMIT_MODE), Paint;
        disabled "disabled": bool = Bool(false) ; Bool, Paint;
    }
}

use calendar_props::CalendarProps;
use color_editor_props::ColorEditorProps;

/// `calendar`: an inline month calendar.
pub static CALENDAR: KindSpec = KindSpec { tag: "calendar", props: CalendarProps::SPECS, signals: DATE_SIGNALS, slots: &[], children: false, role: Role::Grid, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `date-picker`: a drop button with a calendar popup (spec only; see the module doc).
pub static DATE_PICKER: KindSpec = KindSpec { tag: "date-picker", props: date_picker_props::DatePickerProps::SPECS, signals: DATE_SIGNALS, slots: &[], children: false, role: Role::DateInput, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `color-editor`: an inline colour editor.
pub static COLOR_EDITOR: KindSpec = KindSpec { tag: "color-editor", props: ColorEditorProps::SPECS, signals: COLOR_SIGNALS, slots: &[], children: false, role: Role::Group, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `color-picker`: a drop button with a colour editor popup (spec only; see the module doc).
pub static COLOR_PICKER: KindSpec = KindSpec { tag: "color-picker", props: color_picker_props::ColorPickerProps::SPECS, signals: COLOR_SIGNALS, slots: &[], children: false, role: Role::ColorWell, name_from: NameFrom::None, text_prop: None, focusable: true };

/// Registers the kinds that have an implementation: `calendar` and `color-editor`.
pub fn register(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    host::register::<CalendarKind>(reg)?;
    host::register::<ColorEditorKind>(reg)?;
    Ok(())
}

impl PickerLook {
    /// The look from `theme`'s tokens (the defaults where a token is missing).
    pub fn from_theme(theme: &Theme) -> PickerLook {
        let d = PickerLook::default();
        let c = |name: &str, fallback| theme.color(name).unwrap_or(fallback);
        let px = |name: &str, fallback| theme.px(name).unwrap_or(fallback);
        PickerLook {
            fg: c("fg", d.fg),
            muted: c("muted", d.muted),
            disabled_fg: c("disabled-fg", d.disabled_fg),
            icon: c("icon", d.icon),
            accent: c("accent", d.accent),
            accent_hover: c("accent-hover", d.accent_hover),
            accent_fg: c("accent-fg", d.accent_fg),
            focus: c("focus", d.focus),
            control_bg: c("control-bg", d.control_bg),
            control_border: c("control-border", d.control_border),
            control_fg: c("control-fg", d.control_fg),
            toolbar_hover: c("toolbar-hover", d.toolbar_hover),
            invalid_border: c("invalid-border", d.invalid_border),
            list_hover: c("list-hover", d.list_hover),
            list_sel: c("list-sel", d.list_sel),
            list_sel_fg: c("list-sel-fg", d.list_sel_fg),
            popup_bg: c("menu-bg", d.popup_bg),
            popup_border: c("menu-border-color", d.popup_border),
            button_bg: c("button-bg", d.button_bg),
            button_bg_hover: c("button-bg-hover", d.button_bg_hover),
            button_fg: c("button-fg", d.button_fg),
            row_h: px("row-h", d.row_h),
            control_h: px("control-h", d.control_h),
            radius: px("radius", d.radius),
            radius_md: px("radius-md", d.radius_md),
            stroke: px("stroke", d.stroke),
            font_size: px("font-size", d.font_size),
            font_family: theme.token("font-family").map(str::to_owned).unwrap_or(d.font_family),
        }
    }
}

/// The look for the node's scheme (the built-in VS Code Modern themes; author CSS on the pickers'
/// drawing waits for presto-core's computed style per node).
fn look(dark: bool) -> &'static PickerLook {
    static DARK: OnceLock<PickerLook> = OnceLock::new();
    static LIGHT: OnceLock<PickerLook> = OnceLock::new();
    if dark { DARK.get_or_init(|| PickerLook::from_theme(&Theme::dark_modern())) } else { LIGHT.get_or_init(|| PickerLook::from_theme(&Theme::light_modern())) }
}

/// A table-shim value as this crate's kind value.
pub fn to_value(v: &tshim::Value) -> w::Value {
    match v {
        tshim::Value::Bool(b) => w::Value::Bool(*b),
        tshim::Value::I64(i) => w::Value::I64(*i),
        tshim::Value::F64(f) => w::Value::F64(*f),
        tshim::Value::Str(s) => w::Value::Str(Arc::from(s.as_str())),
        tshim::Value::List(l) => w::Value::List(l.iter().map(to_value).collect()),
    }
}

/// Raises `e` from the kind with `spec` (unknown signal names are dropped: the models only raise
/// their spec's signals and `custom-colors-changed`, which the colour kind handles itself).
fn emit(cx: &mut EventCx, spec: &KindSpec, e: &Emit) {
    if let Some(s) = spec.signal(e.signal) {
        cx.emit(s, e.args.iter().map(to_value).collect());
    }
}

fn cancel_event() -> PointerEvent {
    PointerEvent { phase: PointerPhase::Cancel, pointer: 0, kind: PointerType::Mouse, button: None, buttons: 0, pos: Point::ZERO, modifiers: Default::default(), pressure: 0.0, tilt: Vec2::ZERO, history: Default::default() }
}

/// The model's pointer event for `ev`: pointer events as they are (a press also asks for focus),
/// the hover ending and a lost capture as a cancel; `None` for other events.
fn pointer_of(cx: &mut EventCx, ev: &Event) -> Option<PointerEvent> {
    match ev {
        Event::Pointer(p) => {
            if p.phase == PointerPhase::Down {
                cx.focus_request = Some(true);
            }
            Some(p.clone())
        }
        Event::Hover(false) | Event::CaptureLost => Some(cancel_event()),
        _ => None,
    }
}

fn strings(l: &[w::Value]) -> Vec<String> {
    l.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}

/// The `calendar` kind.
#[derive(Default)]
pub struct CalendarKind {
    pub props: CalendarProps,
    pub model: Calendar,
}

impl CalendarKind {
    /// Copies the props into the model.
    fn sync(&mut self) -> Result<(), String> {
        let p = &self.props;
        let cfg = CalCfg {
            week_numbers: p.calendar_week,
            min: parse_date_prop(&p.min)?,
            max: parse_date_prop(&p.max)?,
            first_day: CalCfg::first_day_of(&p.first_day),
            weekday_format: CalCfg::weekday_format_of(&p.weekday_format),
            navigation: p.navigation,
            read_only: p.read_only,
            day_parts: strings(&p.day_parts),
        };
        self.model.cfg = cfg;
        Ok(())
    }

    fn deliver(&mut self, cx: &mut EventCx, emits: Vec<Emit>) {
        for e in &emits {
            if e.signal == "changed" {
                self.props.value = self.model.value.map(|d| d.to_string()).unwrap_or_default();
                cx.prop_changed(calendar_props::P::value.id());
            }
            emit(cx, &CALENDAR, e);
        }
    }
}

impl w::Kind for CalendarKind {
    fn spec() -> &'static KindSpec {
        &CALENDAR
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &CALENDAR
    }

    fn set_prop(&mut self, prop: PropId, v: &w::Value) -> Result<(), PropError> {
        let before = self.props.clone();
        self.props.set(prop, v)?;
        let bad = |name: &'static str| PropError::Type { prop: name, expected: ValueType::Str };
        if prop == calendar_props::P::value.id() {
            match parse_date_prop(&self.props.value) {
                Ok(d) => self.model.set_value(d),
                Err(_) => {
                    self.props = before;
                    return Err(bad("value"));
                }
            }
        }
        if self.sync().is_err() {
            self.props = before;
            self.sync().ok();
            return Err(bad(CALENDAR.prop_spec(prop).map_or("", |s| s.name)));
        }
        Ok(())
    }

    fn get_prop(&self, prop: PropId) -> Option<w::Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.model.size(look(cx.style.dark));
        taffy::Size { width: s.w, height: s.h }
    }

    fn record(&self, cx: &mut RecordCx) {
        let state = cx.style.state;
        let (on, fv) = (!state.contains(StateBits::DISABLED) && !self.props.disabled, state.contains(StateBits::FOCUS_VISIBLE));
        let look = look(cx.style.dark);
        let RecordCx { list, text, res, .. } = cx;
        let mut painter = ShaperPainter { text: &mut **text, res: Some(&mut **res) };
        self.model.record(list, &mut painter, look, Date::today_utc(), on, fv);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = cx.enabled && !self.props.disabled;
        let today = Date::today_utc();
        let look = look(true);
        if let Some(p) = pointer_of(cx, ev) {
            let emits = self.model.pointer(&p, look, today, on);
            self.deliver(cx, emits);
            cx.repaint = true;
            return Handled::Yes;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key.filter(|key| CalBody::claims(*key)) else { return Handled::No };
                let emits = self.model.key(key, k.modifiers, today, on);
                self.deliver(cx, emits);
                cx.repaint = true;
                Handled::Yes
            }
            Event::Focus { .. } => {
                cx.repaint = true;
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(Role::Grid);
        let today = Date::today_utc();
        if let Some(v) = self.model.value {
            cx.node.set_value(v.to_string());
        }
        let cursor = self.model.body.cursor(self.model.value, today);
        let (y, m) = self.model.body.page(self.model.value, today);
        if let Some(k) = grid_days(y, m, self.model.cfg.first_day).iter().position(|d| *d == cursor) {
            let (_, desc) = self.model.body.describe(k, &self.model.cfg, self.model.value, today);
            cx.node.set_description(desc);
        }
        if self.props.disabled || cx.state.contains(StateBits::DISABLED) {
            cx.node.set_disabled();
        } else {
            cx.node.add_action(accesskit::Action::Focus);
            if !self.model.cfg.read_only {
                cx.node.add_action(accesskit::Action::Click);
            }
        }
    }

    fn claims_key(&self, key: Key, _m: presto_base::event::Modifiers) -> bool {
        CalBody::claims(key)
    }
}

/// The `color-editor` kind.
#[derive(Default)]
pub struct ColorEditorKind {
    pub props: ColorEditorProps,
    pub model: ColorEditor,
}

impl ColorEditorKind {
    fn sync(&mut self) {
        let p = &self.props;
        let (hex, rgb, hsv) = ColorCfg::fields_of(&p.fields);
        let cfg = &mut self.model.cfg;
        cfg.alpha = ColorCfg::alpha_of(&p.alpha);
        (cfg.hex, cfg.rgb, cfg.hsv) = (hex, rgb, hsv);
        cfg.swatches = strings(&p.swatches);
        cfg.custom = strings(&p.custom_colors);
    }

    fn deliver(&mut self, cx: &mut EventCx, emits: Vec<Emit>) {
        for e in &emits {
            match e.signal {
                "custom-colors-changed" => {
                    self.props.custom_colors = self.model.cfg.custom.iter().map(|s| w::Value::str(s)).collect();
                    cx.prop_changed(color_editor_props::P::custom_colors.id());
                }
                "changed" => {
                    self.props.value = self.model.value.clone();
                    cx.prop_changed(color_editor_props::P::value.id());
                    emit(cx, &COLOR_EDITOR, e);
                }
                _ => emit(cx, &COLOR_EDITOR, e),
            }
        }
    }
}

impl w::Kind for ColorEditorKind {
    fn spec() -> &'static KindSpec {
        &COLOR_EDITOR
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &COLOR_EDITOR
    }

    fn set_prop(&mut self, prop: PropId, v: &w::Value) -> Result<(), PropError> {
        if prop == color_editor_props::P::value.id() {
            let rgba = v.as_str().map(parse_hex).ok_or(PropError::Type { prop: "value", expected: ValueType::Str })?;
            let Ok(rgba) = rgba else { return Err(PropError::Type { prop: "value", expected: ValueType::Str }) };
            self.props.set(prop, v)?;
            self.model.value = ColorBody::hex_of(Hsv::from_srgba(rgba), &self.model.cfg);
            return Ok(());
        }
        self.props.set(prop, v)?;
        self.sync();
        Ok(())
    }

    fn get_prop(&self, prop: PropId) -> Option<w::Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = self.model.size(look(cx.style.dark));
        taffy::Size { width: s.w, height: s.h }
    }

    fn record(&self, cx: &mut RecordCx) {
        let state = cx.style.state;
        let (on, fv) = (!state.contains(StateBits::DISABLED) && !self.props.disabled, state.contains(StateBits::FOCUS_VISIBLE));
        let look = look(cx.style.dark);
        let RecordCx { list, text, res, .. } = cx;
        let mut painter = ShaperPainter { text: &mut **text, res: Some(&mut **res) };
        self.model.record(list, &mut painter, look, on, fv);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = cx.enabled && !self.props.disabled;
        let look = look(true);
        if let Some(p) = pointer_of(cx, ev) {
            let emits = self.model.pointer(&p, look, on);
            self.deliver(cx, emits);
            cx.repaint = true;
            return Handled::Yes;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key.filter(|key| self.model.body.claims(*key)) else { return Handled::No };
                let emits = self.model.key(key, k.modifiers, on);
                self.deliver(cx, emits);
                cx.repaint = true;
                Handled::Yes
            }
            Event::Focus { gained: true, .. } => {
                if self.model.body.focus.is_none() {
                    self.model.body.focus = ColorBody::stops(&self.model.geom(look)).first().copied();
                }
                cx.repaint = true;
                Handled::No
            }
            Event::Focus { gained: false, .. } => {
                self.model.body.focus = None;
                cx.repaint = true;
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(Role::Group);
        cx.node.set_value(self.model.value.clone());
        if self.props.disabled || cx.state.contains(StateBits::DISABLED) {
            cx.node.set_disabled();
        } else {
            cx.node.add_action(accesskit::Action::Focus);
        }
    }

    fn claims_key(&self, key: Key, _m: presto_base::event::Modifiers) -> bool {
        self.model.body.claims(key)
    }
}

#[cfg(test)]
mod tests {
    use super::super::spec::{self as rows, KindRows};
    use super::*;
    use crate::shim::core::Kind as _;
    use crate::shim::core::{Emitted, SignalId};
    use crate::interact::Policy;
    use presto_base::time::Time;

    fn check(spec: &KindSpec, rows: &KindRows) {
        assert_eq!(spec.tag, rows.tag);
        let props: Vec<_> = spec.props.iter().filter(|p| p.name != "disabled").collect();
        let names: Vec<&str> = props.iter().map(|p| p.name).collect();
        let row_names: Vec<&str> = rows.props.iter().map(|p| p.name).collect();
        assert_eq!(names, row_names, "{}", spec.tag);
        for (p, r) in props.iter().zip(rows.props) {
            assert_eq!(p.change, r.change, "{}.{}", spec.tag, p.name);
            assert_eq!(p.bindable, r.bind, "{}.{}", spec.tag, p.name);
            match (p.ty, r.ty) {
                (ValueType::Str, rows::Ty::Str) | (ValueType::Bool, rows::Ty::Bool) | (ValueType::List, rows::Ty::StrList) => {}
                (ValueType::Enum(a), rows::Ty::Enum(b)) => assert_eq!(a, b, "{}.{}", spec.tag, p.name),
                (a, b) => panic!("{}.{}: {a:?} vs {b:?}", spec.tag, p.name),
            }
        }
        let sigs: Vec<(&str, Vec<&str>)> = spec.signals.iter().map(|s| (s.name, s.args.iter().map(|a| a.0).collect())).collect();
        let row_sigs: Vec<(&str, Vec<&str>)> = rows.signals.iter().map(|s| (s.name, s.args.to_vec())).collect();
        assert_eq!(sigs, row_sigs, "{}", spec.tag);
    }

    #[test]
    fn specs_match_the_rows_and_register() {
        check(&CALENDAR, &rows::CALENDAR);
        check(&DATE_PICKER, &rows::DATE_PICKER);
        check(&COLOR_EDITOR, &rows::COLOR_EDITOR);
        check(&COLOR_PICKER, &rows::COLOR_PICKER);
        let mut reg = KindRegistry::new();
        register(&mut reg).expect("the inline pickers register");
        assert!(reg.by_name("calendar").is_some() && reg.by_name("color-editor").is_some());
    }

    #[test]
    fn bad_values_are_rejected_and_keep_the_old_one() {
        let mut k = CalendarKind::default();
        let value = CALENDAR.prop("value").unwrap();
        k.set_prop(value, &w::Value::str("2026-10-22")).unwrap();
        assert!(k.set_prop(value, &w::Value::str("not a date")).is_err());
        assert_eq!(k.get_prop(value), Some(w::Value::str("2026-10-22")));
        let mut c = ColorEditorKind::default();
        let cv = COLOR_EDITOR.prop("value").unwrap();
        assert!(c.set_prop(cv, &w::Value::str("#12")).is_err());
        c.set_prop(cv, &w::Value::str("#336699")).unwrap();
        assert_eq!(c.model.value, "#336699");
    }

    #[test]
    fn enter_on_the_calendar_emits_changed_and_reports_the_value() {
        let mut k = CalendarKind::default();
        let today = Date::today_utc();
        k.set_prop(CALENDAR.prop("value").unwrap(), &w::Value::str(&today.to_string())).unwrap();
        let mut state = StateBits::FOCUSED;
        let policy = Policy::default();
        let mut cx = EventCx::new(Time(1), presto_base::geom::Size::new(300.0, 300.0), &mut state, &policy);
        let key = presto_base::event::KeyEvent { key: Some(Key::Enter), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: presto_base::event::KeyLocation::Standard, modifiers: Default::default() };
        assert_eq!(k.event(&mut cx, &Event::Key(key)), Handled::Yes);
        let changed: SignalId = CALENDAR.signal("changed").unwrap();
        assert_eq!(cx.emitted.first(), Some(&Emitted { signal: changed, args: vec![w::Value::str(&today.to_string())] }));
        assert!(cx.changed.contains(&CALENDAR.prop("value").unwrap()));
    }
}
