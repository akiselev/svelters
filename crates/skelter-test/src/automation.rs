//! The automation bridge (feature `automation`): a `svelters_automation::Session` drives a skelter app
//! with the same selectors, actions, waits, journal and MCP methods as an egui backend, through one of
//! two drivers:
//! - [`SkelterNativeDriver`] (a `SourceDriver`): skelter-native UI, read as a [`SkelterScene`] (the
//!   window's AccessKit projection, island subtrees included), the session's input (in window
//!   points) translated as the platform would deliver it, the scene journal as a diff of the
//!   projection between takes; no egui;
//! - `SkelterDriver` (feature `automation-egui`, a `Driver`): svelters-egui's retained scene in an
//!   egui island.
//!
//! For `SkelterDriver`, the scene lives in an egui island (cadatomic's panes during the migration): the
//! driver is given the island's
//! [`SceneHandle`] and its AccessKit subtree, and translates between the scene's coordinates (the
//! island's points) and the window's:
//! - `send`: egui events from the session (synthetic input at a node, AccessKit requests) are moved
//!   into window points and injected through the harness's egui adapter, so they take skelter's input
//!   path into the island;
//! - `a11y_nodes`: the island's grafted nodes, in island points, with the island's tree id, so semantic
//!   actions name the island's tree (the island maps them back for egui);
//! - `screenshot`: skelter's render of the window, cropped to the island.

use crate::a11y::{AkNode, NodeKey};
use crate::harness::Harness;
use crate::scene_source::SkelterScene;
use skelter::seams::DocumentApi;
use skelter_core::Document;
use std::cell::RefCell;
use std::collections::HashMap;
use svelters_automation::input::Event;
use svelters_automation::{A11yNode, Changes, Image, SceneSource, SourceDriver, pos2};
#[cfg(feature = "automation-egui")]
pub use egui_island::SkelterDriver;

/// Extra busy reasons of the hosted app (cadatomic's idle barrier, `svelters_egui::Embed::settled`).
pub type BusyFn<D> = Box<dyn Fn(&D) -> Option<String>>;

/// `SkelterDriver`: svelters-egui's scene in an egui island (feature `automation-egui`).
#[cfg(feature = "automation-egui")]
mod egui_island {
    use super::*;
    use skelter_base::accesskit::TreeId;
    use skelter_base::geom::{Rect, Vec2};
    use svelters_automation::Driver;
    use svelters_egui::scene::SceneHandle;

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
                        rect: svelters_automation::Rect::from_min_max(
                            pos2(b.min.x - o.x, b.min.y - o.y),
                            pos2(b.max.x - o.x, b.max.y - o.y),
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
}

/// The document of a harness's state, for CSS selectors ([`SkelterNativeDriver::with_document`]).
pub type DocFn<D> = Box<dyn for<'a> Fn(&'a D) -> &'a Document>;

/// A `svelters_automation::SourceDriver` for skelter-native UI: a `Session` resolves selectors on the
/// window's [`SkelterScene`], acts through the harness's input (the session's events, already in
/// window points, become the platform's input: skelter's hit testing and dispatch), and counts frames
/// on the harness's virtual clock.
///
/// Node ids in JSON are numbers this driver hands out per node key, in the order it first reports them.
/// The scene journal (`journal.*`, an act's `changed`) is a diff of the projection: a node whose name,
/// value, states or rect changed, or that came or went, since the observer's last take.
pub struct SkelterNativeDriver<D: DocumentApi + 'static> {
    pub harness: Harness<D>,
    doc: Option<DocFn<D>>,
    busy: Option<BusyFn<D>>,
    ids: RefCell<Ids>,
}

#[derive(Default)]
struct Ids {
    keys: Vec<NodeKey>,
    raw: HashMap<NodeKey, u64>,
}

