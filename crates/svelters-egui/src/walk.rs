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
use crate::scene::{Arena, Handle};
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

    /// Lays out root `r` if it has queued work: the whole root when new, resized or its top
    /// changed, else only its dirty boundaries (`layout::run_root`).
    fn layout_root(&self, scene: &mut Arena, ctx: &egui::Context, r: crate::scene::RootIx, size: Vec2) {
        let due = scene.roots.get(r).is_some_and(|e| e.size != Some(size) || !e.dirty.relayout.is_empty() || e.dirty.full);
        if due {
            let m = layout::Metrics::new(ctx);
            if let Some(run) = layout::run_root(scene, &m, r, size) {
                self.count_layout(run);
            }
        }
    }

    /// Lays out window `w`'s root if it has queued work and paints its content in `ui`'s
    /// available rect.
    pub(crate) fn window_ui(&self, ui: &mut egui::Ui, w: Handle, out: &mut Vec<Out>) {
        let rect = ui.available_rect_before_wrap();
        let mut scene = self.0.scene.borrow_mut();
        let id = scene.live(w).expect("a live window");
        let r = scene.core[scene.ix(id)].root;
        crate::provider::pass::prepare_root(&mut scene, r); // SG4: Caps::PREPARE (§7.2 b)
        self.layout_root(&mut scene, ui.ctx(), r, rect.size());
        paint(&mut scene, w, rect.min.to_vec2(), ui, out);
        ui.allocate_rect(rect, egui::Sense::hover());
    }

    /// EM1: lays out (if needed) and paints slot `slot`'s subtree in `ui`'s available rect
    /// (ARCH-EMBED §5.1 b-c). A slot is its own scene root (`TreeRoot::Slot`): it is laid out
    /// again when its available size changed or its root queued work (style, structure, a changed
    /// measured size), only the dirty boundaries when the size is the same, whatever other slots
    /// and windows did. Returns the rect it took and the content size (its children's extent).
    pub(crate) fn slot_ui(&self, ui: &mut egui::Ui, slot: Handle, out: &mut Vec<Out>) -> (egui::Rect, Vec2) {
        let rect = ui.available_rect_before_wrap();
        let mut scene = self.0.scene.borrow_mut();
        let id = scene.live(slot).expect("a live slot");
        let i = scene.ix(id);
        let r = scene.core[i].root;
        // leaves whose text changed after this frame's logic (a host op in a fault disposal)
        if scene.roots.get(r).is_some_and(|e| !e.dirty.remeasure.is_empty()) {
            layout::remeasure(&mut scene, &layout::Metrics::new(ui.ctx()), r);
        }
        crate::provider::pass::prepare_root(&mut scene, r); // SG4: Caps::PREPARE (§7.2 b)
        self.layout_root(&mut scene, ui.ctx(), r, rect.size());
        paint_at(&mut scene, i, rect.min.to_vec2(), ui, out);
        // what the content needs: the extent of the slot's children (laid out from its origin)
        let content = scene.children[i].iter().map(|&c| scene.core[scene.ix(c)].rect).fold(Vec2::ZERO, |acc, r| {
            if r == egui::Rect::NOTHING { acc } else { acc.max(r.max.to_vec2()) }
        });
        drop(scene);
        ui.allocate_rect(rect, egui::Sense::hover());
        (rect, content)
    }
}

/// Paints node `h` (and its subtree) with rects offset by `origin` (the window content's screen
/// position).
pub fn paint(arena: &mut Arena, h: Handle, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let i = arena.ix(arena.live(h).expect("live node"));
    paint_at(arena, i, origin, ui, out);
}

/// SG4: a provider's `WidgetCx::children`/`child` draws its children through the walk.
pub(crate) fn paint_child(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    paint_at(arena, i, origin, ui, out);
}

fn paint_at(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    if !arena.common(i).visible {
        return;
    }
    if arena.core[i].flags.contains(crate::scene::NodeFlags::HAS_VISUAL) {
        return paint_visual(arena, i, origin, ui, out);
    }
    paint_node(arena, i, origin, ui, out);
}

