//! `button`, svelters' own (OW0, ARCH-WIDGETS §6.1): box, text; Hoverable, Focusable, Pressable;
//! no parts. Replaces the `egui::Button` wrapper of `leaf.rs` behind the own-widget switch.
//!
//! Look (CSS, theme `sv.base`): `button`, `:hover`, `:active`, `:selected`, `:focus-visible`
//! (the focus ring is the outline), `:default`, `:disabled`. Where no rule sets a property the UA
//! table [`UA`] does, from the built-in VS Code theme's tokens. Signals as the built-in row:
//! `clicked`, `double-clicked`, `secondary-clicked`. `default`/`cancel` stay EV3's (stored in
//! `Common`; Enter or Escape in the focus scope activate it through `focus::take_activation`).

use crate::interact::{Focusable, Hoverable, Press, Pressable};
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};

/// The button's UA sheet (ARCH-WIDGETS §1.1): VS Code's secondary button, through the theme's
/// component tokens, with the focus ring as an outline (DESIGN R8: 1px `focus`, offset +1).
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
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// A push button.
#[svelters::widget(namespace = "sv", replaces = "button", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Button {
    /// The caption.
    #[prop(measure)]
    text: String,
    /// Drawn as selected (a toggle look); clicking does not change it.
    #[prop]
    selected: bool,
    /// Draws the background and border (otherwise only while hovered, pressed or selected).
    #[prop(default = true)]
    frame: bool,
    /// No vertical padding.
    #[prop(measure)]
    small: bool,
    /// CSS font weight of the caption (a prop the runtime sets beats CSS).
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// Pressed and released (or Space, Enter, AccessKit `Click`; Enter/Escape as default/cancel).
    #[event]
    clicked: Event<()>,
    #[event]
    double_clicked: Event<()>,
    #[event]
    secondary_clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    press: Pressable,
}

impl Widget for Button {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        let text = paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent);
        let mut extra = paint::box_extra(&s);
        if self.small {
            let [_, _, pt, pb] = s.sides("padding");
            extra.y -= pt + pb;
        }
        text + extra
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        // the prop mirror (§2.3): set before the units run, so one restyle covers both
        cx.set_state(StateBits::SELECTED, self.selected);
        let default = cx.node().common().default_button;
        cx.set_state(StateBits::DEFAULT, default);
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        // EV3: Enter or Escape on the scope's default or cancel button
        let mut clicks = (usize::from(crate::focus::take_activation(ui, cx.id())), 0, 0);
        for p in fired.press.iter() {
            match p {
                Press::Click { count: 1, button: egui::PointerButton::Primary, .. } => clicks.0 += 1,
                Press::Click { count: 2, button: egui::PointerButton::Primary, .. } => clicks.1 += 1,
                Press::Click { button: egui::PointerButton::Secondary, .. } => clicks.2 += 1,
                _ => {}
            }
        }
        // EV1: each is dispatched before a default action (a button has none)
        for _ in 0..clicks.0.min(1) {
            cx.dispatch(&self.clicked, ());
        }
        for _ in 0..clicks.1.min(1) {
            cx.dispatch(&self.double_clicked, ());
        }
        for _ in 0..clicks.2.min(1) {
            cx.dispatch(&self.secondary_clicked, ());
        }
        if ui.is_rect_visible(cx.rect()) {
            let st = cx.style(UA);
            let framed = self.frame || st.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::SELECTED.0));
            let geom = if framed { cx.paint_box(ui, UA) } else { paint::box_geom(&st, cx.rect()) };
            let mut content = geom.content;
            if self.small {
                content = geom.padding;
            }
            cx.paint_text(ui, UA, &self.text, content, Some(self.font_weight), Align::Center);
            cx.paint_outline(ui, UA);
        }
        let (enabled, selected, text) = (cx.enabled(), self.selected, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Button);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
