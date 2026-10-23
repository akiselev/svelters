//! `switch`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box, text, path; Hoverable, Focusable,
//! Pressable, Toggleable. Paint parts `track` and `knob`, and the caption (`label`). Replaces the
//! self-painted wrapper of `elements/inputs.rs` behind the own-widget switch.
//!
//! VS Code has no switch, so its look follows the user's Q4 decision: DESIGN R1–R18 with Fluent 2
//! metrics on the VS Code tokens (40×20 track, radius 10, 12 px knob that grows to 14 on hover).
//! R17: the state shows by fill **and** knob position, not colour alone: off is a bordered track
//! with an `fg` knob on the left, on is the accent fill with an `accent-fg` knob on the right.
//! The knob moves with egui's responsive bool animation (instant under reduced motion); OW-CSS's
//! `transition` on `--knob` takes over when it lands.

use super::{checkbox, parts};
use crate::interact::{Focusable, Hoverable, Pressable, Toggle, ToggleParams, Toggleable};
use crate::paint::{Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};

/// The host: caption, gap, focus ring (around the track, R8: standalone controls 1 px at +1).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("gap", Px(8.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

// The part's UA layer: `::part()` rules in a sheet override it (OW1a3, `parts::part_of`).
/// `::part(track)`.
pub const TRACK: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(40.0)),
        ("height", Px(20.0)),
        ("background-color", Token("control-bg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(10.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("accent")), ("border-color", Token("accent"))]),
    UaRule::when(StateBits(StateBits::CHECKED.0 | StateBits::HOVERED.0), &[("background-color", Token("accent-hover")), ("border-color", Token("accent-hover"))]),
]);

// The part's UA layer: `::part()` rules in a sheet override it (OW1a3, `parts::part_of`).
/// `::part(knob)`: its colour and size.
pub const KNOB: Ua = Ua(&[
    UaRule::base(&[("color", Token("fg")), ("width", Px(12.0)), ("height", Px(12.0))]),
    UaRule::when(StateBits::HOVERED, &[("width", Px(14.0)), ("height", Px(14.0))]),
    UaRule::when(StateBits::CHECKED, &[("color", Token("accent-fg"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// An on/off switch with a caption.
// the `#[part]` fields name the kind\'s `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "switch", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Switch {
    /// The caption (right of the track).
    #[prop(measure)]
    text: String,
    /// On (`bind:active`); the user toggles it.
    #[prop(bind)]
    active: bool,
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
    track: crate::provider::Part,
    #[part]
    knob: crate::provider::Part,
}

impl Switch {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.active, ..ToggleParams::default() }
    }
}

/// Paints the switch at `rect` under `host` with `part_css`: track, knob (moving to `active` over
/// the motion time unless reduced, animated under `id`), caption, the focus ring around the track.
/// The kind and `imm::Switch` (OW-RUST) call it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint<'c>(pc: &mut crate::paint::PaintCx<'_>, ctx: &egui::Context, host: &crate::paint::StyleView<'_>, part_css: impl Fn(&str) -> Option<&'c svelters_style::ComputedStyle>, rect: egui::Rect, id: egui::Id, text: &str, active: bool) {
    // OW1a3: `::part(knob)` rules reach the knob
    let knob_css = part_css("knob");
    checkbox::paint_indicator(pc, ctx, host, part_css, rect, "track", TRACK, false, text, "", |pc, host, tr| {
        let knob = parts::part_with(host, knob_css, KNOB, false);
        let d = parts::part_size(host, KNOB, 12.0).x.min(tr.height());
        let t = if parts::reduced_motion(ctx) { f32::from(u8::from(active)) } else { ctx.animate_bool_responsive(id.with("knob"), active) };
        let inset = tr.height() / 2.0;
        let x = egui::lerp((tr.left() + inset)..=(tr.right() - inset), t);
        let color = knob.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
        parts::paint_dot(&mut pc.part("knob"), egui::pos2(x, tr.center().y), d / 2.0, color, egui::Stroke::NONE);
    });
}

impl Widget for Switch {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        checkbox::measure_with_indicator(cx, UA, "track", TRACK, &self.text, "", false)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
            if !cx.dispatch(&self.changed, checked).prevented() {
                cx.set(&mut self.active, checked);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            paint(&mut cx.paint_cx(ui), ui.ctx(), &cx.style(UA), |n| cx.part_css(n), cx.rect(), cx.id(), &self.text, self.active);
        }
        let (enabled, on, text) = (cx.enabled(), self.active, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, enabled, on, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Switch);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
