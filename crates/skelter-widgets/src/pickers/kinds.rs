//! The pickers as kinds of this crate's kind interface ([`crate::shim::core::Kind`], hosted on
//! skelter-core by [`crate::host::Hosted`]): the specs of `calendar`, `color-editor`, `date-picker` and
//! `color-picker` (the rows of [`super::spec`], declared with [`kind_props!`](crate::kind_props)), the
//! kinds of the two inline ones, [`CalendarKind`] and [`ColorEditorKind`], over their models
//! ([`Calendar`], [`ColorEditor`]), and of the two drop ones, [`DatePickerKind`] and
//! [`ColorPickerKind`] ([`DatePicker`], [`ColorPicker`]), whose popups open in the window's overlay
//! stack (as `select`'s list and the menus do): the node records the closed button only; the open
//! popup is placed under it ([`popup::place_popup`]), recorded on its own for its overlay layer
//! (`record_popup`), entered in the stack ([`popup::overlay_spec`]) and light-dismissed (core's
//! `Kind::light_dismiss` and `Event::OutsidePress`, or the stack's `Closed` through `dismissed`).
//!
//! - A prop write is checked against its spec, then parsed into the model; a malformed date or colour
//!   is rejected (the stored value stays).
//! - Events reach the model in the kind's local points; a press asks for focus (the host captures the
//!   pointer). The model's [`Emit`]s go out as signals (several arguments become a record named by the
//!   spec), and `value` / `custom-colors` changed by the user are reported as native changes, which
//!   the host writes back for `bind:`.
//! - Text goes through [`ShaperPainter`] (the thread's text system); the look is
//!   [`PickerLook::from_theme`] of the built-in theme for the node's scheme.
//! - "Today" is [`Date::today_utc`]: skelter has no time-zone database yet.
//!
//! The colour editor's hex box edits through the text-edit field ([`HexField`]): a press on it gives
//! it the body's focus and the field takes the keys (hex digits and `#` only); Enter or the focus
//! leaving applies a valid hex (`changed`, `commit`), an invalid one marks the box `:invalid`, and
//! Escape drops the edit. The field lays out over the document's text system, lent by the host
//! ([`w::LayoutLender`]; fixed-advance text without one), and while it edits it records the box's
//! text with its selection and a caret.
//!
//! The open popup is the node's overlay part (`record_overlay`): skelter-core enters it in the
//! window's overlay stack, draws it in a layer above the content and routes presses inside it to the
//! node (in the node's points, outside its box); the stack's light dismiss and Escape reach the kind
//! as `overlay_closed`. `overlay_spec` and `record_popup` stay for a host that keeps its own stack.
//!
//! Left out: Tab between the colour editor's inner stops (the focus model has no sub-node stops;
//! focus lands on the first stop), and the popup's room (`PopupHost::bounds`, unbounded: the
//! window's content area is not handed to `prepare` yet).

use super::calendar::{CalBody, Calendar, CalCfg, FIRST_DAY, WEEKDAY_FORMAT, grid_days};
use super::color::{ALPHA, Hsv, parse_hex};
use super::color_editor::{COLOR_COMMIT_MODE, ColorBody, ColorCfg, ColorEditor, Ctl};
use crate::text::editor::FocusReason as FieldFocus;
use crate::text::hex::{HexField, HexSignal, color_bytes, color_hex};
use crate::text::keys::KeyAction;
use crate::text::policy::{Platform as TextPlatform, TextPolicy};
use crate::text::field::FieldColors;
use crate::text::shim::RecordSink;
use super::date::{Date, parse_date_prop};
use super::color_picker::ColorPicker;
use super::date_picker::{DATE_COMMIT_MODE, DatePicker};
use super::popup;
use super::PickerLook;
use crate::host;
use crate::interact::StateBits;
use crate::shim::core::{self as w, A11yCx, Event, EventCx, Handled, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, ValueType};
use crate::shim::text as w_text;
use crate::table::shim::TextPainter as _;
use crate::views::overlay::{Closed, DismissReason, OverlayClass, OverlayPart, OverlaySpec};
use crate::table::shim::{self as tshim, Emit};
use crate::table::text_painter::ShaperPainter;
use crate::theme::Theme;
use skelter_base::accesskit::{self, Role};
use skelter_base::event::{KeyState, PointerEvent, PointerPhase, PointerType};
use skelter_base::display::DisplayList;
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::ids::{LayerId, NodeId, PropNodeId};
use skelter_base::key::Key;
use skelter_core::{KindRegistry, RegistryError};
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
use color_picker_props::ColorPickerProps;
use date_picker_props::DatePickerProps;

