//! `progress-bar` and `spinner`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box, text, path; no
//! units. Replace the `egui::ProgressBar`/`egui::Spinner` wrappers behind the own-widget switch.
//!
//! progress-bar: paint parts `track`, `fill` and `label` (WIDGET-PARITY §2.9). VS Code's progress
//! is a thin bar in `progress` (`progressBar.background`): the track is 4 px tall, centred, unless
//! the bar shows text (`text` or `show-percentage`), when it takes the node's height and the
//! label is centred on it. `animate` is the busy state: `:busy` and a sweep (a 30 % segment
//! crossing the track every `--sv-busy-period`, 1.5 s; static under reduced motion).
//! spinner: a 270° arc in `progress` turning once a second while `spinning` (`:busy`).

use super::parts;
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, MeasureCx, StateBits, Widget, WidgetCx};

pub const UA: Ua = Ua(&[UaRule::base(&[("text-align", Keyword("center")), ("width", Px(100.0))])]);

/// `::part(track)`.
pub const TRACK: Ua = Ua(&[UaRule::base(&[("background-color", Token("control-bg")), ("border-radius", Px(2.0)), ("height", Px(4.0))])]);

/// `::part(fill)`.
pub const FILL: Ua = Ua(&[UaRule::base(&[("background-color", Token("progress"))])]);

/// A horizontal progress bar.
// the `#[part]` fields name the kind\'s `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "progress-bar")]
pub struct ProgressBar {
    /// Done, from 0 to 1.
    #[prop]
    fraction: f64,
    /// Text on the bar.
    #[prop(measure)]
    text: String,
    /// Shows the percentage (when `text` is empty).
    #[prop(measure)]
    show_percentage: bool,
    /// Busy: progress without a known fraction (a sweep).
    #[prop]
    animate: bool,
    #[part]
    track: crate::provider::Part,
    #[part]
    fill: crate::provider::Part,
}

impl ProgressBar {
    fn fraction(&self) -> f32 {
        if self.fraction.is_finite() { self.fraction.clamp(0.0, 1.0) as f32 } else { 0.0 }
    }

    fn label(&self) -> Option<String> {
        if !self.text.is_empty() {
            Some(self.text.clone())
        } else if self.show_percentage {
            Some(format!("{}%", (self.fraction() * 100.0).round() as i64))
        } else {
            None
        }
    }
}

impl Widget for ProgressBar {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA);
        let w = s.px("width").unwrap_or(100.0);
        let th = parts::part_size(&s, TRACK, 4.0).y;
        match self.label() {
            Some(t) => {
                let t = paint::measure_text(cx.ctx, &s, &t, AvailableSpace::MaxContent);
                egui::vec2(w.max(t.x + 8.0), (t.y + 4.0).max(th))
            }
            None => egui::vec2(w, th),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        cx.set_state(StateBits::BUSY, self.animate);
        cx.commit_state();
        let resp = cx.interact(ui, egui::Sense::hover());
        let label = self.label();
        if ui.is_rect_visible(cx.rect()) {
            let host = cx.style(UA);
            let r = cx.rect();
            let track = parts::part_view(&host, TRACK, false);
            let th = if label.is_some() { r.height() } else { parts::part_size(&host, TRACK, 4.0).y.min(r.height()) };
            let tr = egui::Rect::from_center_size(r.center(), egui::vec2(r.width(), th));
            let mut pc = cx.paint_cx(ui);
            paint::paint_box(&mut pc.part("track"), &track, tr);
            let fill = parts::part_view(&host, FILL, false);
            let color = fill.color("background-color").unwrap_or(egui::Color32::PLACEHOLDER);
            let radius = track.px("border-radius").unwrap_or(0.0);
            let fr = if self.animate {
                let seg = 0.3 * tr.width();
                let t = if parts::reduced_motion(ui.ctx()) {
                    0.0
                } else {
                    ui.ctx().request_repaint();
                    ((ui.input(|i| i.time) / 1.5).fract()) as f32
                };
                let x = tr.left() - seg + t * (tr.width() + seg);
                egui::Rect::from_min_max(egui::pos2(x.max(tr.left()), tr.top()), egui::pos2((x + seg).min(tr.right()), tr.bottom()))
            } else {
                egui::Rect::from_min_size(tr.min, egui::vec2(tr.width() * self.fraction(), tr.height()))
            };
            if fr.width() > 0.0 {
                parts::paint_fill(&mut pc.part("fill"), fr, radius, color);
            }
            if let Some(t) = &label {
                let g = paint::text_galley(ui.ctx(), &host, t, None);
                paint::paint_text(&mut pc.part("label"), &host, &g, r, Align::of(&host, Align::Center));
            }
        }
        let enabled = cx.enabled();
        let text = label.unwrap_or_default();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, enabled, &text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::ProgressIndicator);
        if self.animate {
            node.set_busy();
        } else {
            node.set_numeric_value(f64::from(self.fraction()));
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(1.0);
        }
    }
}

/// The spinner's UA: the arc's colour and default size.
pub const SPINNER: Ua = Ua(&[UaRule::base(&[("color", Token("progress")), ("width", Px(16.0))])]);

/// A busy indicator.
#[svelters::widget(namespace = "sv", replaces = "spinner")]
pub struct Spinner {
    /// Side in points (default: CSS `width`, else 16).
    #[prop(measure)]
    size: Option<f64>,
    /// Turns (and is `:busy`); a stopped spinner keeps its space and draws nothing.
    #[prop(default = true)]
    spinning: bool,
}

impl Widget for Spinner {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let d = self.size.filter(|s| s.is_finite() && *s > 0.0).map(|s| s as f32).or_else(|| cx.style(SPINNER).px("width")).unwrap_or(16.0);
        egui::Vec2::splat(d)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        cx.set_state(StateBits::BUSY, self.spinning);
        cx.commit_state();
        let resp = cx.interact(ui, egui::Sense::hover());
        if self.spinning && ui.is_rect_visible(cx.rect()) {
            let s = cx.style(SPINNER);
            let r = cx.rect();
            let d = r.width().min(r.height());
            let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
            let mut pc = cx.paint_cx(ui);
            let w = pc.snap_width((d / 8.0).max(1.0));
            let radius = (d - w) / 2.0;
            let start = if parts::reduced_motion(ui.ctx()) {
                0.0
            } else {
                ui.ctx().request_repaint();
                (ui.input(|i| i.time).fract() * std::f64::consts::TAU) as f32
            };
            let n = 24;
            let pts: Vec<egui::Pos2> = (0..=n)
                .map(|k| {
                    let a = start + 1.5 * std::f32::consts::PI * k as f32 / n as f32;
                    r.center() + radius * egui::vec2(a.cos(), a.sin())
                })
                .collect();
            let stroke = egui::Stroke::new(w, color);
            pc.painter.add(egui::Shape::line(pts, stroke));
            pc.record(paint::TraceKind::Path, r, color, stroke, 0.0);
        }
        let enabled = cx.enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, enabled, ""));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::ProgressIndicator);
        if self.spinning {
            node.set_busy();
        }
    }
}