/// TR1 visual channels: the node's subtree in a child `Ui` with its opacity multiplied, its paint
/// cut at the node's (clipped) rect, its shapes moved by the transform, and its events dropped while
/// inert. The transform is visual only (egui's `with_visual_transform`): input still hits the
/// layout rect, and a non-uniform scale is drawn uniform (the geometric mean).
fn paint_visual(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let v = arena.vis(i);
    let r = arena.core[i].rect.translate(origin);
    let mut child = ui.new_child(UiBuilder::new().max_rect(ui.max_rect()).scope_id(arena.core[i].egui_id.with("vis")));
    if let Some(o) = v.opacity {
        child.multiply_opacity(o.clamp(0.0, 1.0));
    }
    if v.clip.is_some() {
        child.set_clip_rect(ui.clip_rect().intersect(r));
    }
    if v.inert {
        // an exiting node leaves the accessibility tree
        ui.ctx().accesskit_node_builder(child.unique_id(), |b| b.set_hidden());
        // TR2b: and out of focus traversal: a disabled widget senses nothing, so Tab skips it and a
        // focused one loses focus. Disabled widgets are dimmed by `disabled_alpha`; it is 1 here, so
        // the outro's own opacity is the only fade.
        child.visuals_mut().disabled_alpha = 1.0;
        child.disable();
    }
    let mut dropped = Vec::new();
    let sink = if v.inert { &mut dropped } else { out };
    match v.transform {
        Some([tx, ty, sx, sy, ox, oy]) => {
            let s = (sx * sy).abs().sqrt();
            let o = r.min + egui::vec2(r.width() * ox, r.height() * oy);
            let t = egui::emath::TSTransform::new(o.to_vec2() + egui::vec2(tx, ty) - o.to_vec2() * s, s);
            child.with_visual_transform(t, |ui| paint_node(arena, i, origin, ui, sink));
        }
        None => paint_node(arena, i, origin, &mut child, sink),
    }
}

fn paint_node(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    // EV2b: a host with behaviours (`use:`) runs their `after_ui` hooks around its own paint
    if arena.core[i]
        .flags
        .contains(crate::scene::NodeFlags::HAS_ATTACHED)
    {
        return paint_host(arena, i, origin, ui, out);
    }
    paint_own(arena, i, origin, ui, out);
}

/// EV2b: [`paint_own`] with the attached behaviours' hooks (`provider/behavior.rs`). A container
/// host gets a response of its own at its rect, registered under its children.
fn paint_host(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let r = arena.core[i].rect.translate(origin);
    if !ui.is_rect_visible(crate::style::paint_bounds(arena, i, r)) {
        return;
    }
    let container = arena.w(i).is_container();
    let mut host = ui.new_child(
        UiBuilder::new()
            .max_rect(r)
            .scope_id(arena.core[i].egui_id.with("behaviors")),
    );
    crate::provider::behavior::around(
        arena,
        i,
        &mut host,
        out,
        |arena, out| paint_own(arena, i, origin, ui, out),
        container,
    );
}

/// Paints node `i` itself: a container's children, or a leaf; returns a leaf's response.
fn paint_own(
    arena: &mut Arena,
    i: usize,
    origin: Vec2,
    ui: &mut Ui,
    out: &mut Vec<Out>,
) -> Option<egui::Response> {
    let r = arena.core[i].rect.translate(origin);
    if arena.w(i).is_container() {
        // Cull whole subtrees outside the clip rect (CS5g: grown by the node's own shadows).
        if !ui.is_rect_visible(crate::style::paint_bounds(arena, i, r)) {
            return None;
        }
        // CS5e: its state and box (background, border), and the opacity of its subtree
        let opacity = crate::style::before_container(arena, i, r, ui);
        let mut faded;
        let ui = match opacity {
            Some(o) => {
                faded = ui.new_child(UiBuilder::new().max_rect(ui.max_rect()).scope_id(arena.core[i].egui_id.with("css")));
                faded.multiply_opacity(o);
                &mut faded
            }
            None => ui,
        };
        // By index, no clone: the walk never changes the structure (ARCH-SCENE §5.2).
        for k in 0..arena.children[i].len() {
            let c = arena.children[i][k].index() as usize;
            paint_at(arena, c, origin, ui, out);
        }
        return None;
    }
    if !ui.is_rect_visible(crate::style::paint_bounds(arena, i, r)) {
        return None;
    }
    let mut b = UiBuilder::new()
        .max_rect(r)
        // Never the widget's own id: a scope sharing it eats the widget's clicks (EGUI.md §1.4).
        .scope_id(arena.core[i].egui_id.with("scope"))
        .layout(Layout::left_to_right(Align::Center));
    if !arena.common(i).enabled {
        b = b.disabled();
    }
    let mut child = ui.new_child(b);
    // CS5e: this pass's state, the restyle it reaches, then the computed visuals on `child`
    crate::style::before_leaf(arena, i, r, &mut child);
    let resp = if matches!(arena.w(i), crate::scene::W::Ext(_)) {
        crate::provider::paint(arena, i, &mut child, origin, out) // RA1: a provider kind
    } else {
        Some(crate::leaf::leaf(arena, i, &mut child, out))
    };
    crate::scene::hit::record(arena, i, &child); // SG3: the viewport's hit list
    resp
}