/// A node as the journal compares it: name, value, states, rect.
type Sig = (String, Option<String>, Vec<&'static str>, Option<[u32; 4]>);

/// A [`SkelterNativeDriver`] journal subscription: the projection at the last take.
pub struct NativeObserver {
    last: RefCell<HashMap<NodeKey, Sig>>,
}

impl<D: DocumentApi + 'static> SkelterNativeDriver<D> {
    pub fn new(harness: Harness<D>) -> Self {
        SkelterNativeDriver { harness, doc: None, busy: None, ids: RefCell::default() }
    }

    /// CSS selectors match through the document `doc` gives (skelter-core's matcher).
    pub fn with_document(mut self, doc: impl for<'a> Fn(&'a D) -> &'a Document + 'static) -> Self {
        self.doc = Some(Box::new(doc));
        self
    }

    /// The hosted app's own busy reasons, on top of the harness's.
    pub fn with_busy(mut self, busy: impl Fn(&D) -> Option<String> + 'static) -> Self {
        self.busy = Some(Box::new(busy));
        self
    }

    fn scene(&self) -> SkelterScene<'_> {
        let scene = SkelterScene::new(self.harness.a11y());
        match &self.doc {
            Some(doc) => scene.with_document(doc(self.harness.state())),
            None => scene,
        }
    }

    fn sigs(&self) -> HashMap<NodeKey, Sig> {
        let s = SkelterScene::new(self.harness.a11y());
        let bits = |r: svelters_automation::Rect| [r.min.x.to_bits(), r.min.y.to_bits(), r.max.x.to_bits(), r.max.y.to_bits()];
        self.harness
            .a11y()
            .walk()
            .iter()
            .map(|n| {
                let k = n.key();
                (k, (s.name(k).into_owned(), s.value(k), s.states(k), s.rect(k).map(bits)))
            })
            .collect()
    }
}

impl<D: DocumentApi + 'static> SourceDriver for SkelterNativeDriver<D> {
    type Node = NodeKey;
    type Scene<'s> = SkelterScene<'s>;
    type Observer = NativeObserver;
    type Filter = ();

    fn read<R>(&self, f: impl for<'s> FnOnce(&SkelterScene<'s>) -> R) -> R {
        f(&self.scene())
    }

    fn node_raw(&self, n: NodeKey) -> u64 {
        let mut ids = self.ids.borrow_mut();
        if let Some(&r) = ids.raw.get(&n) {
            return r;
        }
        ids.keys.push(n);
        let r = ids.keys.len() as u64;
        ids.raw.insert(n, r);
        r
    }

    fn node_from_raw(&self, raw: u64) -> Option<NodeKey> {
        let i = usize::try_from(raw).ok()?.checked_sub(1)?;
        self.ids.borrow().keys.get(i).copied()
    }

    fn observe(&self, _filter: ()) -> NativeObserver {
        NativeObserver { last: RefCell::new(self.sigs()) }
    }

    fn take_changes(&self, obs: &NativeObserver) -> Changes<NodeKey> {
        let now = self.sigs();
        let mut last = obs.last.borrow_mut();
        let mut by_node = Vec::new();
        // in tree order
        for n in self.harness.a11y().walk() {
            let k = n.key();
            let (Some(new), old) = (now.get(&k), last.get(&k)) else { continue };
            let what = match old {
                None => vec!["added"],
                Some(old) => [
                    (old.0 != new.0, "name"),
                    (old.1 != new.1, "value"),
                    (old.2 != new.2, "states"),
                    (old.3 != new.3, "rect"),
                ]
                .into_iter()
                .filter_map(|(changed, what)| changed.then_some(what))
                .collect(),
            };
            if !what.is_empty() {
                by_node.push((k, what.join(" | ")));
            }
        }
        let mut gone: Vec<NodeKey> = last.keys().filter(|k| !now.contains_key(k)).copied().collect();
        gone.sort_by_key(|k| (format!("{:?}", k.0), k.1.0));
        by_node.extend(gone.into_iter().map(|k| (k, "removed".to_string())));
        *last = now;
        Changes { by_node, overflowed: false }
    }

    fn busy(&self) -> Option<String> {
        self.harness.busy().or_else(|| self.busy.as_ref().and_then(|f| f(self.harness.state())))
    }

    fn step(&mut self) {
        self.harness.step();
    }

    fn send(&mut self, event: Event) {
        self.harness.event(event);
    }

    fn frame_ms(&self) -> f64 {
        self.harness.interval().0 as f64 / 1e6
    }

    fn a11y_nodes(&self) -> Vec<A11yNode> {
        let tree = self.harness.a11y();
        tree.walk()
            .into_iter()
            .filter_map(|n| {
                let b = n.bounds?;
                Some(A11yNode {
                    node: n.id,
                    tree: n.tree,
                    role: n.role(),
                    label: AkNode { info: n, tree }.label(),
                    author_id: n.author_id().map(str::to_owned),
                    rect: svelters_automation::Rect::from_min_max(pos2(b.min.x, b.min.y), pos2(b.max.x, b.max.y)),
                })
            })
            .collect()
    }

    fn screenshot(&mut self) -> Option<Image> {
        let img = self.harness.render().ok()?;
        let rgba = crate::snapshot::to_rgba_image(&img);
        Some(Image::from_rgba_unmultiplied([img.width as usize, img.height as usize], rgba.as_raw()))
    }

    fn clock_ms(&self) -> Option<f64> {
        Some(self.harness.now().0 as f64 / 1e6)
    }
}
