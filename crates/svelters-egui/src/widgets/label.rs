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
    /// The cut, if any: `elide`, `wrap-mode="truncate"` (an end cut) or `text-overflow: ellipsis`.
    fn elision(&self, s: &paint::StyleView<'_>) -> Option<Elide> {
        Elide::parse(&self.elide).or_else(|| (self.wrap_mode == "truncate").then_some(Elide::End)).or_else(|| Elide::of_style(s))
    }

    /// The galley at `width` (RT2: the one place a rich label's layout job plugs in).
    fn galley(&self, ctx: &egui::Context, s: &paint::StyleView<'_>, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
        match (self.elision(s), self.wrap_mode.as_str(), width) {
            (Some(mode), _, Some(w)) => paint::text_galley(ctx, s, &paint::elide(ctx, s, &self.text, mode, w), None),
            (None, "wrap", Some(w)) => paint::text_galley(ctx, s, &self.text, Some(w)),
            _ => paint::text_galley(ctx, s, &self.text, None),
        }
    }
}

impl Widget for Label {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        match (self.wrap_mode.as_str(), avail) {
            ("wrap", AvailableSpace::Definite(w)) if self.elision(&s).is_none() => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::Definite(w)),
            _ => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let target = (!self.for_anchor.is_empty()).then(|| {
            let sc = cx.scene();
            let root = cx.node().root();
            sc.by_anchor(root, &self.for_anchor).map(|n| (n.id(), n.egui_id()))
        });
        let target = target.flatten();
        let clickable = cx.listens(&self.clicked) || target.is_some();
        let sense = if clickable { self.units_sense() } else { egui::Sense::hover() };
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        if clickable && fired.press.iter().any(|p| p.is_click()) {
            if cx.listens(&self.clicked) {
                cx.dispatch(&self.clicked, ());
            }
            if let Some((t, _)) = target {
                cx.focus_node(t);
            }
        }
        let mut elided = false;
        if ui.is_rect_visible(cx.rect()) {
            let s = cx.style(UA).with_font_weight(Some(self.font_weight));
            let w = (self.wrap_mode != "extend" || self.elision(&s).is_some()).then(|| cx.rect().width());
            let g = self.galley(ui.ctx(), &s, w);
            elided = g.job.text != self.text;
            paint::paint_text(&mut cx.paint_cx(ui), &s, &g, cx.rect(), Align::of(&s, Align::Start));
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