/// `calendar`: an inline month calendar.
pub static CALENDAR: KindSpec = KindSpec { tag: "calendar", props: CalendarProps::SPECS, signals: DATE_SIGNALS, slots: &[], children: false, role: Role::Grid, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `date-picker`: a drop button with a calendar popup.
pub static DATE_PICKER: KindSpec = KindSpec { tag: "date-picker", props: date_picker_props::DatePickerProps::SPECS, signals: DATE_SIGNALS, slots: &[], children: false, role: Role::DateInput, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `color-editor`: an inline colour editor.
pub static COLOR_EDITOR: KindSpec = KindSpec { tag: "color-editor", props: ColorEditorProps::SPECS, signals: COLOR_SIGNALS, slots: &[], children: false, role: Role::Group, name_from: NameFrom::None, text_prop: None, focusable: true };

/// `color-picker`: a drop button with a colour editor popup.
pub static COLOR_PICKER: KindSpec = KindSpec { tag: "color-picker", props: color_picker_props::ColorPickerProps::SPECS, signals: COLOR_SIGNALS, slots: &[], children: false, role: Role::ColorWell, name_from: NameFrom::None, text_prop: None, focusable: true };

/// Registers the four pickers.
pub fn register(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    host::register::<CalendarKind>(reg)?;
    host::register::<ColorEditorKind>(reg)?;
    host::register::<DatePickerKind>(reg)?;
    host::register::<ColorPickerKind>(reg)?;
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
/// drawing waits for skelter-core's computed style per node).
/// The hex box's text style, as the field lays it out (the font the body draws the box's text in).
fn hex_style(look: &PickerLook) -> w_text::TextStyle {
    w_text::TextStyle { family: Arc::from(look.font_family.as_str()), size: look.font_size, ..w_text::TextStyle::default() }
}

/// A recording's list and resources as the text field's [`RecordSink`].
struct ListRes<'a> {
    list: &'a mut DisplayList,
    res: &'a mut dyn w::Resources,
}

impl RecordSink for ListRes<'_> {
    fn push(&mut self, p: skelter_base::display::Prim, ink: Rect) {
        self.list.push(p, ink);
    }
    fn draw_run(&mut self, run: Arc<skelter_base::text::GlyphRun>, origin: Point, color: skelter_base::color::Color) {
        let ink = Rect::new(origin.x, origin.y - run.ascent, origin.x + run.advance, origin.y + run.descent);
        let id = self.res.run(&run);
        self.list.push(skelter_base::display::Prim::Glyphs(skelter_base::display::Glyphs { run: id, origin, color }), ink);
    }
}

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
    // drawn from `PickerLook` (theme tokens), not from `::part` styles; a leaf: no children to place or fold
    fn parts() -> &'static [&'static str] {
        &[]
    }
    fn places_children() -> bool {
        false
    }
    fn folds_children() -> bool {
        false
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

    fn claims_key(&self, key: Key, _m: skelter_base::event::Modifiers) -> bool {
        CalBody::claims(key)
    }
}

/// The `color-editor` kind.
#[derive(Default)]
pub struct ColorEditorKind {
    pub props: ColorEditorProps,
    pub model: ColorEditor,
    /// The hex box's text-edit field while the box has the body's focus.
    pub hex: Option<HexField>,
}

impl ColorEditorKind {
    /// Opens the hex field when the hex box got the body's focus, and closes it (applying its edit)
    /// when the focus went elsewhere.
    fn sync_hex(&mut self, cx: &mut EventCx) {
        let focused = self.model.body.focus == Some(Ctl::Hex);
        match (&mut self.hex, focused) {
            (None, true) => {
                let [r, g, b, a] = self.model.rgba();
                let mut h = HexField::new(skelter_base::color::Color::from_rgba8(r, g, b, a), self.model.cfg.alpha, TextPolicy::from_policy(TextPlatform::current(), cx.policy));
                h.focus_in(FieldFocus::Pointer);
                // a press on the box selects the hex: typing replaces it
                h.field.editor.select_all();
                self.model.body.hex_text = Some(h.field.text());
                self.model.body.hex_by_field = true;
                self.hex = Some(h);
            }
            (Some(h), false) => {
                h.focus_out();
                self.hex_applied(cx);
                self.hex = None;
                self.model.body.hex_text = None;
                self.model.body.hex_by_field = false;
            }
            _ => {}
        }
    }

