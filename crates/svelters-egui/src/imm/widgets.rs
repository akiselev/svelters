//! The first `imm` widgets (OW1a's kinds): button, checkbox, toggle-button, switch, label,
//! separator, and the icon primitive. Each one runs the kind's units and calls the kind's own
//! paint function (`crate::widgets::<kind>::paint`) with its UA table, so a Rust-drawn widget
//! and a Svelte one paint the same shapes in the same state (PaintTrace) and expose the same
//! AccessKit node.
//!
//! Builders, as egui's: options, then `show(ui, &sv)` (allocates its measured size in `ui`'s
//! layout) or `show_at(ui, rect, &sv)` (a rect the caller laid out: berth's tab strip).

use super::{Compose, El, Response, Sv, run, widget_id};
use crate::focus::Role as FocusRole;
use crate::interact::{FocusParams, Press, PressParams, Toggle, ToggleParams};
use crate::paint::{self, IconKey, StyleView};
use crate::provider::{AvailableSpace, StateBits};
use crate::widgets::{button, checkbox, label, separator, switch, toggle_button};
use egui::accesskit;

/// What every `imm` widget takes: its virtual element (tag and classes), id, enabled, tooltip.
#[derive(Clone, Debug)]
struct Common {
    el: El,
    id: Option<egui::Id>,
    enabled: bool,
    tooltip: Option<String>,
}

impl Common {
    fn new(tag: &'static str) -> Common {
        Common { el: El::new(tag), id: None, enabled: true, tooltip: None }
    }
    fn enabled(&self, ui: &egui::Ui) -> bool {
        self.enabled && ui.is_enabled()
    }
}

macro_rules! common_setters {
    () => {
        /// A class on the virtual element (`.primary`), for CSS.
        pub fn class(mut self, c: impl Into<std::borrow::Cow<'static, str>>) -> Self {
            self.c.el = self.c.el.class(c);
            self
        }
        /// The whole virtual element (tag, id, classes, attributes): another tag styles it as
        /// that tag's rules say, with this widget's UA table under them.
        pub fn element(mut self, el: El) -> Self {
            self.c.el = el;
            self
        }
        /// Its egui id (focus, state and animation follow it); else `ui`'s next auto id.
        pub fn id(mut self, id: egui::Id) -> Self {
            self.c.id = Some(id);
            self
        }
        /// Disabled: no intents, `:disabled`, out of the Tab order, still hoverable (R9).
        pub fn enabled(mut self, on: bool) -> Self {
            self.c.enabled = on;
            self
        }
        /// A tooltip (shown while disabled too: the reason, R9).
        pub fn tooltip(mut self, t: impl Into<String>) -> Self {
            self.c.tooltip = Some(t.into());
            self
        }
    };
}

fn resp(inner: egui::Response, state: StateBits) -> Response {
    Response { inner, activated: false, double_clicked: false, secondary_clicked: false, changed: false, state }
}

// ---- button ----

/// A push button (`<button>`): caption, optional icon, `frame`, `small`, `selected`, `repeat`.
#[derive(Clone, Debug)]
pub struct Button {
    c: Common,
    text: String,
    icon: String,
    selected: bool,
    frame: bool,
    small: bool,
    font_weight: i64,
    repeat: bool,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Button {
        Button { c: Common::new("button"), text: text.into(), icon: String::new(), selected: false, frame: true, small: false, font_weight: 400, repeat: false }
    }
    common_setters!();
    /// An icon before the caption (`name` in the `icons` family, or `family:name`).
    pub fn icon(mut self, key: impl Into<String>) -> Self {
        self.icon = key.into();
        self
    }
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    /// Draws the background and border (otherwise only while hovered, pressed or selected).
    pub fn frame(mut self, on: bool) -> Self {
        self.frame = on;
        self
    }
    pub fn small(mut self, on: bool) -> Self {
        self.small = on;
        self
    }
    pub fn font_weight(mut self, w: i64) -> Self {
        self.font_weight = w;
        self
    }
    /// Activates again and again while held (300 ms, then every 100 ms).
    pub fn repeat(mut self, on: bool) -> Self {
        self.repeat = on;
        self
    }

