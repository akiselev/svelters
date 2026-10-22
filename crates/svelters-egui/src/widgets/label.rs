//! `label`, svelters' own (OW1a, ARCH-WIDGETS §6.1): text; Hoverable, and Pressable only while a
//! runtime listens to `clicked` (so a label never steals clicks from what is under it).
//! Replaces the `egui::Label` wrapper behind the own-widget switch.
//!
//! `wrap-mode`: `wrap` (height for width), `truncate` (one line, elided at the end with `…`;
//! OW0b's text primitive adds `elide="middle"`/`"start"`), `extend` (one line, never wrapped).
//! Deferred to OW1a slice 2 (milestone record): `selectable` (a read-only TextInput: it is
//! accepted and stored, the text is not yet selectable), `for` (buddy).
//!
//! Rich text is RT2's (ci/rt2): when the label has run children, paint the galley of
//! `crate::rich::layout_job(ctx, style, &rich, colors, wrap, underline)` instead of
//! `Label::galley` (the one place below that builds the galley; measure likewise).

use crate::interact::{Hoverable, Pressable};
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
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
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    press: Pressable,
}

/// The galley of `text` under `s` for `wrap_mode` (`wrap`, `truncate`, `extend`) at `width`.
fn galley(ctx: &egui::Context, s: &paint::StyleView<'_>, text: &str, wrap_mode: &str, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
    match (wrap_mode, width) {
        ("truncate", Some(w)) => {
            let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
            let mut job = egui::text::LayoutJob::simple(text.to_string(), egui::FontId::proportional(s.font_size()), color, w.max(0.0));
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            ctx.fonts_mut(|f| f.layout_job(job))
        }
        ("wrap", Some(w)) => paint::text_galley(ctx, s, text, Some(w)),
        _ => paint::text_galley(ctx, s, text, None),
    }
}

/// Its size under `s` (with the `font-weight` prop) in `avail`.
pub(crate) fn measure(ctx: &egui::Context, s: &paint::StyleView<'_>, text: &str, wrap_mode: &str, avail: AvailableSpace) -> egui::Vec2 {
    match (wrap_mode, avail) {
        ("wrap", AvailableSpace::Definite(w)) => paint::measure_text(ctx, s, text, AvailableSpace::Definite(w)),
        _ => paint::measure_text(ctx, s, text, AvailableSpace::MaxContent),
    }
}

/// Paints `text` in `r` under `s`. The kind and `imm::Label` (OW-RUST) call it.
pub(crate) fn paint(pc: &mut paint::PaintCx<'_>, ctx: &egui::Context, s: &paint::StyleView<'_>, r: egui::Rect, text: &str, wrap_mode: &str) {
    let w = (wrap_mode != "extend").then(|| r.width());
    let g = galley(ctx, s, text, wrap_mode, w);
    paint::paint_text(pc, s, &g, r, Align::of(s, Align::Start));
}

impl Widget for Label {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        measure(cx.ctx, &cx.style(UA).with_font_weight(Some(self.font_weight)), &self.text, &self.wrap_mode, avail)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let clickable = cx.listens(&self.clicked);
        let sense = if clickable { self.units_sense() } else { egui::Sense::hover() };
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        if clickable && fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        if ui.is_rect_visible(cx.rect()) {
            let s = cx.style(UA).with_font_weight(Some(self.font_weight));
            paint(&mut cx.paint_cx(ui), ui.ctx(), &s, cx.rect(), &self.text, &self.wrap_mode);
        }
        let (enabled, text) = (cx.enabled(), &self.text);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, enabled, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Label);
        node.set_value(self.text.as_str());
    }
}
