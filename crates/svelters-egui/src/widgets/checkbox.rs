//! `checkbox`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box, text, path; Hoverable, Focusable,
//! Pressable, Toggleable. Paint parts `indicator` (the box) and `mark` (check or dash), and the
//! caption (`label`, WIDGET-PARITY §2.2). Replaces the `egui::Checkbox` wrapper behind the
//! own-widget switch.
//!
//! States: `:hover`, `:active`, `:focus-visible` (the ring around the indicator, DESIGN R8),
//! `:checked`, `:indeterminate`, `:disabled`. The base sheet's `checkbox` rule styles the
//! indicator (see `parts.rs`); the UA tables below are VS Code's 18×18 radius-3 box (DESIGN §1.3)
//! with R17's on-fill.

use super::parts;
use crate::interact::{Focusable, Hoverable, Pressable, Toggle, ToggleParams, Toggleable};
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};

/// The host: the caption's colour and font, the gap, the focus ring (drawn around the indicator).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("gap", Px(6.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

// TODO(OW-CSS `::part`): a hard-coded part UA table; replace it with the part's computed style once the engine computes `::part()` styles (see `parts::part_view`).
/// `::part(indicator)`: the box.
pub const INDICATOR: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(18.0)),
        ("height", Px(18.0)),
        ("background-color", Token("checkbox-bg")),
        ("border-color", Token("checkbox-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(3.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::INDETERMINATE, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
]);

// TODO(OW-CSS `::part`): a hard-coded part UA table; replace it with the part's computed style once the engine computes `::part()` styles (see `parts::part_view`).
/// `::part(mark)`: the glyph's colour.
pub const MARK: Ua = Ua(&[UaRule::base(&[("color", Token("fg"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// A check box with a caption.
// the `#[part]` fields name the kind\'s `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "checkbox", caps = FOCUSABLE)]
pub struct Checkbox {
    /// The caption.
    #[prop(measure)]
    text: String,
    /// Checked (`bind:checked`); the user toggles it.
    #[prop(bind)]
    checked: bool,
    /// The user toggled it; carries the new state.
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
    #[part]
    indicator: crate::provider::Part,
    #[part]
    mark: crate::provider::Part,
}

impl Checkbox {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, ..ToggleParams::default() }
    }
}

/// The indicator's rect and the caption's rect in `r` (indicator left, vertically centred).
pub(crate) fn split(r: egui::Rect, ind: egui::Vec2, gap: f32) -> (egui::Rect, egui::Rect) {
    let i = egui::Rect::from_min_size(egui::pos2(r.left(), r.center().y - ind.y / 2.0), ind);
    let t = egui::Rect::from_min_max(egui::pos2(i.right() + gap, r.top()), r.max);
    (i, t)
}

/// Indicator, gap and caption: what a toggle with an indicator measures.
pub(crate) fn measure_with_indicator(cx: &MeasureCx<'_>, ua: Ua, ind: Ua, text: &str, default: f32) -> egui::Vec2 {
    let s = cx.style(ua);
    let isz = parts::part_size(&s, ind, default);
    if text.is_empty() {
        return isz;
    }
    let t = paint::measure_text(cx.ctx, &s, text, AvailableSpace::MaxContent);
    egui::vec2(isz.x + parts::gap(&s, 6.0) + t.x, isz.y.max(t.y))
}

impl Widget for Checkbox {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        measure_with_indicator(cx, UA, INDICATOR, &self.text, 18.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
            // EV1: dispatched before the default action (the prop write)
            if !cx.dispatch(&self.changed, checked).prevented {
                cx.set(&mut self.checked, checked);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            let host = cx.style(UA);
            let isz = parts::part_size(&host, INDICATOR, 18.0);
            let (ir, tr) = split(cx.rect(), isz, parts::gap(&host, 6.0));
            let mut pc = cx.paint_cx(ui);
            let ind = parts::part_view(&host, INDICATOR, true);
            paint::paint_box(&mut pc.part("indicator"), &ind, ir);
            if self.checked {
                let mark = parts::part_view(&host, MARK, false);
                let color = mark.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
                parts::paint_mark(&mut pc.part("mark"), ir, color, false);
            }
            if !self.text.is_empty() {
                let g = paint::text_galley(ui.ctx(), &host, &self.text, None);
                paint::paint_text(&mut pc.part("label"), &host, &g, tr, Align::Start);
            }
            paint::paint_outline(&mut pc, &host, ir);
        }
        let (enabled, checked, text) = (cx.enabled(), self.checked, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, checked, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::CheckBox);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