    fn look(&self) -> button::Look<'_> {
        button::Look { text: &self.text, icon: &self.icon, frame: self.frame, small: self.small }
    }

    /// Its size at rest under `sv`.
    pub fn size(&self, ui: &egui::Ui, sv: &Sv) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, button::UA, StateBits::NONE);
        let host = st.view().with_font_weight(Some(self.font_weight));
        let css = st.css.as_deref();
        let (icon, label) = (css.and_then(|h| sv.part_computed(&self.c.el, StateBits::NONE, h, "icon")), css.and_then(|h| sv.part_computed(&self.c.el, StateBits::NONE, h, "label")));
        button::measure(ui.ctx(), &host, icon.as_deref(), label.as_deref(), &self.look())
    }

    /// Allocates its size in `ui`'s layout and shows it there.
    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let (_, rect) = ui.allocate_space(self.size(ui, sv));
        self.show_at(ui, rect, sv)
    }

    /// Shows it at `rect` (laid out by the caller).
    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let id = widget_id(ui, self.c.id);
        let enabled = self.c.enabled(ui);
        let mut props = StateBits::NONE;
        props.set(StateBits::SELECTED, self.selected);
        let press = PressParams { repeat: self.repeat.then_some((0.3, 0.1)), ..PressParams::default() };
        let mut ran = run(ui, id, rect, enabled, Compose { hover: true, focus: Some(FocusParams::default()), press: Some(press), toggle: None }, props);
        let mut out = resp(ran.resp.clone(), ran.state);
        for p in ran.press.iter() {
            match p {
                Press::Click { count: 1, button: egui::PointerButton::Primary, .. } | Press::Repeat => out.activated = true,
                Press::Click { count: 2, button: egui::PointerButton::Primary, .. } => out.double_clicked = true,
                Press::Click { button: egui::PointerButton::Secondary, .. } => out.secondary_clicked = true,
                _ => {}
            }
        }
        let st = sv.styled(ui.ctx(), &self.c.el, button::UA, ran.state);
        let host = st.view().with_font_weight(Some(self.font_weight));
        // R9: disabled keeps its shape at `opacity` .4
        if host.opacity() < 1.0 {
            ran.ui.multiply_opacity(host.opacity());
        }
        if ran.ui.is_rect_visible(rect) {
            let css = st.css.as_deref();
            let (icon, label) = (css.and_then(|h| sv.part_computed(&self.c.el, ran.state, h, "icon")), css.and_then(|h| sv.part_computed(&self.c.el, ran.state, h, "label")));
            button::paint(&mut ran.paint_cx("button"), ui.ctx(), &host, icon.as_deref(), label.as_deref(), rect, &self.look());
        }
        let cursor = host.cursor();
        let (selected, text) = (self.selected, self.text.as_str());
        let info = || egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, text);
        out.inner = ran.finish(cursor, Some(&info), |n| n.set_role(accesskit::Role::Button), true, Some(FocusRole::Button { default: false, cancel: false }), self.c.tooltip.as_deref());
        out
    }
}

// ---- toggles ----

/// A check box with a caption (`<checkbox bind:checked>`); writes the new value to `checked`.
#[derive(Debug)]
pub struct Checkbox<'a> {
    c: Common,
    checked: &'a mut bool,
    text: String,
}

