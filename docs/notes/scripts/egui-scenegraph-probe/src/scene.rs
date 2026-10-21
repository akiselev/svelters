//! A scene: the arena plus the per-frame walk that emits egui calls, in two layout modes.
//!
//! - `Native`: containers map to egui layouts (`top_down` / `left_to_right`); optional culling uses
//!   the size each child took last frame to skip whole off-screen subtrees.
//! - `Taffy`: a mirrored `TaffyTree` computes rects (leaves measured with egui's own fonts) only
//!   when something is dirty; the walk paints visible nodes at fixed rects.
//!
//! Interaction never runs "JS" during the walk: responses push to `events` / `changed`, which the
//! caller drains after `run_ui` returns.

use crate::tree::{Arena, Handle, Kind};
use egui::{Align, Id, Layout, Rect, Ui, UiBuilder, Vec2, pos2, vec2};
use taffy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdMode {
    /// `Id::unique(identity)`: survives keyed reorders, reparenting and hot reload.
    Identity,
    /// `Id::unique(handle)`: survives reorders, not hot reload (new handles).
    Handle,
    /// egui's positional auto ids (no explicit ids at all).
    Positional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Native,
    Taffy,
}

pub struct Scene {
    pub arena: Arena,
    pub root: Handle,
    pub id_mode: IdMode,
    pub layout: LayoutMode,
    /// Native mode: skip off-screen children of columns using last frame's sizes.
    pub cull: bool,
    /// Wrap every leaf in its own `Ui` scope so its auto id is stable (costs one child `Ui`).
    pub scoped_leaves: bool,
    /// Wrap the root in a vertical `ScrollArea`.
    pub scroll: bool,
    pub events: Vec<(Handle, &'static str)>,
    pub changed: Vec<(Handle, &'static str)>,
    pub taffy: TaffyTree<Handle>,
    pub content_size: Vec2,
    pub visited: usize,
    /// Taffy mode: layout must be recomputed for this available width.
    layout_width: f32,
    /// Probe instrumentation: measure-function calls, time in taffy's compute (last layout).
    pub measure_calls: std::cell::Cell<usize>,
    pub compute_us: f64,
}

impl Scene {
    pub fn new(arena: Arena, root: Handle) -> Scene {
        Scene {
            arena,
            root,
            id_mode: IdMode::Identity,
            layout: LayoutMode::Native,
            cull: false,
            scoped_leaves: false,
            scroll: true,
            events: Vec::new(),
            changed: Vec::new(),
            taffy: TaffyTree::new(),
            content_size: Vec2::ZERO,
            visited: 0,
            layout_width: -1.0,
            measure_calls: std::cell::Cell::new(0),
            compute_us: 0.0,
        }
    }

    pub fn egui_id(&self, h: Handle) -> Option<Id> {
        match self.id_mode {
            IdMode::Identity => Some(Id::unique(("svl", self.arena.n(h).identity))),
            IdMode::Handle => Some(Id::unique(("svl-h", h))),
            IdMode::Positional => None,
        }
    }

    /// The whole frame for this scene.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.visited = 0;
        match self.layout {
            LayoutMode::Native => {
                if self.scroll {
                    egui::ScrollArea::vertical().id_salt("svl-root-scroll").show(ui, |ui| {
                        self.node_ui(ui, self.root);
                    });
                } else {
                    self.node_ui(ui, self.root);
                }
            }
            LayoutMode::Taffy => {
                let w = ui.available_width();
                self.layout_taffy(ui.ctx(), w);
                let size = self.content_size;
                egui::ScrollArea::vertical().id_salt("svl-root-scroll").show_viewport(ui, |ui, _vp| {
                    ui.set_min_size(size);
                    let origin = ui.max_rect().min.to_vec2();
                    self.paint_taffy(ui, self.root, origin);
                });
            }
        }
    }

    // ---------------------------------------------------------------- native layout mode

    fn node_ui(&mut self, ui: &mut Ui, h: Handle) -> Rect {
        self.visited += 1;
        let kind = self.arena.n(h).kind;
        let rect = if kind.is_container() {
            let layout = match kind {
                // Align::Center would make the row take the whole available height (egui centers
                // in the cross axis of the space it is given); `ui.horizontal` avoids this by
                // allocating an initial interact_size.y. Min keeps rows as tall as their content.
                Kind::Row => Layout::left_to_right(Align::Min),
                _ => Layout::top_down(Align::Min),
            };
            let mut b = UiBuilder::new().layout(layout);
            if let Some(id) = self.egui_id(h) {
                b = b.scope_id(id);
            }
            let cull = self.cull && kind == Kind::Column;
            ui.scope_builder(b, |ui| {
                let n = self.arena.n(h).children.len();
                for i in 0..n {
                    let c = self.arena.n(h).children[i];
                    if cull {
                        let sz = self.arena.n(c).last_size;
                        if sz != Vec2::ZERO {
                            let r = Rect::from_min_size(ui.cursor().min, sz);
                            if !ui.is_rect_visible(r) {
                                ui.allocate_space(sz);
                                continue;
                            }
                        }
                    }
                    let r = self.node_ui(ui, c);
                    let cn = self.arena.n_mut(c);
                    cn.last_size = r.size();
                    cn.screen_rect = r;
                }
            })
            .response
            .rect
        } else if self.scoped_leaves {
            let mut b = UiBuilder::new();
            if let Some(id) = self.egui_id(h) {
                b = b.scope_id(id.with("scope")); // never the widget's own id
            }
            ui.scope_builder(b, |ui| self.leaf_ui(ui, h)).inner
        } else {
            self.leaf_ui(ui, h)
        };
        if h == self.root {
            let n = self.arena.n_mut(h);
            n.last_size = rect.size();
            n.screen_rect = rect;
        }
        rect
    }

    fn leaf_ui(&mut self, ui: &mut Ui, h: Handle) -> Rect {
        let id = self.egui_id(h);
        let n = self.arena.n_mut(h);
        match n.kind {
            Kind::Label => ui.label(n.text.as_str()).rect,
            Kind::Button => {
                let r = ui.button(n.text.as_str());
                if r.clicked() && n.listen_clicked {
                    self.events.push((h, "clicked"));
                }
                r.rect
            }
            Kind::Checkbox => {
                let r = ui.checkbox(&mut n.checked, n.text.as_str());
                if r.changed() {
                    self.changed.push((h, "checked"));
                }
                r.rect
            }
            Kind::TextEdit => {
                let mut te = egui::TextEdit::singleline(&mut n.text).desired_width(120.0);
                if let Some(id) = id {
                    te = te.id(id);
                }
                // 0.37 kittest refuses inputs without an accessible name
                let r = ui.add(te).accessible_name("field");
                if r.changed() && n.observe_text {
                    self.changed.push((h, "text"));
                }
                r.rect
            }
            Kind::Column | Kind::Row => unreachable!(),
        }
    }

    // ---------------------------------------------------------------- taffy layout mode

    /// Mirrors the arena into the taffy tree (once; a backend would mirror insert/remove).
    pub fn build_taffy(&mut self) {
        fn go(s: &mut Scene, h: Handle) -> taffy::NodeId {
            let n = s.arena.n(h);
            let kind = n.kind;
            let children = n.children.clone();
            let tn = if kind.is_container() {
                let style = match kind {
                    Kind::Row => Style {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::CENTER,
                        gap: Size { width: length(8.0), height: length(0.0) },
                        ..Default::default()
                    },
                    _ => Style {
                        flex_direction: FlexDirection::Column,
                        gap: Size { width: length(0.0), height: length(4.0) },
                        ..Default::default()
                    },
                };
                let mut style = style;
                if h == s.root {
                    // a window root has a definite width; with `auto` it would shrink to fit and
                    // every row would re-layout whenever the widest row changes (correct CSS)
                    style.size = Size { width: percent(1.0), height: auto() };
                }
                let kids: Vec<taffy::NodeId> = children.iter().map(|&c| go(s, c)).collect();
                s.taffy.new_with_children(style, &kids).unwrap()
            } else {
                s.taffy.new_leaf_with_context(Style::default(), h).unwrap()
            };
            s.arena.n_mut(h).tnode = Some(tn);
            tn
        }
        self.taffy = TaffyTree::new();
        go(self, self.root);
        self.layout_width = -1.0;
    }

    /// Marks a leaf's measurement stale (text changed).
    pub fn mark_dirty(&mut self, h: Handle) {
        self.arena.n(h).intrinsic.set(None);
        if let Some(t) = self.arena.n(h).tnode {
            self.taffy.mark_dirty(t).unwrap();
        }
    }

    /// Recomputes layout if the width changed or a node is dirty. Returns true if it ran.
    pub fn layout_taffy(&mut self, ctx: &egui::Context, width: f32) -> bool {
        let root = self.arena.n(self.root).tnode.expect("build_taffy first");
        if width == self.layout_width && !self.taffy.dirty(root).unwrap() {
            return false;
        }
        self.layout_width = width;
        let style = ctx.global_style();
        let font = egui::TextStyle::Body.resolve(&style);
        let pad = style.spacing.button_padding;
        let interact = style.spacing.interact_size;
        let icon = style.spacing.icon_width + style.spacing.icon_spacing;
        let arena = &self.arena;
        let calls = &self.measure_calls;
        let t0 = std::time::Instant::now();
        self.taffy
            .compute_layout_with_measure(
                root,
                Size { width: AvailableSpace::Definite(width), height: AvailableSpace::MaxContent },
                |inputs, _id, ctxt, style| {
                    let Some(&mut h) = ctxt else {
                        return taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO);
                    };
                    let n = arena.n(h);
                    calls.set(calls.get() + 1);
                    taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, avail| {
                        if let Size { width: Some(w), height: Some(hh) } = known {
                            return Size { width: w, height: hh };
                        }
                        let wrap = match avail.width {
                            AvailableSpace::Definite(w) => w,
                            _ => f32::INFINITY,
                        };
                        // Intrinsic (no-wrap) size cached in the node until the text changes; the
                        // fonts are only asked again when the text must wrap.
                        let text = |extra: f32| {
                            let intrinsic = n.intrinsic.get().unwrap_or_else(|| {
                                let sz = ctx.fonts_mut(|f| {
                                    f.layout_no_wrap(n.text.clone(), font.clone(), egui::Color32::WHITE).size()
                                });
                                n.intrinsic.set(Some(sz));
                                sz
                            });
                            if intrinsic.x <= wrap - extra {
                                return intrinsic;
                            }
                            ctx.fonts_mut(|f| {
                                f.layout(n.text.clone(), font.clone(), egui::Color32::WHITE, (wrap - extra).max(0.0))
                                    .size()
                            })
                        };
                        let s = match n.kind {
                            Kind::Label => text(0.0),
                            Kind::Button => {
                                let t = text(2.0 * pad.x);
                                vec2(t.x + 2.0 * pad.x, (t.y + 2.0 * pad.y).max(interact.y))
                            }
                            Kind::Checkbox => {
                                let t = text(icon);
                                vec2(t.x + icon, t.y.max(interact.y))
                            }
                            Kind::TextEdit => vec2(120.0, interact.y),
                            _ => Vec2::ZERO,
                        };
                        Size { width: s.x.ceil(), height: s.y.ceil() }
                    })
                },
            )
            .unwrap();
        self.compute_us = t0.elapsed().as_secs_f64() * 1e6;
        // absolute rects
        fn place(s: &mut Scene, h: Handle, origin: egui::Pos2) {
            let t = s.arena.n(h).tnode.unwrap();
            let l = *s.taffy.layout(t).unwrap();
            let min = origin + vec2(l.location.x, l.location.y);
            let r = Rect::from_min_size(min, vec2(l.size.width, l.size.height));
            s.arena.n_mut(h).layout_rect = r;
            let n = s.arena.n(h).children.len();
            for i in 0..n {
                let c = s.arena.n(h).children[i];
                place(s, c, min);
            }
        }
        place(self, self.root, pos2(0.0, 0.0));
        self.content_size = self.arena.n(self.root).layout_rect.size();
        true
    }

    fn paint_taffy(&mut self, ui: &mut Ui, h: Handle, origin: Vec2) {
        let r = self.arena.n(h).layout_rect.translate(origin);
        if !ui.is_rect_visible(r) {
            return; // cull the whole subtree
        }
        self.visited += 1;
        let kind = self.arena.n(h).kind;
        if kind.is_container() {
            let n = self.arena.n(h).children.len();
            for i in 0..n {
                let c = self.arena.n(h).children[i];
                self.paint_taffy(ui, c, origin);
            }
            return;
        }
        let mut b = UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center));
        if let Some(id) = self.egui_id(h) {
            b = b.scope_id(id.with("scope")); // never the widget's own id (an Id clash eats the clicks)
        }
        let mut child = ui.new_child(b);
        let rr = self.leaf_ui(&mut child, h);
        self.arena.n_mut(h).screen_rect = rr;
    }

    /// Hit area of a node from last frame (used by tests to synthesize clicks).
    pub fn center_of(&self, h: Handle) -> egui::Pos2 {
        self.arena.n(h).screen_rect.center()
    }
}
