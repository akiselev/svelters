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
use std::sync::Arc;
use crate::interact::{Focusable, Hoverable, Pressable, Toggle, ToggleParams, Toggleable};
use crate::paint::{self, Align, IconKey, PartItem, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};

/// The host: the caption's colour and font, the gap, the focus ring (drawn around the indicator).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("gap", Px(6.0)),
        ("align-items", Keyword("center")),
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

/// `::part(icon)` of the indicator kinds (16px, as the button's).
pub const ICON: Ua = Ua(&[UaRule::base(&[("font-size", Px(16.0))])]);

/// A check box with a caption.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "checkbox", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Checkbox {
    /// The caption.
    #[prop(measure)]
    text: String,
    /// Checked (`bind:checked`); the user toggles it.
    #[prop(bind)]
    checked: bool,
    /// WIDGET-PARITY §2.2: partially checked (`bind:indeterminate`), shown as a dash. The app
    /// writes it; a click clears it and checks the box (Qt, GTK).
    #[prop(bind)]
    indeterminate: bool,
    /// The user cycles unchecked → checked → indeterminate (Qt's tristate).
    #[prop]
    tristate: bool,
    /// An icon before the caption (a key, or `family:name`).
    #[prop(measure)]
    icon: String,
    /// The user toggled it; carries the new `checked` (`indeterminate` is announced as a prop).
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
    indicator: Part,
    #[part]
    mark: Part,
    #[part(name = "icon")]
    icon_part: Part,
    #[part(name = "label")]
    label_part: Part,
}

impl Checkbox {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, indeterminate: self.indeterminate, tristate: self.tristate, radio: false }
    }
}

/// An `icon` prop's key: `family:name`, or a name in the `icons` family.
pub(crate) fn icon_key(icon: &str) -> Option<IconKey> {
    (!icon.is_empty()).then(|| match icon.split_once(':') {
        Some((f, n)) => IconKey::new(f, n),
        None => IconKey::new("icons", icon),
    })
}

/// The content of an indicator kind, laid out by P3 in the host's content box: the `indicator`
/// part, an optional `icon` and the caption `label`, spaced by `gap` (`flex-direction`, `order`
/// and `display: none` on the parts apply). `f` gets the items, the icon and the caption galley.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_parts<'c, R>(
    ctx: &egui::Context,
    host: &StyleView<'_>,
    ind_name: &'static str,
    ind: &StyleView<'_>,
    part_css: impl Fn(&str) -> Option<&'c svelters_style::ComputedStyle>,
    text: &str,
    icon: &str,
    f: impl FnOnce(&[PartItem<'_>], Option<&IconKey>, Option<&Arc<egui::Galley>>) -> R,
) -> R {
    let icon_v = host.part(part_css("icon"), ICON);
    let label_v = host.part(part_css("label"), Ua::NONE);
    let key = icon_key(icon).and_then(|k| paint::content_icon(&icon_v, &k));
    let galley = (!text.is_empty()).then(|| paint::text_galley(ctx, &label_v, text, None));
    let isz = egui::vec2(ind.px("width").unwrap_or(16.0), ind.px("height").unwrap_or(16.0));
    let mut items = vec![PartItem { name: ind_name, style: ind, content: isz }];
    if let Some(k) = &key {
        items.push(PartItem { name: "icon", style: &icon_v, content: paint::icon_size(ctx, &icon_v, k) });
    }
    if let Some(g) = &galley {
        items.push(PartItem { name: "label", style: &label_v, content: g.size() });
    }
    f(&items, key.as_ref(), galley.as_ref())
}

/// The indicator's view: its `::part()` style when a sheet computed one, else `ua` (over the
/// host's computed style when `host_css`: the base sheet's `checkbox` rule styles the box).
pub(crate) fn indicator_view<'a>(host: &'a StyleView<'a>, css: Option<&'a svelters_style::ComputedStyle>, ua: Ua, host_css: bool) -> StyleView<'a> {
    match css {
        Some(c) => host.part(Some(c), ua),
        None => parts::part_view(host, ua, host_css),
    }
}

/// What an indicator kind measures: its parts (P3) plus the host's padding and border.
pub(crate) fn measure_with_indicator(cx: &MeasureCx<'_>, ua: Ua, ind_name: &'static str, ind: Ua, text: &str, icon: &str, host_css: bool) -> egui::Vec2 {
    measure_indicator(cx.ctx, &cx.style(ua), |n| cx.part_css(n), ind_name, ind, text, icon, host_css)
}

