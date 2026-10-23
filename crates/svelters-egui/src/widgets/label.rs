//! `label`, svelters' own (OW1a, ARCH-WIDGETS §6.1): text; Hoverable, and Pressable only while a
//! runtime listens to `clicked` (so a label never steals clicks from what is under it).
//! Replaces the `egui::Label` wrapper behind the own-widget switch.
//!
//! `wrap-mode`: `wrap` (height for width), `truncate` (one line, cut at the end with `…`),
//! `extend` (one line, never wrapped). OW1a2: `elide` (`start`, `middle`, `end`; WG6 QW13) cuts one
//! line to the width wherever `wrap-mode` says, and so does CSS `text-overflow: ellipsis`; an
//! elided label shows its full text as the hover text (without a `tooltip`) and accessible name.
//! `for` (Qt's buddy, WIDGET-PARITY §2.10) names a target's `anchor`: a click on the label focuses
//! it, and the label names it in AccessKit (`labelled_by`).
//! Deferred to OW1a3: `selectable` (a read-only TextInput: accepted and stored, not yet selectable).
//!
//! Rich text is RT2's (ci/rt2): when the label has run children, paint the galley of
//! `crate::rich::layout_job(ctx, style, &rich, colors, wrap, underline)` instead of
//! `Label::galley` (the one place below that builds the galley; measure likewise).

use crate::interact::{Hoverable, Pressable};
use crate::paint::{self, Align, Elide, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Widget, WidgetCx};

pub const UA: Ua = Ua(&[UaRule::base(&[("text-align", Keyword("start"))])]);

/// Static text.
#[svelters::widget(namespace = "sv", replaces = "label", caps = HEIGHT_FOR_WIDTH)]
pub struct Label {
    #[prop(measure)]
    text: String,
    /// `wrap`, `truncate` or `extend`.
    #[prop(measure, default = "wrap".to_string())]
    wrap_mode: String,
    /// The text can be selected and copied (slice 2: stored, not yet selectable).
    #[prop]
    selectable: bool,
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// WG6 QW13: cut one line with `…` to the width: `none`, `start`, `middle`, `end`.
    #[prop(measure, enumeration = "none|start|middle|end", default = "none".to_string())]
    elide: String,
    /// The `anchor` of the control this label names (Qt's buddy): a click focuses it.
    #[prop(rename = "for")]
    for_anchor: String,
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    press: Pressable,
}

impl Label {
    fn look(&self) -> Look<'_> {
        Look { text: &self.text, wrap_mode: &self.wrap_mode, elide: &self.elide }
    }
}

/// What a label shows, from its props: the kind's and `imm::Label`'s (OW-RUST).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Look<'a> {
    pub text: &'a str,
    /// `wrap`, `truncate` or `extend`.
    pub wrap_mode: &'a str,
    /// `none`, `start`, `middle`, `end`.
    pub elide: &'a str,
}

impl Look<'_> {
    /// The cut, if any: `elide`, `wrap-mode="truncate"` (an end cut) or `text-overflow: ellipsis`.
    fn elision(&self, s: &paint::StyleView<'_>) -> Option<Elide> {
        Elide::parse(self.elide).or_else(|| (self.wrap_mode == "truncate").then_some(Elide::End)).or_else(|| Elide::of_style(s))
    }

    /// The galley at `width` (RT2: the one place a rich label's layout job plugs in).
    fn galley(&self, ctx: &egui::Context, s: &paint::StyleView<'_>, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
        match (self.elision(s), self.wrap_mode, width) {
            (Some(mode), _, Some(w)) => paint::text_galley(ctx, s, &paint::elide(ctx, s, self.text, mode, w), None),
            (None, "wrap", Some(w)) => paint::text_galley(ctx, s, self.text, Some(w)),
            _ => paint::text_galley(ctx, s, self.text, None),
        }
    }
}

/// Its size under `s` (with the `font-weight` prop) in `avail`.
pub(crate) fn measure(ctx: &egui::Context, s: &paint::StyleView<'_>, look: &Look, avail: AvailableSpace) -> egui::Vec2 {
    match (look.wrap_mode, avail) {
        ("wrap", AvailableSpace::Definite(w)) if look.elision(s).is_none() => paint::measure_text(ctx, s, look.text, AvailableSpace::Definite(w)),
        _ => paint::measure_text(ctx, s, look.text, AvailableSpace::MaxContent),
    }
}

/// The galley in `r` under `s`, and the rect its text takes (the label's hit area and drawn
/// rect, as egui's label).
pub(crate) fn layout(ctx: &egui::Context, s: &paint::StyleView<'_>, look: &Look, r: egui::Rect) -> (std::sync::Arc<egui::Galley>, egui::Rect) {
    let w = (look.wrap_mode != "extend" || look.elision(s).is_some()).then(|| r.width());
    let g = look.galley(ctx, s, w);
    let size = g.size();
    let x = match Align::of(s, Align::Start) {
        Align::Start => r.left(),
        Align::Center => r.center().x - size.x / 2.0,
        Align::End => r.right() - size.x,
    };
    (g, egui::Rect::from_min_size(egui::pos2(x, r.center().y - size.y / 2.0), size))
}

/// Paints `g` (from [`layout`]) in `r` under `s`. The kind and `imm::Label` (OW-RUST) call it.
pub(crate) fn paint(pc: &mut paint::PaintCx<'_>, s: &paint::StyleView<'_>, g: &std::sync::Arc<egui::Galley>, r: egui::Rect) {
    paint::paint_text(pc, s, g, r, Align::of(s, Align::Start));
}

impl Widget for Label {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        measure(cx.ctx, &cx.style(UA).with_font_weight(Some(self.font_weight)), &self.look(), avail)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let target = (!self.for_anchor.is_empty()).then(|| {
            let sc = cx.scene();
            let root = cx.node().root();
            sc.by_anchor(root, &self.for_anchor).map(|n| (n.id(), n.egui_id()))
        });
        let target = target.flatten();
        let clickable = cx.listens(&self.clicked) || target.is_some();
        // the galley first: the label's hit area and drawn rect are its text (as egui's label)
        let (g, text_rect) = layout(ui.ctx(), &cx.style(UA).with_font_weight(Some(self.font_weight)), &self.look(), cx.rect());
        let elided = g.job.text != self.text;
        let sense = if clickable { self.units_sense() } else { egui::Sense::hover() };
        let resp = cx.interact_at(ui, text_rect, sense);
        let fired = self.run_units(cx, ui, &resp);
        if clickable && fired.press.iter().any(|p| p.is_click()) {
            if cx.listens(&self.clicked) {
                cx.dispatch(&self.clicked, ());
            }
            if let Some((t, _)) = target {
                cx.focus_node(t);
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            paint(&mut cx.paint_cx(ui), &cx.style(UA).with_font_weight(Some(self.font_weight)), &g, cx.rect());
        }
        if let Some((_, eid)) = target {
            // AccessKit: the label names its buddy (`labelled_by` on the target's node)
            let me = resp.id.accesskit_id();
            ui.ctx().accesskit_node_builder(eid, |n| n.push_labelled_by(me));
        }
        let (enabled, text) = (cx.enabled(), &self.text);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, enabled, text));
        if elided && cx.node().common().tooltip.is_empty() {
            return resp.on_hover_text(self.text.clone());
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Label);
        node.set_value(self.text.as_str());
    }
}
