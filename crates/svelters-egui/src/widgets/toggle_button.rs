//! `toggle-button`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box, text; Hoverable, Focusable,
//! Pressable, Toggleable (radio: only turns on). Replaces the `egui::Button::selected` wrapper
//! behind the own-widget switch.
//!
//! Look: the button's (DESIGN R1–R3, R8) plus R17's on state: `:checked` fills with `toggle-on-bg`
//! and a 1 px `toggle-on-border` (VS Code's `inputOption.active*`, VIS1 `toggle-on-fg`).

use crate::interact::{Focusable, Hoverable, Pressable, Toggle, ToggleParams, Toggleable};
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
    #[event]
    changed: Event<bool>,
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    #[behavior(params = toggle_params)]
    toggle: Toggleable,
}

impl ToggleButton {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, radio: self.radio, ..ToggleParams::default() }
    }
}

impl Widget for ToggleButton {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
            if !cx.dispatch(&self.changed, checked).prevented {
                cx.set(&mut self.checked, checked);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            let st = cx.style(UA);
            let framed = self.frame || st.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::CHECKED.0));
            let geom = if framed { cx.paint_box(ui, UA) } else { paint::box_geom(&st, cx.rect()) };
            cx.paint_text(ui, UA, &self.text, geom.content, Some(self.font_weight), Align::Center);
            cx.paint_outline(ui, UA);
        }
        let role = if self.radio { egui::Role::RadioButton } else { egui::Role::Button };
        let (enabled, on, text) = (cx.enabled(), self.checked, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(role, enabled, on, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(if self.radio { egui::accesskit::Role::RadioButton } else { egui::accesskit::Role::Button });
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