    /// Applies what the hex field committed (a valid hex: `changed`, `commit`) and shows its text.
    fn hex_applied(&mut self, cx: &mut EventCx) {
        let Some(h) = self.hex.as_mut() else { return };
        let signals = h.take_signals();
        let (text, invalid) = (h.field.text(), h.invalid());
        let mut emits = Vec::new();
        for sig in signals {
            let HexSignal::Commit(c) = sig;
            let outs = self.model.body.set_hex(&color_hex(color_bytes(c)), &self.model.cfg);
            emits.extend(self.model.apply(outs));
        }
        self.model.body.hex_invalid = invalid;
        self.model.body.hex_text = Some(text);
        self.deliver(cx, emits);
    }

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
    // drawn from `PickerLook` (theme tokens), not from `::part` styles; a leaf: no children to place or fold
    fn parts() -> &'static [&'static str] {
        &[]
    }
    fn places_children() -> bool {
        false
    }
    fn folds_children() -> bool {
        false
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
        // the editing hex box: the field's text with its selection, and the caret
        if let Some(h) = &self.hex {
            let fg = if on { look.control_fg } else { look.disabled_fg };
            let colors = FieldColors { text: fg, placeholder: look.muted, selection: look.list_sel, selection_inactive: look.list_sel, preedit_underline: fg };
            let mut sink = ListRes { list: &mut **list, res: &mut **res };
            h.field.record(&mut sink, &colors);
            if let Some(c) = h.field.caret_local() {
                let c = Rect::new(c.min.x, c.min.y, c.min.x + c.width().max(1.0), c.max.y);
                tshim::frame(list, c, fg, skelter_base::color::Color::TRANSPARENT, 0.0, 0.0);
            }
        }
    }

    /// Lays the editing hex field out in the box (its caret and selection geometry).
    fn prepare(&mut self, cx: &mut PrepareCx) {
        let look = look(cx.style.dark);
        let (Some(h), Some(r)) = (self.hex.as_mut(), self.model.geom(look).hex) else { return };
        let mut lay = cx.layouter(hex_style(look));
        let row = lay.layout(" ", None).size().h;
        let top = r.min.y + (r.height() - row) / 2.0;
        h.field.prepare(&mut *lay, Rect::new(r.min.x + 4.0, top, r.max.x - 4.0, top + row), skelter_base::geom::Affine::IDENTITY);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = cx.enabled && !self.props.disabled;
        let look = look(true);
        if let Some(p) = pointer_of(cx, ev) {
            let emits = self.model.pointer(&p, look, on);
            self.deliver(cx, emits);
            self.sync_hex(cx);
            cx.repaint = true;
            return Handled::Yes;
        }
        // the hex box's keys are its text field's
        if let (Event::Key(k), Some(h)) = (ev, self.hex.as_mut())
            && on
        {
            let now = cx.now;
            let action = h.key(k, now, &mut *cx.layouter(hex_style(look)));
            self.hex_applied(cx);
            cx.repaint = true;
            return if matches!(action, KeyAction::Ignore) { Handled::No } else { Handled::Yes };
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
                self.sync_hex(cx);
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

    /// The body's stepping keys, and every key but Tab while the hex field edits.
    fn claims_key(&self, key: Key, _m: skelter_base::event::Modifiers) -> bool {
        self.model.body.claims(key) || (self.hex.is_some() && key != Key::Tab)
    }
}

// ---- the drop kinds: date-picker, color-picker ----

/// The footer labels' sizes as `prepare` measured them, for the event handlers (which have no
/// shaper): the footer's hit rects match the recorded buttons.
#[derive(Clone, Debug, Default)]
struct Measured(Vec<(String, Size)>);

impl Measured {
    fn fill(&mut self, text: &mut dyn w_text::TextShaper, look: &PickerLook) {
        let mut p = ShaperPainter { text, res: None };
        let style = look.text(look.fg);
        self.0 = [popup::Footer::Save, popup::Footer::Cancel].iter().map(|f| (f.label().to_owned(), p.measure(f.label(), &style))).collect();
    }
}

impl tshim::TextPainter for Measured {
    fn measure(&mut self, text: &str, _style: &tshim::TextStyle) -> Size {
        self.0.iter().find(|(t, _)| t == text).map_or(Size::ZERO, |(_, s)| *s)
    }
    fn draw(&mut self, _list: &mut DisplayList, _text: &str, _style: &tshim::TextStyle, _rect: Rect, _align: tshim::HAlign, _elide: bool) {}
}

/// What both drop kinds keep about their popup: the window's room, where the open popup sits and
/// the measured footer. The popup is a layer of the window's overlay stack
/// ([`popup::overlay_spec`]), recorded on its own ([`DatePickerKind::record_popup`],
/// [`ColorPickerKind::record_popup`]); the node's own list is the closed button only.
#[derive(Clone, Debug)]
pub struct PopupHost {
    /// Where the popup may go, in the node's local points (the window's content area; unbounded
    /// until the host that owns the overlay stack sets it).
    pub bounds: Rect,
    /// The open popup's rect in the node's local points (from the last `prepare`).
    placed: Option<Rect>,
    /// The button's size (from the last `prepare`).
    button: Size,
    measured: Measured,
    /// The primary button went down in the popup: its moves and release go there.
    pressed: bool,
}

impl Default for PopupHost {
    fn default() -> Self {
        PopupHost { bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY), placed: None, button: Size::ZERO, measured: Measured::default(), pressed: false }
    }
}

impl PopupHost {
    /// The open popup's rect (node-local points).
    pub fn placed(&self) -> Option<Rect> {
        self.placed
    }

    fn prepare(&mut self, cx: &mut PrepareCx, open: bool, popup_size: Size, look: &PickerLook) {
        self.button = cx.size;
        self.measured.fill(cx.text, look);
        self.placed = open.then(|| popup::place_popup(cx.size, popup_size, self.bounds));
    }

    /// The overlay-stack entry for the open popup: `origin` is the node's top-left in window points.
    fn overlay_spec(&self, node: NodeId, layer: LayerId, transform: PropNodeId, origin: Vec2) -> Option<OverlaySpec> {
        let r = self.placed?;
        Some(popup::overlay_spec(node, layer, transform, r.translate(origin), Rect::from_origin_size(Point::ZERO, self.button).translate(origin)))
    }

    /// A press outside the popup and the button (the host's light dismiss).
    fn outside(&self, p: Point) -> bool {
        !self.placed.is_some_and(|r| r.contains(p)) && !Rect::from_origin_size(Point::ZERO, self.button).contains(p)
    }

    /// The popup closed: its layer goes.
    fn close(&mut self) {
        self.placed = None;
        self.pressed = false;
    }

    /// `ev` in popup-local points when it is the popup's: it lands in the open popup, or it moves
    /// or ends a press that began there.
    fn in_popup(&mut self, ev: &PointerEvent) -> Option<PointerEvent> {
        let r = self.placed?;
        let inside = r.contains(ev.pos);
        let take = match ev.phase {
            PointerPhase::Down => {
                self.pressed = inside;
                inside
            }
            PointerPhase::Up | PointerPhase::Leave | PointerPhase::Cancel => std::mem::take(&mut self.pressed) || (inside && ev.phase == PointerPhase::Up),
            _ => self.pressed || inside,
        };
        if !take {
            return None;
        }
        let mut local = ev.clone();
        local.pos = Point::new(ev.pos.x - r.min.x, ev.pos.y - r.min.y);
        Some(local)
    }

    /// Records the open popup with `f` into `cx.list` in the node's local points, as the node's
    /// overlay part ([`w::Kind::record_overlay`]): a light-dismissable popup whose button counts as
    /// inside.
    fn record_part(&self, cx: &mut RecordCx, f: impl FnOnce(&mut DisplayList, &mut dyn tshim::TextPainter)) -> Option<OverlayPart> {
        let r = self.placed?;
        let RecordCx { list, text, res, .. } = cx;
        let mut popup = self.record(&mut **text, &mut **res, f)?;
        popup.translate(Vec2::new(r.min.x, r.min.y));
        list.bounds = list.bounds.union(&popup.bounds);
        list.items.extend(popup.items);
        let mut part = OverlayPart::new(OverlayClass::Popup, r);
        part.inside = vec![Rect::from_origin_size(Point::ZERO, self.button)];
        Some(part)
    }

