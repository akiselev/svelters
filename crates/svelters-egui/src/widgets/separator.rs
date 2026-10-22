//! `separator`, svelters' own (OW1a, ARCH-WIDGETS §6.1): box. Replaces the `egui::Separator`
//! wrapper behind the own-widget switch. A 1 px `border` hairline centred across the node, filling
//! it along `orientation` (DESIGN §2.3: separators stay flat). The line is the node's
//! `border-color` and `border-width` (CSS), else the UA's; its margin is the node's padding.

use crate::paint::{self, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, MeasureCx, Widget, WidgetCx};

pub const UA: Ua = Ua(&[UaRule::base(&[("border-color", Token("border")), ("border-width", Token("stroke")), ("padding", Px(3.0))])]);

/// A horizontal or vertical rule.
#[svelters::widget(namespace = "sv", replaces = "separator")]
pub struct Separator {
    /// `horizontal` (a line across, the default) or `vertical`.
    #[prop(measure, default = "horizontal".to_string())]
    orientation: String,
}

impl Separator {
    fn vertical(&self) -> bool {
        self.orientation == "vertical"
    }
}

/// Its thickness across under `s` (the length along is the layout's).
pub(crate) fn measure(s: &StyleView, vertical: bool) -> egui::Vec2 {
    let [l, r, t, b] = s.sides("padding");
    let w = s.sides("border-width")[2].max(1.0);
    if vertical { egui::vec2(l + w + r, 0.0) } else { egui::vec2(0.0, t + w + b) }
}

/// Paints the rule centred in `r` under `s`. The kind and `imm::Separator` (OW-RUST) call it.
pub(crate) fn paint(pc: &mut PaintCx<'_>, s: &StyleView, r: egui::Rect, vertical: bool) {
    let w = pc.snap_width(s.sides("border-width")[2].max(1.0));
    let color = s.color("border-top-color").or_else(|| s.color("border-color")).unwrap_or(egui::Color32::PLACEHOLDER);
    let line = if vertical { egui::Rect::from_center_size(r.center(), egui::vec2(w, r.height())) } else { egui::Rect::from_center_size(r.center(), egui::vec2(r.width(), w)) };
    let line = pc.snap_rect(line);
    pc.painter.rect_filled(line, 0.0, color);
    pc.record(paint::TraceKind::Box, line, color, egui::Stroke::NONE, 0.0);
}

impl Widget for Separator {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        measure(&cx.style(UA), self.vertical())
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, egui::Sense::hover());
        if ui.is_rect_visible(cx.rect()) {
            paint(&mut cx.paint_cx(ui), &cx.style(UA), cx.rect(), self.vertical());
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Splitter);
        node.set_orientation(if self.vertical() { egui::accesskit::Orientation::Vertical } else { egui::accesskit::Orientation::Horizontal });
    }
}
