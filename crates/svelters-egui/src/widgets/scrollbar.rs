//! `scrollbar`, a new kind (OW2; WIDGET-PARITY §2.11 `QScrollBar`): a standalone scroll bar for
//! a view that scrolls itself. Scrollable over `value` from `min` to `max` (`page` sizes the
//! thumb); parts `scrollbar` (the track) and `thumb` with `:hover`, `:active`, `:dragging`.
//! Always shown (no fade); the look is a scroll area's bar ([`super::scroll_area::THUMB_UA`]).
//!
//! The wheel over it scrolls it; the thumb drags; a press on the track pages or jumps (per
//! `native::Policy`); AccessKit sees a `ScrollBar` with its value, step and Increment/Decrement.
//! No egui wrapper: always svelters' own ([`super::OWN_ONLY`]).

use super::pick;
use super::scroll_area::{THUMB_UA, TRACK_UA};
use crate::interact::{Scroll, ScrollParams, Scrollable, Unit};
use crate::paint::{self, StyleView, Ua};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Color32, Vec2, accesskit, vec2};

pub const UA: Ua = Ua(&[]);

/// A standalone scroll bar.
#[svelters::widget(namespace = "sv", replaces = "scrollbar", caps = PAINTS_BOX)]
pub struct Scrollbar {
    #[prop(bind)]
    value: f64,
    #[prop]
    min: f64,
    #[prop(default = 100.0)]
    max: f64,
    #[prop(default = 10.0)]
    page: f64,
    #[prop(default = 1.0)]
    step: f64,
    #[prop(default = "vertical")]
    orientation: String,
    #[event]
    changed: Event<f64>,
    #[state]
    scroll: Scrollable,
    #[state]
    thumb_state: StateBits,
}

impl Scrollbar {
    fn axis(&self) -> usize {
        if pick(&self.orientation, crate::kinds::ORIENTATION, "vertical") == "horizontal" { 0 } else { 1 }
    }
    /// (min, max, page), sanitised: finite, max ≥ min, page ≥ 0.
    fn range(&self) -> (f64, f64, f64) {
        let f = |v: f64, d: f64| if v.is_finite() { v } else { d };
        let min = f(self.min, 0.0);
        let max = f(self.max, 100.0).max(min);
        (min, max, f(self.page, 10.0).max(0.0))
    }
}

impl Widget for Scrollbar {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        let w = StyleView::ua_only(TRACK_UA, StateBits::NONE, true).px("width").unwrap_or(10.0);
        let _ = cx;
        if self.axis() == 0 { vec2(0.0, w) } else { vec2(w, 0.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let rect = cx.rect();
        let dark = cx.dark();
        let d = self.axis();
        let (min, max, page) = self.range();
        // content: the whole range plus a page; the viewport: a page
        let mut content = Vec2::ZERO;
        let mut view = Vec2::ZERO;
        content[d] = (max - min + page) as f32;
        view[d] = page as f32;
        let mut off = Vec2::ZERO;
        off[d] = (self.value.clamp(min, max) - min) as f32;
        self.scroll.set_offset(off);
        self.scroll.set_geometry(content, view, [None, None]);
        let min_thumb = StyleView::ua_only(THUMB_UA, StateBits::NONE, dark).px("min-height").unwrap_or(20.0);
        let mut show = [false, false];
        show[d] = true;
        // the bar is the whole rect
        let thick = rect.size()[1 - d];
        let bars = self.scroll.bar_geometry(rect, thick, min_thumb, show);
        self.scroll.set_geometry(content, view, bars);
        let area = cx.interact(ui, egui::Sense::click_and_drag());
        let b = bars[d].expect("its bar");
        let th = ui.interact(b.thumb, cx.id().with("thumb"), egui::Sense::drag());
        let (bn, tn) = if d == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
        let parts = crate::interact::PartResponses(vec![(bn, crate::interact::Resp::of(&area)), (tn, crate::interact::Resp::of(&th))]);
        let mut axes = [false, false];
        axes[d] = true;
        let params = ScrollParams { axes, chain: true, page_keys: false, arrow_keys: false };
        let mut fired = crate::interact::Intents::default();
        {
            let sc = &mut self.scroll;
            cx.run_units_with(ui, &area, &parts, |icx| sc.input(&params, icx, &mut fired));
        }
        let consumed = self.scroll.consumed();
        if consumed != Vec2::ZERO {
            ui.ctx().input_mut(|i| i.smooth_scroll_delta -= consumed);
        }
        // AccessKit Increment/Decrement (the unit handles the Scroll* actions)
        let step = if self.step.is_finite() && self.step > 0.0 { self.step } else { 1.0 };
        let mut v = min + self.scroll.offset()[d] as f64;
        ui.input(|i| {
            for e in &i.events {
                if let egui::Event::AccessKitActionRequest(r) = e
                    && r.target_node == area.id.accesskit_id()
                {
                    match r.action {
                        accesskit::Action::Increment => v = (v + step).min(max),
                        accesskit::Action::Decrement => v = (v - step).max(min),
                        _ => {}
                    }
                }
            }
        });
        self.thumb_state.set(StateBits::HOVERED, th.hovered());
        self.thumb_state.set(StateBits::PRESSED, th.is_pointer_button_down_on());
        self.thumb_state.set(StateBits::DRAGGING, self.scroll.dragging(d));
        if fired.iter().any(|i| matches!(i, Scroll::Scrolled { .. })) || v != self.value.clamp(min, max) {
            ui.ctx().request_repaint();
        }
        if v != self.value {
            cx.set(&mut self.value, v);
            cx.dispatch(&self.changed, v);
        }
        if let Some(t) = self.scroll.next_frame() {
            ui.ctx().request_repaint_after(t);
        }
        // paint: track, thumb (always shown)
        let mut pc = cx.paint_cx(ui);
        let track = StyleView::ua_only(TRACK_UA, StateBits::NONE, dark).color("background-color").unwrap_or(Color32::TRANSPARENT);
        let mut tp = pc.part("scrollbar");
        if track.a() > 0 {
            tp.painter.rect_filled(b.track, 0.0, track);
        }
        tp.record(paint::TraceKind::Box, b.track, track, egui::Stroke::NONE, 0.0);
        let t = StyleView::ua_only(THUMB_UA, self.thumb_state, dark);
        let fill = t.color("background-color").unwrap_or(Color32::GRAY);
        let mut tp = pc.part("thumb");
        tp.painter.rect_filled(b.thumb, t.px("border-radius").unwrap_or(0.0), fill);
        tp.record(paint::TraceKind::Box, b.thumb, fill, egui::Stroke::NONE, 0.0);
        area
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        let (min, max, _) = self.range();
        node.set_role(accesskit::Role::ScrollBar);
        node.set_orientation(if self.axis() == 0 { accesskit::Orientation::Horizontal } else { accesskit::Orientation::Vertical });
        node.set_numeric_value(self.value.clamp(min, max));
        node.set_min_numeric_value(min);
        node.set_max_numeric_value(max);
        if self.step.is_finite() && self.step > 0.0 {
            node.set_numeric_value_step(self.step);
        }
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
    }
}