    /// Records the popup (popup-local points) with `f`, registering its runs through `res`.
    fn record(&self, text: &mut dyn w_text::TextShaper, res: &mut dyn w::Resources, f: impl FnOnce(&mut DisplayList, &mut dyn tshim::TextPainter)) -> Option<DisplayList> {
        self.placed?;
        let mut list = DisplayList::new();
        let mut p = ShaperPainter { text, res: Some(res) };
        f(&mut list, &mut p);
        Some(list)
    }
}

/// The `date-picker` kind: a drop button over the [`DatePicker`] model, its calendar in a popup
/// of the window's overlay stack.
#[derive(Default)]
pub struct DatePickerKind {
    pub props: DatePickerProps,
    pub model: DatePicker,
    pub popup: PopupHost,
}

impl DatePickerKind {
    fn sync(&mut self) -> Result<(), String> {
        let p = &self.props;
        self.model.cfg = CalCfg {
            week_numbers: p.calendar_week,
            min: parse_date_prop(&p.min)?,
            max: parse_date_prop(&p.max)?,
            first_day: CalCfg::first_day_of(&p.first_day),
            weekday_format: CalCfg::weekday_format_of(&p.weekday_format),
            navigation: p.navigation,
            read_only: p.read_only,
            day_parts: strings(&p.day_parts),
        };
        self.model.format = p.format.clone();
        self.model.confirm = p.commit_mode != "immediate";
        Ok(())
    }

    fn deliver(&mut self, cx: &mut EventCx, emits: Vec<Emit>) {
        for e in &emits {
            if e.signal == "changed" {
                self.props.value = self.model.value.map(|d| d.to_string()).unwrap_or_default();
                cx.prop_changed(date_picker_props::P::value.id());
            }
            emit(cx, &DATE_PICKER, e);
        }
        if !self.model.drop.is_open() {
            self.popup.close();
        }
        cx.repaint = true;
    }

    /// The overlay-stack entry of the open popup (`origin`: the node's top-left, window points).
    pub fn overlay_spec(&self, node: NodeId, layer: LayerId, transform: PropNodeId, origin: Vec2) -> Option<OverlaySpec> {
        self.popup.overlay_spec(node, layer, transform, origin)
    }

    /// The open popup's display list (popup-local points), for its overlay layer; `None` while closed.
    pub fn record_popup(&self, text: &mut dyn w_text::TextShaper, res: &mut dyn w::Resources, dark: bool) -> Option<DisplayList> {
        let look = look(dark);
        self.popup.record(text, res, |list, p| self.model.record_popup(list, p, look, tshim::Platform::current(), Date::today_utc()))
    }

    /// The overlay stack closed the popup (a press outside, Escape there).
    pub fn dismissed(&mut self, cx: &mut EventCx, _closed: &Closed) {
        self.model.dismiss();
        self.deliver(cx, Vec::new());
    }
}

impl w::Kind for DatePickerKind {
    fn spec() -> &'static KindSpec {
        &DATE_PICKER
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &DATE_PICKER
    }
    fn parts() -> &'static [&'static str] {
        &[]
    }
    fn places_children() -> bool {
        false
    }
    fn folds_children() -> bool {
        false
    }

    fn set_prop(&mut self, prop: PropId, v: &w::Value) -> Result<(), PropError> {
        let before = self.props.clone();
        self.props.set(prop, v)?;
        let bad = |name: &'static str| PropError::Type { prop: name, expected: ValueType::Str };
        if prop == date_picker_props::P::value.id() {
            match parse_date_prop(&self.props.value) {
                Ok(d) => self.model.value = d,
                Err(_) => {
                    self.props = before;
                    return Err(bad("value"));
                }
            }
        }
        if self.sync().is_err() {
            self.props = before;
            self.sync().ok();
            return Err(bad(DATE_PICKER.prop_spec(prop).map_or("", |s| s.name)));
        }
        Ok(())
    }

    fn get_prop(&self, prop: PropId) -> Option<w::Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let look = look(cx.style.dark);
        let mut p = ShaperPainter { text: &mut *cx.text, res: None };
        let s = self.model.size(&mut p, look, Date::today_utc());
        taffy::Size { width: s.w, height: s.h }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let look = look(cx.style.dark);
        let size = self.model.popup_size(look);
        self.popup.prepare(cx, self.model.drop.is_open(), size, look);
    }

    /// The closed button only: the open popup is its overlay layer's ([`DatePickerKind::record_popup`]).
    fn record(&self, cx: &mut RecordCx) {
        let state = cx.style.state;
        let (on, fv) = (!state.contains(StateBits::DISABLED) && !self.props.disabled, state.contains(StateBits::FOCUS_VISIBLE));
        let look = look(cx.style.dark);
        let size = cx.size;
        let RecordCx { list, text, res, .. } = cx;
        let mut painter = ShaperPainter { text: &mut **text, res: Some(&mut **res) };
        self.model.record_button(list, &mut painter, look, size, Date::today_utc(), fv, on);
    }

    /// The open popup, in a layer of the window's overlay stack.
    fn record_overlay(&self, cx: &mut RecordCx) -> Option<OverlayPart> {
        let look = look(cx.style.dark);
        self.popup.record_part(cx, |list, p| self.model.record_popup(list, p, look, tshim::Platform::current(), Date::today_utc()))
    }

    fn overlay_closed(&mut self, cx: &mut EventCx, _reason: DismissReason) -> Handled {
        self.model.dismiss();
        self.deliver(cx, Vec::new());
        Handled::Yes
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = cx.enabled && !self.props.disabled;
        let today = Date::today_utc();
        let look = look(true);
        match ev {
            Event::OutsidePress(p) => {
                if self.model.drop.is_open() && self.popup.outside(*p) {
                    self.model.dismiss();
                    self.deliver(cx, Vec::new());
                }
                Handled::No
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                if !self.model.drop.is_open() && !self.model.drop.claims(key, k.modifiers) {
                    return Handled::No;
                }
                let emits = self.model.key(key, k.modifiers, today, on);
                self.deliver(cx, emits);
                Handled::Yes
            }
            Event::Focus { gained: false, .. } => {
                self.model.dismiss();
                self.deliver(cx, Vec::new());
                Handled::No
            }
            _ => {
                let Some(p) = pointer_of(cx, ev) else { return Handled::No };
                let emits = match self.popup.in_popup(&p) {
                    Some(local) => {
                        let mut m = self.popup.measured.clone();
                        self.model.popup_pointer(&local, &mut m, look, tshim::Platform::current(), today, on)
                    }
                    None => self.model.button_pointer(&p, cx.size, on),
                };
                self.deliver(cx, emits);
                Handled::Yes
            }
        }
    }

    /// Open: a press outside the popup and the button closes it.
    fn light_dismiss(&self) -> bool {
        self.model.drop.is_open()
    }
    fn prop_state(&self) -> StateBits {
        if self.model.drop.is_open() { StateBits::OPEN } else { StateBits::NONE }
    }
    fn prop_states() -> StateBits {
        StateBits::OPEN
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(Role::DateInput);
        cx.node.set_value(self.model.text(Date::today_utc()));
        cx.node.set_expanded(self.model.drop.is_open());
        if self.props.disabled || cx.state.contains(StateBits::DISABLED) {
            cx.node.set_disabled();
        } else {
            cx.node.add_action(accesskit::Action::Focus);
            cx.node.add_action(accesskit::Action::Click);
        }
    }

    fn claims_key(&self, key: Key, m: skelter_base::event::Modifiers) -> bool {
        self.model.drop.claims(key, m) || (self.model.drop.is_open() && CalBody::claims(key))
    }
}