/// [`measure_with_indicator`] under `host` (the style at rest) with `part_css` (the parts'
/// computed styles): the kind's and `imm`'s (OW-RUST).
#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_indicator<'c>(ctx: &egui::Context, host: &StyleView<'_>, part_css: impl Fn(&str) -> Option<&'c svelters_style::ComputedStyle>, ind_name: &'static str, ind: Ua, text: &str, icon: &str, host_css: bool) -> egui::Vec2 {
    let ind_v = indicator_view(host, part_css(ind_name), ind, host_css);
    with_parts(ctx, host, ind_name, &ind_v, &part_css, text, icon, |items, _, _| paint::measure_parts(host, items)) + paint::box_extra(host)
}

#[allow(clippy::too_many_arguments)]
/// Paints an indicator kind: the indicator box, `mark` inside it (the check, the dot, the knob),
/// the icon and the caption, then the focus ring around the indicator (DESIGN R8).
pub(crate) fn paint_indicator_kind(
    ui: &egui::Ui,
    cx: &WidgetCx<'_>,
    ua: Ua,
    ind_name: &'static str,
    ind: Ua,
    host_css: bool,
    text: &str,
    icon: &str,
    mark: impl FnOnce(&mut paint::PaintCx<'_>, &StyleView<'_>, egui::Rect),
) {
    paint_indicator(&mut cx.paint_cx(ui), ui.ctx(), &cx.style(ua), |n| cx.part_css(n), cx.rect(), ind_name, ind, host_css, text, icon, mark);
}

/// [`paint_indicator_kind`] with no scene node: in `pc` at `rect` under `host` (the style in this
/// pass's state) with `part_css` (the parts' computed styles). The kind and `imm` (OW-RUST) call it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_indicator<'c>(
    pc: &mut paint::PaintCx<'_>,
    ctx: &egui::Context,
    host: &StyleView<'_>,
    part_css: impl Fn(&str) -> Option<&'c svelters_style::ComputedStyle>,
    rect: egui::Rect,
    ind_name: &'static str,
    ind: Ua,
    host_css: bool,
    text: &str,
    icon: &str,
    mark: impl FnOnce(&mut paint::PaintCx<'_>, &StyleView<'_>, egui::Rect),
) {
    let ind_v = indicator_view(host, part_css(ind_name), ind, host_css);
    let content = paint::box_geom(host, rect).content;
    let ir = with_parts(ctx, host, ind_name, &ind_v, &part_css, text, icon, |items, key, galley| {
        let rects = paint::layout_parts(host, content, items);
        let mut ir = None;
        for (item, r) in items.iter().zip(rects) {
            let Some(r) = r else { continue };
            match item.name {
                n if n == ind_name => {
                    paint::paint_box(&mut pc.part(ind_name), item.style, r);
                    ir = Some(r);
                }
                "icon" => {
                    if let Some(k) = key {
                        paint::paint_icon(&mut pc.part("icon"), item.style, k, r);
                    }
                }
                _ => {
                    if let Some(g) = galley {
                        paint::paint_text(&mut pc.part("label"), item.style, g, r, Align::Start);
                    }
                }
            }
        }
        ir
    });
    if let Some(ir) = ir {
        mark(pc, host, ir);
        paint::paint_outline(pc, host, ir);
    }
}

/// Paints the check box at `rect` under `host` with `part_css`: indicator, mark (check or dash),
/// icon, caption, focus ring. The kind and `imm::Checkbox` (OW-RUST) call it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint<'c>(pc: &mut paint::PaintCx<'_>, ctx: &egui::Context, host: &StyleView<'_>, part_css: impl Fn(&str) -> Option<&'c svelters_style::ComputedStyle>, rect: egui::Rect, text: &str, icon: &str, on: bool, partial: bool) {
    paint_indicator(pc, ctx, host, part_css, rect, "indicator", INDICATOR, true, text, icon, |pc, host, ir| {
        if on || partial {
            let mark = parts::part_view(host, MARK, false);
            let color = mark.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
            parts::paint_mark(&mut pc.part("mark"), ir, color, partial);
        }
    });
}

impl Widget for Checkbox {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        measure_with_indicator(cx, UA, "indicator", INDICATOR, &self.text, &self.icon, true)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        for Toggle::Toggled { checked, indeterminate } in fired.toggle.iter().copied() {
            // EV1: dispatched before the default action (the prop writes)
            if !cx.dispatch(&self.changed, checked).prevented() {
                cx.set(&mut self.checked, checked);
                cx.set(&mut self.indeterminate, indeterminate);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            paint(&mut cx.paint_cx(ui), ui.ctx(), &cx.style(UA), |n| cx.part_css(n), cx.rect(), &self.text, &self.icon, self.checked, self.indeterminate);
        }
        let (enabled, checked, text) = (cx.enabled(), self.checked && !self.indeterminate, &self.text);
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