impl<'a> Checkbox<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<String>) -> Checkbox<'a> {
        Checkbox { c: Common::new("checkbox"), checked, text: text.into() }
    }
    common_setters!();

    pub fn size(&self, ui: &egui::Ui, sv: &Sv) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, checkbox::UA, StateBits::NONE);
        checkbox::measure_indicator(ui.ctx(), &st.view(), checkbox::INDICATOR, &self.text, 18.0)
    }

    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let (_, rect) = ui.allocate_space(self.size(ui, sv));
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let Checkbox { c, checked, text } = self;
        let (ran, mut out) = toggle(ui, &c, rect, checked, ToggleParams { checked: *checked, ..ToggleParams::default() });
        let st = sv.styled(ui.ctx(), &c.el, checkbox::UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            checkbox::paint(&mut ran.paint_cx("checkbox"), ui.ctx(), &st.view(), rect, &text, *checked);
        }
        let (enabled, on) = (c.enabled(ui), *checked);
        let info = || egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, &text);
        out.inner = ran.finish(st.view().cursor(), Some(&info), |n| n.set_role(accesskit::Role::CheckBox), true, Some(FocusRole::Toggle), c.tooltip.as_deref());
        out
    }
}

/// Runs a toggle's units (hover, focus, press, toggle) and applies `Toggled` to `value`, before
/// paint (the kind's default action, so this pass paints the new value).
fn toggle(ui: &mut egui::Ui, c: &Common, rect: egui::Rect, value: &mut bool, tp: ToggleParams) -> (super::Ran, Response) {
    let id = widget_id(ui, c.id);
    let enabled = c.enabled(ui);
    let ran = run(ui, id, rect, enabled, Compose { hover: true, focus: Some(FocusParams::default()), press: Some(PressParams::default()), toggle: Some(tp) }, StateBits::NONE);
    let mut out = resp(ran.resp.clone(), ran.state);
    out.activated = ran.press.iter().any(Press::is_click);
    for Toggle::Toggled { checked, .. } in ran.toggle.iter().copied() {
        *value = checked;
        out.changed = true;
    }
    if out.changed {
        out.inner.mark_changed();
    }
    (ran, out)
}

/// A switch (`<switch bind:active>`): track, knob, caption.
#[derive(Debug)]
pub struct Switch<'a> {
    c: Common,
    active: &'a mut bool,
    text: String,
}

impl<'a> Switch<'a> {
    pub fn new(active: &'a mut bool, text: impl Into<String>) -> Switch<'a> {
        Switch { c: Common::new("switch"), active, text: text.into() }
    }
    common_setters!();

    pub fn size(&self, ui: &egui::Ui, sv: &Sv) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, switch::UA, StateBits::NONE);
        checkbox::measure_indicator(ui.ctx(), &st.view(), switch::TRACK, &self.text, 20.0)
    }

    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let (_, rect) = ui.allocate_space(self.size(ui, sv));
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let Switch { c, active, text } = self;
        let (ran, mut out) = toggle(ui, &c, rect, active, ToggleParams { checked: *active, ..ToggleParams::default() });
        let st = sv.styled(ui.ctx(), &c.el, switch::UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            switch::paint(&mut ran.paint_cx("switch"), ui.ctx(), &st.view(), rect, ran.resp.id, &text, *active);
        }
        let (enabled, on) = (c.enabled(ui), *active);
        let info = || egui::WidgetInfo::selected(egui::Role::Switch, enabled, on, &text);
        out.inner = ran.finish(st.view().cursor(), Some(&info), |n| n.set_role(accesskit::Role::Switch), true, Some(FocusRole::Toggle), c.tooltip.as_deref());
        out
    }
}

/// A toggle button (`<toggle-button bind:checked>`); `radio`: a click only turns it on.
#[derive(Debug)]
pub struct ToggleButton<'a> {
    c: Common,
    checked: &'a mut bool,
    text: String,
    radio: bool,
    frame: bool,
    font_weight: i64,
}