/// The `color-picker` kind: a drop button over the [`ColorPicker`] model, its colour editor in a
/// popup of the window's overlay stack.
#[derive(Default)]
pub struct ColorPickerKind {
    pub props: ColorPickerProps,
    pub model: ColorPicker,
    pub popup: PopupHost,
}

impl ColorPickerKind {
    fn sync(&mut self) {
        let p = &self.props;
        let (hex, rgb, hsv) = ColorCfg::fields_of(&p.fields);
        let cfg = &mut self.model.cfg;
        cfg.alpha = ColorCfg::alpha_of(&p.alpha);
        (cfg.hex, cfg.rgb, cfg.hsv) = (hex, rgb, hsv);
        cfg.swatches = strings(&p.swatches);
        cfg.custom = strings(&p.custom_colors);
        self.model.live = p.commit_mode != "confirm";
    }

    fn deliver(&mut self, cx: &mut EventCx, emits: Vec<Emit>) {
        for e in &emits {
            match e.signal {
                "custom-colors-changed" => {
                    self.props.custom_colors = self.model.cfg.custom.iter().map(|s| w::Value::str(s)).collect();
                    cx.prop_changed(color_picker_props::P::custom_colors.id());
                }
                "changed" => {
                    self.props.value = self.model.value.clone();
                    cx.prop_changed(color_picker_props::P::value.id());
                    emit(cx, &COLOR_PICKER, e);
                }
                _ => emit(cx, &COLOR_PICKER, e),
            }
        }
        if !self.model.drop.is_open() {
            self.popup.close();
        }
        cx.repaint = true;
    }

    /// The overlay-stack entry of the open popup (`origin`: the node's top-left, window points).
    pub fn overlay_spec(&self, node: NodeId, layer: LayerId, transform: PropNodeId, origin: Vec2) -> Option<OverlaySpec> {
        self.popup.overlay_spec(node, layer, transform, origin)
    }

    /// The open popup's display list (popup-local points), for its overlay layer; `None` while closed.
    pub fn record_popup(&self, text: &mut dyn w_text::TextShaper, res: &mut dyn w::Resources, dark: bool, focus_visible: bool) -> Option<DisplayList> {
        let look = look(dark);
        self.popup.record(text, res, |list, p| self.model.record_popup(list, p, look, tshim::Platform::current(), focus_visible))
    }

    /// The overlay stack closed the popup: live edits made in it are committed.
    pub fn dismissed(&mut self, cx: &mut EventCx, _closed: &Closed) {
        let emits = self.model.dismiss();
        self.deliver(cx, emits);
    }
}

