//! `panel`, svelters' own (OW2, ARCH-WIDGETS §6.1): box; Resizable on its inner edge; parts
//! `separator` (the 1 px line on the inner edge) and `sash` (its hit area and R11 hover bar).
//! Replaces WG2b's panel chrome (`Visuals::panel_fill`, egui's resize grip).
//!
//! A flex box with the chrome background (`bg`, VS Code's `sideBar.background`), sized by `size`
//! along its side's axis (width for `left`/`right`, height for `top`/`bottom`; it does not
//! shrink). `resizable` makes the inner edge a sash: drag it (within `min-*`/`max-*`), or focus it
//! and use the arrow keys; `changed(size)` reports each new size and `bind:size` follows.

use super::{Len, pick};
use crate::interact::{Resizable, Resize, ResizeParams, Unit};
use crate::paint::{self, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Color32, Vec2, accesskit};

/// The panel: VS Code's side bar (`bg`).
pub const UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("bg"))])]);

/// `::part(separator)`: the inner edge line (`border`).
pub const SEPARATOR_UA: Ua = Ua(&[UaRule::base(&[("width", Token("stroke")), ("background-color", Token("border"))])]);

/// A side or top/bottom panel.
#[svelters::widget(namespace = "sv", replaces = "panel", caps = CONTAINER | PAINTS_BOX)]
pub struct Panel {
    /// `left`, `right`, `top` or `bottom`: where it sits; the sash is on the opposite edge.
    #[prop(default = "left")]
    side: String,
    /// Its width or height in points; null for its content's.
    #[prop(bind)]
    size: Len,
    #[prop]
    resizable: bool,
    /// Draws the line on the inner edge.
    #[prop(default = true)]
    separator: bool,
    /// The user resized it: the new size.
    #[event]
    changed: Event<f64>,
    #[state]
    sash: Resizable,
    #[state]
    sash_state: StateBits,
}

impl Panel {
    /// Its inner edge can be dragged.
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }

    fn side(&self) -> &'static str {
        pick(&self.side, crate::kinds::SIDE, "left")
    }
}

impl Widget for Panel {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        Vec2::ZERO
    }

    fn taffy_style(&self, s: &mut crate::taffy::Style) {
        if let Some(size) = self.size.0 {
            let d = crate::taffy::Dimension::length(size as f32);
            if matches!(self.side(), "top" | "bottom") {
                s.size.height = d;
            } else {
                s.size.width = d;
            }
            // a sized panel keeps its size: the rest of the row or column gives way
            s.flex_shrink = 0.0;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let r = cx.rect();
        let dark = cx.dark();
        paint::paint_box(&mut cx.paint_cx(ui), &cx.style(UA), r);
        let area = cx.interact(ui, egui::Sense::hover());
        cx.record_hit(ui);
        cx.children(ui);

        let side = self.side();
        let ax = if matches!(side, "top" | "bottom") { 1 } else { 0 };
        // the inner edge: right of a left panel, left of a right panel, ...
        let (at, sign) = match side {
            "right" => (r.left(), -1.0),
            "top" => (r.bottom(), 1.0),
            "bottom" => (r.top(), -1.0),
            _ => (r.right(), 1.0),
        };
        let sep = StyleView::ua_only(SEPARATOR_UA, self.sash_state, dark);
        let line = sep.px("width").unwrap_or(1.0);
        let mut lr = r;
        // the line inside the panel, along its inner edge
        if sign > 0.0 {
            lr.min[ax] = at - line;
            lr.max[ax] = at;
        } else {
            lr.min[ax] = at;
            lr.max[ax] = at + line;
        }
        let enabled = cx.enabled() && self.resizable;
        let mut shown = false;
        if enabled {
            let sash_ua = StyleView::ua_only(super::split_view::SASH_UA, self.sash_state, dark);
            let hit = sash_ua.px("--hit").unwrap_or(4.0);
            let mut hr = r;
            hr.min[ax] = at - hit / 2.0;
            hr.max[ax] = at + hit / 2.0;
            let id = cx.id().with("sash");
            let resp = ui.interact(hr, id, egui::Sense::click_and_drag());
            let c = cx.node().common().clone();
            let (lo, hi) = if ax == 0 { (c.min_width, c.max_width) } else { (c.min_height, c.max_height) };
            let p = ResizeParams {
                axis: ax,
                sign,
                min: lo.unwrap_or(0.0).max(0.0) as f32,
                max: hi.filter(|h| h.is_finite()).map_or(f32::INFINITY, |h| h as f32),
                step: 10.0,
                collapsible: false,
                double_click: false,
            };
            let unit = &mut self.sash;
            unit.set_size(self.size.0.map_or(r.size()[ax], |s| s as f32));
            let mut fired = crate::interact::Intents::default();
            crate::interact::run(ui, &resp, &mut self.sash_state, &crate::interact::PartResponses::default(), |icx| unit.input(&p, icx, &mut fired));
            cx.record_part_focus(ui, &resp);
            let name = c.accessible_name.clone();
            ui.ctx().accesskit_node_builder(resp.id, |nd| {
                unit.a11y(&p, nd);
                // a splitter needs a name: the panel's, else what it does
                nd.set_label(if name.is_empty() { "Resize panel".to_string() } else { name });
            });
            if unit.hovered() {
                ui.ctx().set_cursor_icon(if ax == 0 { egui::CursorIcon::ResizeColumn } else { egui::CursorIcon::ResizeRow });
            }
            if let Some(t) = unit.next_frame() {
                ui.ctx().request_repaint_after(t);
            }
            shown = unit.hover_shown();
            for i in fired {
                let v = match i {
                    Resize::Resize { size } | Resize::Commit { size } | Resize::Cancel { size } => size,
                    Resize::Start => continue,
                };
                let new = Len(Some(v as f64));
                if new != self.size {
                    cx.set(&mut self.size, new);
                    cx.dispatch(&self.changed, v as f64);
                    cx.relayout_self();
                    ui.ctx().request_repaint();
                }
            }
            if shown {
                let w = sash_ua.px("--hover-width").unwrap_or(4.0);
                let col = sash_ua.color("--hover-color").unwrap_or(Color32::TRANSPARENT);
                let mut bar = r;
                bar.min[ax] = at - w / 2.0;
                bar.max[ax] = at + w / 2.0;
                let mut pc = cx.paint_cx(ui);
                let mut pp = pc.part("sash");
                pp.painter.rect_filled(bar, 0.0, col);
                pp.record(paint::TraceKind::Box, bar, col, egui::Stroke::NONE, 0.0);
            }
        }
        if self.separator && !shown {
            let fill = sep.color("background-color").unwrap_or(Color32::TRANSPARENT);
            let mut pc = cx.paint_cx(ui);
            let mut pp = pc.part("separator");
            pp.painter.rect_filled(pp.snap_rect(lr), 0.0, fill);
            pp.record(paint::TraceKind::Box, lr, fill, egui::Stroke::NONE, 0.0);
        }
        area
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Pane);
    }
}