impl<'a> ToggleButton<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<String>) -> ToggleButton<'a> {
        ToggleButton { c: Common::new("toggle-button"), checked, text: text.into(), radio: false, frame: true, font_weight: 400 }
    }
    common_setters!();
    pub fn radio(mut self, on: bool) -> Self {
        self.radio = on;
        self
    }
    pub fn frame(mut self, on: bool) -> Self {
        self.frame = on;
        self
    }
    pub fn font_weight(mut self, w: i64) -> Self {
        self.font_weight = w;
        self
    }

    pub fn size(&self, ui: &egui::Ui, sv: &Sv) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, toggle_button::UA, StateBits::NONE);
        toggle_button::measure(ui.ctx(), &st.view().with_font_weight(Some(self.font_weight)), &self.text)
    }

    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let (_, rect) = ui.allocate_space(self.size(ui, sv));
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let ToggleButton { c, checked, text, radio, frame, font_weight } = self;
        let (ran, mut out) = toggle(ui, &c, rect, checked, ToggleParams { checked: *checked, radio, ..ToggleParams::default() });
        let st = sv.styled(ui.ctx(), &c.el, toggle_button::UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            toggle_button::paint(&mut ran.paint_cx("toggle-button"), ui.ctx(), &st.view(), rect, &text, frame, font_weight);
        }
        let (enabled, on) = (c.enabled(ui), *checked);
        let (role, a11y_role) = if radio { (egui::Role::RadioButton, accesskit::Role::RadioButton) } else { (egui::Role::Button, accesskit::Role::Button) };
        let info = || egui::WidgetInfo::selected(role, enabled, on, &text);
        out.inner = ran.finish(st.view().cursor(), Some(&info), |n| n.set_role(a11y_role), true, Some(FocusRole::Toggle), c.tooltip.as_deref());
        out
    }
}

// ---- static ----

/// Static text (`<label>`); `wrap_mode` `wrap` (default), `truncate` or `extend`. Clickable only
/// when asked ([`Label::sense_click`]), as the kind is only when `clicked` is listened to.
#[derive(Clone, Debug)]
pub struct Label {
    c: Common,
    text: String,
    wrap_mode: &'static str,
    font_weight: i64,
    click: bool,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Label {
        Label { c: Common::new("label"), text: text.into(), wrap_mode: "wrap", font_weight: 400, click: false }
    }
    common_setters!();
    pub fn wrap_mode(mut self, mode: &'static str) -> Self {
        self.wrap_mode = mode;
        self
    }
    pub fn font_weight(mut self, w: i64) -> Self {
        self.font_weight = w;
        self
    }
    /// Takes clicks ([`Response::clicked`]).
    pub fn sense_click(mut self, on: bool) -> Self {
        self.click = on;
        self
    }

    /// Its size in `avail` (`wrap` wraps at a definite width).
    pub fn size(&self, ui: &egui::Ui, sv: &Sv, avail: AvailableSpace) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, label::UA, StateBits::NONE);
        label::measure(ui.ctx(), &st.view().with_font_weight(Some(self.font_weight)), &self.text, self.wrap_mode, avail)
    }

    /// Allocates its size (wrapping at `ui`'s available width) and shows it there.
    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let size = self.size(ui, sv, AvailableSpace::Definite(ui.available_width()));
        let (_, rect) = ui.allocate_space(size);
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let id = widget_id(ui, self.c.id);
        let enabled = self.c.enabled(ui);
        let press = self.click.then(PressParams::default);
        let ran = run(ui, id, rect, enabled, Compose { hover: true, focus: None, press, toggle: None }, StateBits::NONE);
        let mut out = resp(ran.resp.clone(), ran.state);
        out.activated = self.click && ran.press.iter().any(Press::is_click);
        let st = sv.styled(ui.ctx(), &self.c.el, label::UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            let s = st.view().with_font_weight(Some(self.font_weight));
            label::paint(&mut ran.paint_cx("label"), ui.ctx(), &s, rect, &self.text, self.wrap_mode);
        }
        let text = self.text.as_str();
        let info = || egui::WidgetInfo::labeled(egui::Role::Label, enabled, text);
        out.inner = ran.finish(
            None,
            Some(&info),
            |n| {
                n.set_role(accesskit::Role::Label);
                n.set_value(text);
            },
            false,
            None,
            self.c.tooltip.as_deref(),
        );
        out
    }
}

/// A horizontal (default) or vertical rule (`<separator>`).
#[derive(Clone, Debug)]
pub struct Separator {
    c: Common,
    vertical: bool,
}

