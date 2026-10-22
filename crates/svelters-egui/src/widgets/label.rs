//! `label`, svelters' own (OW1a, ARCH-WIDGETS §6.1): text; Hoverable, and Pressable only while a
//! runtime listens to `clicked` (so a label never steals clicks from what is under it).
//! Replaces the `egui::Label` wrapper behind the own-widget switch.
//!
//! `wrap-mode`: `wrap` (height for width), `truncate` (one line, elided at the end with `…`;
//! OW0b's text primitive adds `elide="middle"`/`"start"`), `extend` (one line, never wrapped).
//! Deferred to OW1a slice 2 (milestone record): `selectable` (a read-only TextInput: it is
//! accepted and stored, the text is not yet selectable), `for` (buddy), Q4 inline rich text.

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

impl Label {
    fn galley(&self, ctx: &egui::Context, s: &paint::StyleView<'_>, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
        match (self.wrap_mode.as_str(), width) {
            ("truncate", Some(w)) => {
                let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
                let mut job = egui::text::LayoutJob::simple(self.text.clone(), egui::FontId::proportional(s.font_size()), color, w.max(0.0));
                job.wrap.max_rows = 1;
                job.wrap.break_anywhere = true;
                ctx.fonts_mut(|f| f.layout_job(job))
            }
            ("wrap", Some(w)) => paint::text_galley(ctx, s, &self.text, Some(w)),
            _ => paint::text_galley(ctx, s, &self.text, None),
        }
    }
}

impl Widget for Label {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        match (self.wrap_mode.as_str(), avail) {
            ("wrap", AvailableSpace::Definite(w)) => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::Definite(w)),
            _ => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent),
        }
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
            let w = (self.wrap_mode != "extend").then(|| cx.rect().width());
            let g = self.galley(ui.ctx(), &s, w);
            paint::paint_text(&mut cx.paint_cx(ui), &s, &g, cx.rect(), Align::of(&s, Align::Start));
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