impl w::Kind for ColorPickerKind {
    fn spec() -> &'static KindSpec {
        &COLOR_PICKER
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &COLOR_PICKER
    }
    fn parts() -> &'static [&'static str] {
        &[]
    }
    fn places_children() -> bool {
        false
    }
    fn folds_children() -> bool {
        false
    }

    fn set_prop(&mut self, prop: PropId, v: &w::Value) -> Result<(), PropError> {
        if prop == color_picker_props::P::value.id() {
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

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let look = look(cx.style.dark);
        let size = self.model.popup_size(look);
        self.popup.prepare(cx, self.model.drop.is_open(), size, look);
    }

    /// The closed button only: the open popup is its overlay layer's ([`ColorPickerKind::record_popup`]).
    fn record(&self, cx: &mut RecordCx) {
        let state = cx.style.state;
        let (on, fv) = (!state.contains(StateBits::DISABLED) && !self.props.disabled, state.contains(StateBits::FOCUS_VISIBLE));
        let look = look(cx.style.dark);
        self.model.record_button(cx.list, look, cx.size, fv, on);
    }

    /// The open popup, in a layer of the window's overlay stack.
    fn record_overlay(&self, cx: &mut RecordCx) -> Option<OverlayPart> {
        let look = look(cx.style.dark);
        let fv = cx.style.state.contains(StateBits::FOCUS_VISIBLE);
        self.popup.record_part(cx, |list, p| self.model.record_popup(list, p, look, tshim::Platform::current(), fv))
    }

    /// Live edits made in the popup are committed.
    fn overlay_closed(&mut self, cx: &mut EventCx, _reason: DismissReason) -> Handled {
        let emits = self.model.dismiss();
        self.deliver(cx, emits);
        Handled::Yes
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let on = cx.enabled && !self.props.disabled;
        let look = look(true);
        match ev {
            Event::OutsidePress(p) => {
                if self.model.drop.is_open() && self.popup.outside(*p) {
                    let emits = self.model.dismiss();
                    self.deliver(cx, emits);
                }
                Handled::No
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                if !self.model.drop.is_open() && !self.model.drop.claims(key, k.modifiers) {
                    return Handled::No;
                }
                let emits = self.model.key(key, k.modifiers, on);
                self.deliver(cx, emits);
                Handled::Yes
            }
            Event::Focus { gained: false, .. } => {
                let emits = self.model.dismiss();
                self.deliver(cx, emits);
                Handled::No
            }
            _ => {
                let Some(p) = pointer_of(cx, ev) else { return Handled::No };
                let emits = match self.popup.in_popup(&p) {
                    Some(local) => {
                        let mut m = self.popup.measured.clone();
                        self.model.popup_pointer(&local, &mut m, look, tshim::Platform::current(), on)
                    }
                    None => self.model.button_pointer(&p, cx.size, on),
                };
                self.deliver(cx, emits);
                Handled::Yes
            }
        }
    }

    /// Open: a press outside the popup and the button closes it.
    fn light_dismiss(&self) -> bool {
        self.model.drop.is_open()
    }
    fn prop_state(&self) -> StateBits {
        if self.model.drop.is_open() { StateBits::OPEN } else { StateBits::NONE }
    }
    fn prop_states() -> StateBits {
        StateBits::OPEN
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(Role::ColorWell);
        cx.node.set_value(self.model.value.clone());
        cx.node.set_expanded(self.model.drop.is_open());
        if self.props.disabled || cx.state.contains(StateBits::DISABLED) {
            cx.node.set_disabled();
        } else {
            cx.node.add_action(accesskit::Action::Focus);
            cx.node.add_action(accesskit::Action::Click);
        }
    }

    fn claims_key(&self, key: Key, m: skelter_base::event::Modifiers) -> bool {
        self.model.drop.claims(key, m) || (self.model.drop.is_open() && self.model.body.claims(key))
    }
}