impl Separator {
    pub fn horizontal() -> Separator {
        Separator { c: Common::new("separator"), vertical: false }
    }
    pub fn vertical() -> Separator {
        Separator { c: Common::new("separator"), vertical: true }
    }
    common_setters!();

    /// Allocates its thickness across `ui`'s available width (or height) and shows it there.
    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let st = sv.styled(ui.ctx(), &self.c.el, separator::UA, StateBits::NONE);
        let mut size = separator::measure(&st.view(), self.vertical);
        if self.vertical {
            size.y = ui.available_height();
        } else {
            size.x = ui.available_width();
        }
        let (_, rect) = ui.allocate_space(size);
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let id = widget_id(ui, self.c.id);
        let enabled = self.c.enabled(ui);
        let ran = run(ui, id, rect, enabled, Compose::default(), StateBits::NONE);
        let out = resp(ran.resp.clone(), ran.state);
        let st = sv.styled(ui.ctx(), &self.c.el, separator::UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            separator::paint(&mut ran.paint_cx("separator"), &st.view(), rect, self.vertical);
        }
        let vertical = self.vertical;
        let inner = ran.finish(
            None,
            None,
            |n| {
                n.set_role(accesskit::Role::Splitter);
                n.set_orientation(if vertical { accesskit::Orientation::Vertical } else { accesskit::Orientation::Horizontal });
            },
            false,
            None,
            self.c.tooltip.as_deref(),
        );
        Response { inner, ..out }
    }
}

/// An icon (the paint primitive, ARCH-WIDGETS §1.1): `color` tints it, `font-size` (16 by
/// default, the codicon size) or `width`/`height` size it. The key resolves as the button's
/// `icon` (`name` in the `icons` family, or `family:name`); ICR1's registry re-points it.
#[derive(Clone, Debug)]
pub struct Icon {
    c: Common,
    key: IconKey,
    label: String,
}

impl Icon {
    pub fn new(key: &str) -> Icon {
        let key = match key.split_once(':') {
            Some((f, n)) => IconKey::new(f, n),
            None => IconKey::new("icons", key),
        };
        Icon { c: Common::new("icon"), key, label: String::new() }
    }
    common_setters!();
    /// Its accessible name (an icon with none is decoration).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }

    fn view<'s>(&self, st: &'s super::Styled) -> StyleView<'s> {
        st.view()
    }

    pub fn size(&self, ui: &egui::Ui, sv: &Sv) -> egui::Vec2 {
        let st = sv.styled(ui.ctx(), &self.c.el, button::ICON_UA, StateBits::NONE);
        paint::icon_size(ui.ctx(), &self.view(&st), &self.key)
    }

    pub fn show(self, ui: &mut egui::Ui, sv: &Sv) -> Response {
        let (_, rect) = ui.allocate_space(self.size(ui, sv));
        self.show_at(ui, rect, sv)
    }

    pub fn show_at(self, ui: &mut egui::Ui, rect: egui::Rect, sv: &Sv) -> Response {
        let id = widget_id(ui, self.c.id);
        let enabled = self.c.enabled(ui);
        let ran = run(ui, id, rect, enabled, Compose { hover: true, ..Compose::default() }, StateBits::NONE);
        let out = resp(ran.resp.clone(), ran.state);
        let st = sv.styled(ui.ctx(), &self.c.el, button::ICON_UA, ran.state);
        if ran.ui.is_rect_visible(rect) {
            paint::paint_icon(&mut ran.paint_cx("icon"), &self.view(&st), &self.key, rect);
        }
        let label = self.label.as_str();
        let info = || egui::WidgetInfo::labeled(egui::Role::Image, enabled, label);
        let inner = ran.finish(
            None,
            Some(&info),
            |n| {
                n.set_role(accesskit::Role::Image);
                if !label.is_empty() {
                    n.set_label(label);
                }
            },
            false,
            None,
            self.c.tooltip.as_deref(),
        );
        Response { inner, ..out }
    }
}
