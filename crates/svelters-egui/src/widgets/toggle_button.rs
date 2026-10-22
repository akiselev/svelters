//! `toggle-button`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box, text; Hoverable, Focusable,
//! Pressable, Toggleable (radio: only turns on). Replaces the `egui::Button::selected` wrapper
//! behind the own-widget switch.
//!
//! Look: the button's (DESIGN R1–R3, R8) plus R17's on state: `:checked` fills with `toggle-on-bg`
//! and a 1 px `toggle-on-border` (VS Code's `inputOption.active*`, VIS1 `toggle-on-fg`).

use crate::interact::{FocusParams, Focusable, Hoverable, Pressable, Roving, RovingParams, Toggle, ToggleParams, Toggleable};
use crate::scene::NodeId;
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};

pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("button-bg")),
        ("color", Token("button-fg")),
        ("border-color", Token("button-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("padding-left", Token("button-pad-x")),
        ("padding-right", Token("button-pad-x")),
        ("padding-top", Token("button-pad-y")),
        ("padding-bottom", Token("button-pad-y")),
        ("text-align", Keyword("center")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border")), ("color", Token("fg"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// A button that stays pressed (`checked`); with `radio`, a click only turns it on.
#[svelters::widget(namespace = "sv", replaces = "toggle-button", caps = FOCUSABLE | PAINTS_BOX)]
pub struct ToggleButton {
    #[prop(measure)]
    text: String,
    #[prop(bind)]
    checked: bool,
    /// A click only turns it on (one of a set of mutually exclusive choices).
    #[prop]
    radio: bool,
    /// Draws the background and border (otherwise only while hovered, pressed or checked).
    #[prop(default = true)]
    frame: bool,
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// WIDGET-PARITY §2.1 (QButtonGroup): the sibling toggle buttons with the same `group` form an
    /// exclusive set (checking one unchecks the others) and one Tab stop the arrow keys move in.
    #[prop]
    group: String,
    #[event]
    changed: Event<bool>,
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior(params = focus_params)]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    #[behavior(params = toggle_params)]
    toggle: Toggleable,
    #[behavior(params = roving_params)]
    roving: Roving,
    /// This frame: in a group of several and not its Tab stop.
    roving_member: bool,
}

impl ToggleButton {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, radio: self.radio || !self.group.is_empty(), ..ToggleParams::default() }
    }
    fn focus_params(&self) -> FocusParams {
        FocusParams { tab_stop: !self.roving_member, ..FocusParams::default() }
    }
    fn roving_params(&self) -> RovingParams {
        RovingParams { on: !self.group.is_empty() }
    }

    /// The siblings in this button's `group` (itself included), in order, with `checked`.
    fn members(&self, cx: &mut WidgetCx<'_>, me: NodeId) -> Vec<(NodeId, bool)> {
        if self.group.is_empty() {
            return Vec::new();
        }
        let mut v = Vec::new();
        for id in cx.sibling_ids() {
            if id == me {
                v.push((id, self.checked));
            } else if let Some(Some(c)) = cx.with_peer::<ToggleButton, _>(id, |t, _| (t.group == self.group).then_some(t.checked)) {
                v.push((id, c));
            }
        }
        v
    }

    /// Unchecks the other members (each announces `changed(false)`).
    fn uncheck_others(&self, cx: &mut WidgetCx<'_>, members: &[(NodeId, bool)], keep: NodeId) {
        for &(id, on) in members {
            if id != keep && on {
                cx.with_peer::<ToggleButton, _>(id, |t, p| {
                    p.set(&mut t.checked, false);
                    p.emit(&t.changed, false);
                });
            }
        }
    }
}

impl Widget for ToggleButton {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let me = cx.node().id();
        let members = self.members(cx, me);
        let stop = members.iter().find(|m| m.1).or(members.first()).map(|m| m.0);
        self.roving_member = members.len() > 1 && stop != Some(me);
        let mut sense = self.units_sense();
        if self.roving_member {
            sense -= egui::Sense::FOCUSABLE;
        }
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
            if !cx.dispatch(&self.changed, checked).prevented {
                cx.set(&mut self.checked, checked);
                if checked {
                    self.uncheck_others(cx, &members, me);
                }
            }
        }
        // Roving: focus the member the arrow lands on and check it (the others follow)
        if let Some(r) = fired.roving.iter().last().copied().filter(|_| members.len() > 1) {
            let at = members.iter().position(|m| m.0 == me).unwrap_or(0);
            let to = members[r.target(at, members.len())].0;
            if to != me {
                cx.with_peer::<ToggleButton, _>(to, |t, p| {
                    p.focus();
                    if !t.checked {
                        p.set(&mut t.checked, true);
                        p.emit(&t.changed, true);
                    }
                });
                if self.checked {
                    cx.set(&mut self.checked, false);
                    cx.dispatch(&self.changed, false);
                }
                self.uncheck_others(cx, &members, to);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            let st = cx.style(UA);
            let framed = self.frame || st.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::CHECKED.0));
            let geom = if framed { cx.paint_box(ui, UA) } else { paint::box_geom(&st, cx.rect()) };
            cx.paint_text(ui, UA, &self.text, geom.content, Some(self.font_weight), Align::Center);
            cx.paint_outline(ui, UA);
        }
        let role = if self.radio || !self.group.is_empty() { egui::Role::RadioButton } else { egui::Role::Button };
        let (enabled, on, text) = (cx.enabled(), self.checked, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(role, enabled, on, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(if self.radio || !self.group.is_empty() { egui::accesskit::Role::RadioButton } else { egui::accesskit::Role::Button });
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