#[cfg(test)]
mod tests {
    use super::super::spec::{self as rows, KindRows};
    use super::*;
    use crate::shim::core::Kind as _;
    use crate::shim::core::{Emitted, SignalId};
    use crate::interact::Policy;
    use skelter_base::time::Time;

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
        let mut cx = EventCx::new(Time(1), skelter_base::geom::Size::new(300.0, 300.0), &mut state, &policy);
        let key = skelter_base::event::KeyEvent { key: Some(Key::Enter), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: skelter_base::event::KeyLocation::Standard, modifiers: Default::default() };
        assert_eq!(k.event(&mut cx, &Event::Key(key)), Handled::Yes);
        let changed: SignalId = CALENDAR.signal("changed").unwrap();
        assert_eq!(cx.emitted.first(), Some(&Emitted { signal: changed, args: vec![w::Value::str(&today.to_string())] }));
        assert!(cx.changed.contains(&CALENDAR.prop("value").unwrap()));
    }

    /// Clicks the drop button of `k` (at its size `size`) through its events.
    fn click_button<K: w::Kind>(k: &mut K, size: skelter_base::geom::Size) {
        use crate::pickers::test_util::ptr;
        let policy = Policy::default();
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let mut state = StateBits::NONE;
            let mut cx = EventCx::new(Time(1), size, &mut state, &policy);
            k.event(&mut cx, &Event::Pointer(ptr(phase, 8.0, 8.0)));
        }
    }

    #[test]
    fn the_date_popup_opens_in_the_overlay_stack_and_light_dismisses() {
        use crate::controls::testkit;
        use crate::shim::testing::{MemResources, MonoShaper};
        use crate::views::overlay::{DismissReason, OverlayClass, OverlayStack};
        let mut k = DatePickerKind::default();
        k.set_prop(DATE_PICKER.prop("value").unwrap(), &w::Value::str("2026-10-22")).unwrap();
        let size = testkit::measure(&k, StateBits::NONE);
        // the window's room, node-local: the node at (20, 20) of a 400×300 window
        k.popup.bounds = Rect::new(-20.0, -20.0, 380.0, 280.0);
        testkit::prepare(&mut k, size, StateBits::NONE);
        assert!(k.overlay_spec(NodeId(5), LayerId(9), PropNodeId(3), Vec2::new(20.0, 20.0)).is_none(), "closed: no entry");
        assert!(!w::Kind::light_dismiss(&k));
        click_button(&mut k, size);
        assert!(k.model.drop.is_open() && w::Kind::light_dismiss(&k));
        assert_eq!(w::Kind::prop_state(&k), StateBits::OPEN);
        testkit::prepare(&mut k, size, StateBits::NONE);
        let placed = k.popup.placed().expect("placed when open");
        assert_eq!(placed.min.y, size.h + popup::POPUP_OFFSET, "below the button");
        // not painted inline: the node's list is the button; the popup records for its layer
        let (list, _) = testkit::record(&k, size, StateBits::OPEN);
        assert!(list.items.iter().all(|i| !matches!(i, skelter_base::display::Prim::Quad(q) if q.rect.max.y > size.h + 0.5)), "nothing below the button");
        let pop = k.record_popup(&mut MonoShaper, &mut MemResources::default(), true).expect("open");
        assert!(pop.items.len() > 40, "the surface, the calendar and the footer");
        // the overlay stack's popup band, the button inside it
        let mut stack = OverlayStack::new(LayerId(1));
        let id = stack.push(k.overlay_spec(NodeId(5), LayerId(9), PropNodeId(3), Vec2::new(20.0, 20.0)).unwrap());
        let e = stack.get(id).unwrap();
        assert_eq!((e.spec.class, e.spec.rect, e.spec.opener), (OverlayClass::Popup, placed.translate(Vec2::new(20.0, 20.0)), Some(NodeId(5))));
        assert!(stack.pointer_down(Point::new(25.0, 25.0)).1.is_empty(), "a press on the button keeps it");
        let r = placed.translate(Vec2::new(20.0, 20.0));
        let inside = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
        assert!(stack.pointer_down(inside).1.is_empty(), "a press in it keeps it");
        let (_, closed) = stack.pointer_down(Point::new(395.0, 295.0));
        assert_eq!((closed.len(), closed[0].reason, closed[0].restore_focus), (1, DismissReason::OutsidePress, Some(NodeId(5))));
        let policy = Policy::default();
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(Time(2), size, &mut state, &policy);
        k.dismissed(&mut cx, &closed[0]);
        assert!(!k.model.drop.is_open() && cx.repaint);
        // core's light dismiss: an outside press on the node
        click_button(&mut k, size);
        testkit::prepare(&mut k, size, StateBits::NONE);
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(Time(3), size, &mut state, &policy);
        k.event(&mut cx, &Event::OutsidePress(inside - Vec2::new(20.0, 20.0)));
        assert!(k.model.drop.is_open(), "inside the popup is not outside");
        k.event(&mut cx, &Event::OutsidePress(Point::new(370.0, 270.0)));
        assert!(!k.model.drop.is_open());
    }

    #[test]
    fn the_colour_popup_takes_presses_in_its_layer_and_commits_live_edits_on_dismiss() {
        use crate::controls::testkit;
        use crate::pickers::test_util::ptr;
        let mut k = ColorPickerKind::default();
        let size = testkit::measure(&k, StateBits::NONE);
        click_button(&mut k, size);
        testkit::prepare(&mut k, size, StateBits::NONE);
        let placed = k.popup.placed().expect("open and placed");
        assert!(k.overlay_spec(NodeId(1), LayerId(2), PropNodeId(3), Vec2::ZERO).is_some());
        // a press in the SV square (popup-local, in the layer) edits live
        let look = look(true);
        let body = popup::popup_parts(look, placed.size(), Size::ZERO, false).0.min;
        let at = Point::new(placed.min.x + body.x + 60.0, placed.min.y + body.y + 30.0);
        let policy = Policy::default();
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(Time(2), size, &mut state, &policy);
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Down, at.x, at.y)));
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Up, at.x, at.y)));
        assert_ne!(k.model.value, "#000000", "the square picked a colour");
        assert!(k.model.drop.is_open());
        let changed = COLOR_PICKER.signal("changed").unwrap();
        assert!(cx.emitted.iter().any(|e| e.signal == changed));
        // dismissed from outside: the live edit is committed
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(Time(3), size, &mut state, &policy);
        k.event(&mut cx, &Event::OutsidePress(Point::new(-50.0, -50.0)));
        assert!(!k.model.drop.is_open());
        let commit = COLOR_PICKER.signal("commit").unwrap();
        assert!(cx.emitted.iter().any(|e| e.signal == commit));
        let mut reg = KindRegistry::new();
        register(&mut reg).unwrap();
        assert!(reg.by_name("date-picker").is_some() && reg.by_name("color-picker").is_some());
    }

    #[test]
    fn typing_in_the_hex_box_edits_through_the_text_field() {
        use crate::pickers::test_util::ptr;
        use skelter_base::event::{KeyEvent, KeyLocation};
        let mut k = ColorEditorKind::default();
        k.set_prop(COLOR_EDITOR.prop("value").unwrap(), &w::Value::str("#ff0000")).unwrap();
        let hex = k.model.geom(look(true)).hex.expect("a hex box by default");
        let policy = Policy::default();
        let mut state = StateBits::FOCUSED;
        let size = skelter_base::geom::Size::new(300.0, 400.0);
        let (x, y) = (hex.min.x + 4.0, hex.min.y + 4.0);
        let mut cx = EventCx::new(Time(1), size, &mut state, &policy);
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Down, x, y)));
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Up, x, y)));
        assert!(k.hex.is_some(), "the box's field is open");
        assert_eq!(k.model.body.hex_text.as_deref(), Some("#ff0000"));
        let key = |k: Key, text: Option<&str>| Event::Key(KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_owned), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Default::default() });
        let mut cx = EventCx::new(Time(2), size, &mut state, &policy);
        // `x` is filtered out by the field
        for c in ["#", "0", "x", "f", "0"] {
            assert_eq!(k.event(&mut cx, &key(Key::A, Some(c))), Handled::Yes);
        }
        assert_eq!(k.model.body.hex_text.as_deref(), Some("#0f0"), "shown as typed");
        assert_eq!(k.model.value, "#ff0000", "not applied before Enter");
        assert!(w::Kind::claims_key(&k, Key::A, Default::default()) && !w::Kind::claims_key(&k, Key::Tab, Default::default()));
        k.event(&mut cx, &key(Key::Enter, Some("\r")));
        assert_eq!(k.model.value, "#00ff00");
        let names: Vec<&str> = cx.emitted.iter().map(|e| COLOR_EDITOR.signals[e.signal.0 as usize].name).collect();
        assert_eq!(names, ["changed", "commit"]);
        assert!(cx.changed.contains(&COLOR_EDITOR.prop("value").unwrap()), "bind:value");
        // an invalid hex keeps the colour and marks the box; focus leaving closes the field
        let mut cx = EventCx::new(Time(3), size, &mut state, &policy);
        k.hex.as_mut().unwrap().field.editor.select_all();
        k.event(&mut cx, &key(Key::A, Some("1")));
        k.event(&mut cx, &key(Key::A, Some("2")));
        k.event(&mut cx, &key(Key::Enter, Some("\r")));
        assert!(k.model.body.hex_invalid && cx.emitted.is_empty());
        assert_eq!(k.model.value, "#00ff00");
        k.event(&mut cx, &Event::Focus { gained: false, reason: crate::interact::FocusReason::Program });
        assert!(k.hex.is_none() && k.model.body.hex_text.is_none());
    }

    #[test]
    fn the_editing_hex_box_records_its_selection_and_caret_through_the_lent_layouts() {
        use crate::pickers::test_util::ptr;
        use crate::shim::testing::{MemResources, MonoShaper};
        use crate::text::shim::{Layouter, MonoLayouter, TextGeometry};
        use skelter_base::display::Prim;
        use skelter_base::event::{KeyEvent, KeyLocation};
        /// The document's text system as the host lends it: here fixed-advance, counting the layouts.
        struct Lender(usize);
        impl w::LayoutLender for Lender {
            fn layout(&mut self, text: &str, style: &w_text::TextStyle, max_width: Option<f32>) -> Arc<dyn TextGeometry> {
                self.0 += 1;
                MonoLayouter { advance: style.size * 0.5, line_height: style.size * 1.25 }.layout(text, max_width)
            }
        }
        let mut k = ColorEditorKind::default();
        k.set_prop(COLOR_EDITOR.prop("value").unwrap(), &w::Value::str("#ff0000")).unwrap();
        let lk = look(true);
        let hex = k.model.geom(lk).hex.expect("a hex box by default");
        let policy = Policy::default();
        let mut state = StateBits::FOCUSED;
        let size = skelter_base::geom::Size::new(300.0, 400.0);
        let mut lender = Lender(0);
        let prepare = |k: &mut ColorEditorKind, lender: &mut Lender| {
            let mut text = MonoShaper;
            let style = w::Styles { computed: None, parts: &[], children: &[], state: StateBits::FOCUSED, dark: true, theme: None, inherited: None };
            w::Kind::prepare(k, &mut PrepareCx { size, style, text: &mut text, emitted: Vec::new(), layouts: Some(lender) });
        };
        let record = |k: &ColorEditorKind| {
            let mut list = DisplayList::new();
            let mut res = MemResources::default();
            let mut text = MonoShaper;
            let style = w::Styles { computed: None, parts: &[], children: &[], state: StateBits::FOCUSED, dark: true, theme: None, inherited: None };
            w::Kind::record(k, &mut RecordCx { size, style, list: &mut list, text: &mut text, res: &mut res, scale: 1.0 });
            list.items.iter().filter_map(|p| match p { Prim::Quad(q) => Some((q.rect, q.fill.clone())), _ => None }).collect::<Vec<_>>()
        };
        let solid = |c| skelter_base::color::Fill::Solid(c);
        let (x, y) = (hex.min.x + 4.0, hex.min.y + 4.0);
        let mut cx = EventCx::new(Time(1), size, &mut state, &policy);
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Down, x, y)));
        k.event(&mut cx, &Event::Pointer(ptr(PointerPhase::Up, x, y)));
        assert!(k.hex.is_some());
        prepare(&mut k, &mut lender);
        assert!(lender.0 > 0, "the field laid out through the lent layouts");
        // all selected: the selection spans the seven chars at the lender's advance, in the box
        let quads = record(&k);
        let sel: Vec<Rect> = quads.iter().filter(|(_, f)| *f == solid(lk.list_sel)).map(|(r, _)| *r).collect();
        assert_eq!(sel.len(), 1, "one selection rect: {quads:?}");
        assert!((sel[0].width() - 7.0 * lk.font_size * 0.5).abs() < 0.01);
        assert_eq!(sel[0].min.x, hex.min.x + 4.0);
        assert!(hex.contains(sel[0].min) && hex.contains(sel[0].max));
        // typing replaces the selection: no selection, a caret after the one char
        let mut cx = EventCx::new(Time(2), size, &mut state, &policy);
        let mut lent = Lender(0);
        cx.layouts = Some(&mut lent);
        let key = Event::Key(KeyEvent { key: Some(Key::A), physical: 0, text: Some("#".into()), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Default::default() });
        assert_eq!(k.event(&mut cx, &key), Handled::Yes);
        drop(cx);
        prepare(&mut k, &mut lender);
        let quads = record(&k);
        assert!(quads.iter().all(|(_, f)| *f != solid(lk.list_sel)), "no selection after typing");
        let fg = solid(lk.control_fg);
        let caret: Vec<Rect> = quads.iter().filter(|(r, f)| *f == fg && r.width() <= 1.0 && hex.contains(r.min)).map(|(r, _)| *r).collect();
        assert_eq!(caret.len(), 1, "a caret in the box: {quads:?}");
        assert!((caret[0].min.x - (hex.min.x + 4.0 + lk.font_size * 0.5)).abs() < 0.01, "after the one char");
    }
}
