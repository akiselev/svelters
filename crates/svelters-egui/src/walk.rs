//! The walk: lays out a window when it is stale and paints its visible nodes as egui widgets at the
//! rects taffy computed, culling containers by the clip rect; widget responses go to the outbox
//! ([`Out`]). Also the pass guard: JS must not run while the scene is drawn.
//!
//! Pure apart from the outbox: on a discarded pass it runs again with no input, so nothing here
//! counts or toggles anything except through widget responses (EGUI.md §2.1).
//!
//! Leaves are painted by `leaf.rs`; which windows are walked is `windows.rs`. Writer lanes: EMBED,
//! then EVENTS (propagation hooks). Its public items are also at the old path `crate::render`.

use crate::backend::EguiBackend;
use crate::layout;
use crate::scene::{Arena, Handle, W};
use egui::{Align, Layout, Ui, UiBuilder, Vec2};
use svelters_abi::Value;

/// What the walk produced for the host, flushed after the pass.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Event(Handle, u32, Vec<Value>),
    Changed(Handle, u32),
}

impl EguiBackend {
    pub(crate) fn assert_no_pass(&self, what: &str) {
        assert!(!self.0.pass_active.get(), "svelters-egui: {what} during the egui pass (JS must not run while the scene is drawn)");
    }

    /// Lays out (if needed) and paints window `w`'s content in `ui`'s available rect.
    pub(crate) fn window_ui(&self, ui: &mut egui::Ui, w: Handle, out: &mut Vec<Out>) {
        let rect = ui.available_rect_before_wrap();
        let size = rect.size();
        let mut scene = self.0.scene.borrow_mut();
        let stale = self.0.dirty.layout_dirty.get() || self.0.dirty.layout_size.borrow().get(&w) != Some(&size);
        if stale || scene.n(w).cache.is_empty() {
            let m = layout::Metrics::new(ui.ctx());
            let measured = layout::compute(&mut scene, &m, w, size);
            self.0.dirty.layout_size.borrow_mut().insert(w, size);
            self.bump(|s| {
                s.layouts += 1;
                s.measured += measured as u64;
            });
        }
        paint(&mut scene, w, rect.min.to_vec2(), ui, out);
        ui.allocate_rect(rect, egui::Sense::hover());
        drop(scene);
        // Every window shown this frame is laid out; a later window resets the flag too.
        self.0.dirty.layout_dirty.set(false);
    }
}

/// Paints node `h` (and its subtree) with rects offset by `origin` (the window content's screen
/// position).
pub fn paint(arena: &mut Arena, h: Handle, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let n = arena.n(h);
    if !n.c.visible {
        return;
    }
    if !n.vis.is_identity() {
        return paint_visual(arena, h, origin, ui, out);
    }
    paint_node(arena, h, origin, ui, out);
}

/// TR1 visual channels: the node's subtree in a child `Ui` with its opacity multiplied, its paint
/// cut at the node's (clipped) rect, its shapes moved by the transform, and its events dropped while
/// inert. The transform is visual only (egui's `with_visual_transform`): input still hits the
/// layout rect, and a non-uniform scale is drawn uniform (the geometric mean).
fn paint_visual(arena: &mut Arena, h: Handle, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let n = arena.n(h);
    let v = n.vis;
    let r = n.rect.translate(origin);
    let mut child = ui.new_child(UiBuilder::new().max_rect(ui.max_rect()).scope_id(n.id.with("vis")));
    if let Some(o) = v.opacity {
        child.multiply_opacity(o.clamp(0.0, 1.0));
    }
    if v.clip.is_some() {
        child.set_clip_rect(ui.clip_rect().intersect(r));
    }
    let mut dropped = Vec::new();
    let sink = if v.inert { &mut dropped } else { out };
    match v.transform {
        Some([tx, ty, sx, sy, ox, oy]) => {
            let s = (sx * sy).abs().sqrt();
            let o = r.min + egui::vec2(r.width() * ox, r.height() * oy);
            let t = egui::emath::TSTransform::new(o.to_vec2() + egui::vec2(tx, ty) - o.to_vec2() * s, s);
            child.with_visual_transform(t, |ui| paint_node(arena, h, origin, ui, sink));
        }
        None => paint_node(arena, h, origin, &mut child, sink),
    }
}

fn paint_node(arena: &mut Arena, h: Handle, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let n = arena.n(h);
    let r = n.rect.translate(origin);
    if matches!(n.w, W::Box | W::Window { .. }) {
        // Containers draw nothing: cull whole subtrees outside the clip rect.
        if !ui.is_rect_visible(r) {
            return;
        }
        let kids = n.children.clone();
        for c in kids {
            paint(arena, c, origin, ui, out);
        }
        return;
    }
    if !ui.is_rect_visible(r) {
        return;
    }
    let mut b = UiBuilder::new()
        .max_rect(r)
        // Never the widget's own id: a scope sharing it eats the widget's clicks (EGUI.md §1.4).
        .scope_id(n.id.with("scope"))
        .layout(Layout::left_to_right(Align::Center));
    if !n.c.enabled {
        b = b.disabled();
    }
    let mut child = ui.new_child(b);
    if matches!(arena.n(h).w, W::Ext(_)) {
        crate::provider::paint(arena, h, &mut child, out); // RA1: a provider kind
    } else {
        crate::leaf::leaf(arena, h, &mut child, out);
    }
}
