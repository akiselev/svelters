//! `frame`, svelters' own (OW1a2, ARCH-WIDGETS §6.1, WIDGET-PARITY §2.16, Qt's QGroupBox): a
//! container that paints a box per `frame-style` and the `caption` paint part, and lays out its
//! children inside the border, the UA padding and the caption row (`Widget::layout`; a CSS
//! `padding` adds to the UA's, as the wrapper's inner margin did). Replaces the `egui::Frame`
//! wrapper (`elements/panes.rs`).
//!
//! `checkable` (QGroupBox): an indicator before the caption (the checkbox's `indicator` and
//! `mark` parts); unchecking it disables the content (egui's disabled `Ui`, so nothing in it
//! takes input). AccessKit `Role::Group` named by the label. States: `:checked` (when
//! checkable), `:hover` over the caption, `:focus-visible` (the ring around the indicator).
//! DESIGN R16: a section is a 1px `border`, never a shadow; the floating styles (`window`, `menu`,
//! `popup`) cast `elev-*` shadows (R7).

use super::{checkbox, parts};
use crate::interact::{Focusable, Hoverable, Pressable, Toggle, ToggleParams, Toggleable};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, LayoutCx, MeasureCx, Part, StateBits, Widget, WidgetCx};
use crate::taffy::{LayoutInput, LayoutOutput};

const FOCUS: UaRule = UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]);
const RING: [(&str, crate::paint::UaValue); 3] = [("outline-color", Token("focus")), ("outline-width", Px(1.0)), ("outline-offset", Px(1.0))];

/// `frame-style="group"` (the default): a 1px `border` section, radius 4 (R16).
pub const GROUP: Ua = Ua(&[
    UaRule::base(&[
        ("border-color", Token("border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(6.0)),
        ("padding-bottom", Px(6.0)),
        ("gap", Px(6.0)),
        RING[0],
        RING[1],
        RING[2],
    ]),
    FOCUS,
]);
/// `canvas`: the field look (sunken content, R4's fill without the inset).
pub const CANVAS: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("control-bg")),
    ("border-color", Token("control-border")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("radius-xs")),
    ("gap", Px(6.0)),
])]);
/// `window`, `menu`, `popup`: floating surfaces (R7).
pub const FLOATING: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("elev-2")),
    ("padding-left", Px(6.0)),
    ("padding-right", Px(6.0)),
    ("padding-top", Px(6.0)),
    ("padding-bottom", Px(6.0)),
    ("gap", Px(6.0)),
])]);
/// `panel`: the panel fill, flat.
pub const PANEL: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("panel-bg")),
    ("padding-left", Px(8.0)),
    ("padding-right", Px(8.0)),
    ("padding-top", Px(2.0)),
    ("padding-bottom", Px(2.0)),
    ("gap", Px(6.0)),
])]);
/// `none`: no chrome.
pub const NONE: Ua = Ua(&[UaRule::base(&[("gap", Px(6.0)), RING[0], RING[1], RING[2]]), FOCUS]);

/// `::part(caption)`: the title, strong (QGroupBox::title).
pub const CAPTION: Ua = Ua(&[UaRule::base(&[("font-weight", Number(600.0)), ("color", Token("fg-strong"))])]);

/// A box around its content, with an optional caption (and check).
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "frame", caps = CONTAINER | CUSTOM_LAYOUT | PAINTS_BOX | FOCUSABLE)]
pub struct Frame {
    /// The caption above the content.
    #[prop(measure)]
    label: String,
    /// The chrome: `group`, `canvas`, `window`, `menu`, `popup`, `panel`, `none`.
    #[prop(measure, default = "group".to_string())]
    frame_style: String,
    /// WIDGET-PARITY §2.16 (QGroupBox `checkable`): a check before the caption; unchecked
    /// disables the content.
    #[prop(measure)]
    checkable: bool,
    /// Checked (`bind:checked`), when `checkable`.
    #[prop(bind, default = true)]
    checked: bool,
    /// The user toggled the check; carries the new state.
    #[event]
    changed: Event<bool>,
    #[behavior]
    hover: Hoverable,
    #[behavior(params = focus_params)]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    #[behavior(params = toggle_params)]
    toggle: Toggleable,
    #[part]
    caption: Part,
    #[part]
    indicator: Part,
    #[part]
    mark: Part,
}

