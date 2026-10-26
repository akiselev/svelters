//! The automation bridge (feature `automation`): [`SkelterDriver`] implements
//! `svelters_automation::Driver` for a skelter harness, so a `Session` drives a skelter app with the same
//! selectors, actions, waits and MCP methods as an egui backend.
//!
//! svelters-automation resolves selectors on svelters-egui's retained scene. On skelter that scene lives
//! in an egui island (cadatomic's panes during the migration): the driver is given the island's
//! [`SceneHandle`] and its AccessKit subtree, and translates between the scene's coordinates (the
//! island's points) and the window's:
//! - `send`: egui events from the session (synthetic input at a node, AccessKit requests) are moved
//!   into window points and injected through the harness's egui adapter, so they take skelter's input
//!   path into the island;
//! - `a11y_nodes`: the island's grafted nodes, in island points, with the island's tree id, so semantic
//!   actions name the island's tree (the island maps them back for egui);
//! - `screenshot`: skelter's render of the window, cropped to the island.

use crate::a11y::AkNode;
use crate::harness::Harness;
use skelter::seams::DocumentApi;
use skelter_base::accesskit::TreeId;
use skelter_base::geom::{Rect, Vec2};
use svelters_automation::{A11yNode, Driver};
use svelters_egui::scene::SceneHandle;

/// Extra busy reasons of the hosted app (`svelters_egui::Embed::settled`, cadatomic's idle barrier).
pub type BusyFn<D> = Box<dyn Fn(&D) -> Option<String>>;

pub struct SkelterDriver<D: DocumentApi + 'static> {
    pub harness: Harness<D>,
    scene: SceneHandle,
    island: TreeId,
    busy: Option<BusyFn<D>>,
}

impl<D: DocumentApi + 'static> SkelterDriver<D> {
    /// A driver for the scene rendered by the island whose AccessKit subtree is `island`.
    pub fn new(harness: Harness<D>, scene: SceneHandle, island: TreeId) -> Self {
        SkelterDriver {
            harness,
            scene,
            island,
            busy: None,
        }
    }

    /// The hosted app's own busy reasons, on top of the harness's.
    pub fn with_busy(mut self, busy: impl Fn(&D) -> Option<String> + 'static) -> Self {
        self.busy = Some(Box::new(busy));
        self
    }

    /// The island's window-space bounds: its subtree root's (`None`: not grafted yet).
    pub fn island_bounds(&self) -> Option<Rect> {
        self.harness
            .a11y()
            .walk()
            .into_iter()
            .find(|n| n.tree == self.island && n.parent.is_none_or(|p| p.0 != self.island))
            .and_then(|n| n.bounds)
    }

    /// The island's origin in window points.
    pub fn origin(&self) -> Option<Vec2> {
        self.island_bounds().map(|b| Vec2::new(b.min.x, b.min.y))
    }

    fn to_window(&self, e: egui::Event) -> egui::Event {
        let o = self.origin().unwrap_or(Vec2::ZERO);
        let shift = |p: egui::Pos2| egui::pos2(p.x + o.x, p.y + o.y);
        match e {
            egui::Event::PointerMoved(p) => egui::Event::PointerMoved(shift(p)),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => egui::Event::PointerButton {
                pos: shift(pos),
                button,
                pressed,
                modifiers,
            },
            egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                force,
            } => egui::Event::Touch {
                device_id,
                id,
                phase,
                pos: shift(pos),
                force,
            },
            other => other,
        }
    }
}

impl<D: DocumentApi + 'static> Driver for SkelterDriver<D> {
    fn scene(&self) -> SceneHandle {
        self.scene.clone()
    }

    fn busy(&self) -> Option<String> {
        self.harness.busy().or_else(|| {
            self.busy
                .as_ref()
                .and_then(|f| f(self.harness.state()))
        })
    }

    fn step(&mut self) {
        self.harness.step();
    }

    fn send(&mut self, event: egui::Event) {
        let e = self.to_window(event);
        self.harness.event(e);
    }

    fn frame_ms(&self) -> f64 {
        self.harness.interval().0 as f64 / 1e6
    }

    fn a11y_nodes(&self) -> Vec<A11yNode> {
        let o = self.origin().unwrap_or(Vec2::ZERO);
        let tree = self.harness.a11y();
        tree.walk()
            .into_iter()
            .filter(|n| n.tree == self.island)
            .filter_map(|n| {
                let b = n.bounds?;
                Some(A11yNode {
                    node: n.id,
                    tree: n.tree,
                    role: n.role(),
                    label: AkNode { info: n, tree }.label(),
                    author_id: n.author_id().map(str::to_owned),
                    rect: egui::Rect::from_min_max(
                        egui::pos2(b.min.x - o.x, b.min.y - o.y),
                        egui::pos2(b.max.x - o.x, b.max.y - o.y),
                    ),
                })
            })
            .collect()
    }

    fn screenshot(&mut self) -> Option<egui::ColorImage> {
        let img = self.harness.render().ok()?;
        let rgba = crate::snapshot::to_rgba_image(&img);
        let full = egui::ColorImage::from_rgba_unmultiplied(
            [img.width as usize, img.height as usize],
            rgba.as_raw(),
        );
        Some(match self.island_bounds() {
            Some(b) => full.region(
                &egui::Rect::from_min_max(egui::pos2(b.min.x, b.min.y), egui::pos2(b.max.x, b.max.y)),
                Some(self.harness.scale()),
            ),
            None => full,
        })
    }

    fn clock_ms(&self) -> Option<f64> {
        Some(self.harness.now().0 as f64 / 1e6)
    }
}