impl Frame {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, ..ToggleParams::default() }
    }
    fn focus_params(&self) -> crate::interact::FocusParams {
        crate::interact::FocusParams { tab_stop: self.checkable, ..Default::default() }
    }
    fn ua(&self) -> Ua {
        match self.frame_style.as_str() {
            "canvas" => CANVAS,
            "window" | "menu" | "popup" => FLOATING,
            "panel" => PANEL,
            "none" => NONE,
            _ => GROUP,
        }
    }

    /// The caption row's size (indicator, gap, caption), zero without a label or check.
    fn caption_size(&self, ctx: &egui::Context, host: &StyleView<'_>, cap: &StyleView<'_>) -> egui::Vec2 {
        if self.label.is_empty() && !self.checkable {
            return egui::Vec2::ZERO;
        }
        let ind = parts::part_view(host, checkbox::INDICATOR, false);
        let indicator = self.checkable.then(|| egui::vec2(ind.px("width").unwrap_or(16.0), ind.px("height").unwrap_or(16.0)));
        let text = (!self.label.is_empty()).then(|| paint::measure_text(ctx, cap, &self.label, AvailableSpace::MaxContent));
        let gap = if indicator.is_some() && text.is_some() { host.px("gap").unwrap_or(6.0) } else { 0.0 };
        let (i, t) = (indicator.unwrap_or_default(), text.unwrap_or_default());
        egui::vec2(i.x + gap + t.x, i.y.max(t.y))
    }

    /// What the content is inset by: (left, right, top, bottom) = UA border + UA padding, plus the
    /// caption row and a gap on top.
    fn chrome(&self, cx: &MeasureCx<'_>) -> [f32; 4] {
        let ua = StyleView::ua_only(self.ua(), StateBits::default(), true);
        let [bl, br, bt, bb] = ua.sides("border-width");
        let [pl, pr, pt, pb] = ua.sides("padding");
        let host = cx.style(self.ua());
        let cap = host.part(cx.part_css("caption"), CAPTION);
        let c = self.caption_size(cx.ctx, &host, &cap);
        let top = if c.y > 0.0 { c.y + host.px("gap").unwrap_or(6.0) } else { 0.0 };
        [bl + pl, br + pr, bt + pt + top, bb + pb]
    }
}

impl Widget for Frame {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let [l, r, t, b] = self.chrome(cx);
        let host = cx.style(self.ua());
        let cap = host.part(cx.part_css("caption"), CAPTION);
        egui::vec2(self.caption_size(cx.ctx, &host, &cap).x + l + r, t + b)
    }

    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: LayoutInput) -> LayoutOutput {
        let add = cx.with_measure(|m| self.chrome(m));
        cx.padded_layout(inputs, add)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let ua = self.ua();
        let sense = if self.checkable { self.units_sense() } else { egui::Sense::hover() };
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        // only a click on the caption row (or Space on the focused frame) toggles the check
        let head = {
            let host = cx.style(ua);
            let cap = host.part(cx.part_css("caption"), CAPTION);
            paint::box_geom(&host, cx.rect()).content.top() + self.caption_size(ui.ctx(), &host, &cap).y
        };
        let on_head = resp.interact_pointer_pos().is_none_or(|p| p.y <= head);
        if self.checkable && on_head {
            for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
                if !cx.dispatch(&self.changed, checked).prevented {
                    cx.set(&mut self.checked, checked);
                }
            }
        }
        cx.set_state(StateBits::CHECKED, self.checkable && self.checked);
        if ui.is_rect_visible(cx.rect()) {
            let geom = cx.paint_box(ui, ua);
            let host = cx.style(ua);
            let cap = host.part(cx.part_css("caption"), CAPTION);
            let csz = self.caption_size(ui.ctx(), &host, &cap);
            if csz.y > 0.0 {
                let row = egui::Rect::from_min_size(geom.content.min, egui::vec2(geom.content.width(), csz.y));
                let mut pc = cx.paint_cx(ui);
                let mut text_left = row.left();
                if self.checkable {
                    let ind = parts::part_view(&host, checkbox::INDICATOR, false);
                    let isz = egui::vec2(ind.px("width").unwrap_or(16.0), ind.px("height").unwrap_or(16.0));
                    let ir = egui::Rect::from_min_size(egui::pos2(row.left(), row.center().y - isz.y / 2.0), isz);
                    paint::paint_box(&mut pc.part("indicator"), &ind, ir);
                    if self.checked {
                        let mark = parts::part_view(&host, checkbox::MARK, false);
                        parts::paint_mark(&mut pc.part("mark"), ir, mark.color("color").unwrap_or(egui::Color32::PLACEHOLDER), false);
                    }
                    paint::paint_outline(&mut pc, &host, ir);
                    text_left = ir.right() + host.px("gap").unwrap_or(6.0);
                }
                if !self.label.is_empty() {
                    let w = (row.right() - text_left).max(0.0);
                    let text = paint::elide(ui.ctx(), &cap, &self.label, paint::Elide::End, w);
                    let g = paint::text_galley(ui.ctx(), &cap, &text, None);
                    let tr = egui::Rect::from_min_max(egui::pos2(text_left, row.top()), row.max);
                    paint::paint_text(&mut pc.part("caption"), &cap, &g, tr, Align::of(&cap, Align::Start));
                }
            }
        }
        if self.checkable && !self.checked {
            ui.scope(|ui| {
                ui.disable();
                cx.children(ui);
            });
        } else {
            cx.children(ui);
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Group);
        if !self.label.is_empty() {
            node.set_label(self.label.as_str());
        }
        if self.checkable {
            self.units_a11y(node);
        }
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.checkable && self.units_claims(key, m)
    }
}
