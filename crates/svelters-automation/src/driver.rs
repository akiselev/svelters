//! The [`Driver`]: what a [`Session`](crate::Session) needs from whatever runs the frames.
//!
//! svelters implements it for its kittest harnesses (feature `kittest`): [`TestApp`] and
//! [`EmbedHarness`]. cadatomic's `KittestDrive` wraps one; its windowed app implements it over its
//! own frame loop (AU2 adds the IPC `Driver`).
//!
//! A [`Session`](crate::Session) runs on a [`SourceDriver`]: frames and input as here, over any
//! [`SceneSource`]. Every [`Driver`] is one (its scene is svelters-egui's); skelter-test's
//! `SkelterNativeDriver` is one over skelter-native UI.

use crate::Error;
use crate::source::SceneSource;
use std::fmt::Debug;
use std::hash::Hash;
use svelters_egui::scene::{ChangeFilter, NodeId, Observer, ProviderId, SceneHandle, SceneRef};

/// An AccessKit node of the last frame (the semantic tier's targets, `a11y.*` reads).
#[derive(Clone, Debug)]
pub struct A11yNode {
    pub node: egui::accesskit::NodeId,
    pub tree: egui::accesskit::TreeId,
    pub role: egui::accesskit::Role,
    pub label: Option<String>,
    pub author_id: Option<String>,
    /// In egui points.
    pub rect: egui::Rect,
}

/// Runs frames and takes input for a session. Every method runs on the UI thread, between frames.
pub trait Driver {
    /// The scene the session reads.
    fn scene(&self) -> SceneHandle;

    /// Why the app is not settled (`None`: settled): the embed's busy reasons
    /// (`Embed::settled`), named.
    fn busy(&self) -> Option<String>;

    /// Runs one frame, with the input queued by [`Driver::send`].
    fn step(&mut self);

    /// Queues `event` for the next frame.
    fn send(&mut self, event: egui::Event);

    /// Virtual milliseconds one [`Driver::step`] advances (the harness's step time). Waits and
    /// timeouts count these, so a 30 s timeout costs frames, not wall time.
    fn frame_ms(&self) -> f64;

    /// The AccessKit nodes of the last frame (empty: no AccessKit tree).
    fn a11y_nodes(&self) -> Vec<A11yNode> {
        Vec::new()
    }

    /// The last frame as an image (`None`: this driver has no renderer).
    fn screenshot(&mut self) -> Option<egui::ColorImage> {
        None
    }

    /// AU1b: the virtual clock's milliseconds (`None`: the host keeps the wall clock). See
    /// [`crate::clock::Clocked`].
    fn clock_ms(&self) -> Option<f64> {
        None
    }

    /// AU1b: the seed of the host's scheduler and `Math.random` (`None`: unseeded).
    fn seed(&self) -> Option<u64> {
        None
    }

    /// AU7: while `true`, frames do not move the virtual clock (a foreground wall-clock busy
    /// source is busy: real work runs, a virtual timeout must not cut it off). Drivers without a
    /// virtual clock ignore it.
    fn hold_clock(&mut self, _held: bool) {}
}

/// A boxed driver (`svelters-run` picks its clock at run time).
impl<D: Driver + ?Sized> Driver for Box<D> {
    fn scene(&self) -> SceneHandle {
        (**self).scene()
    }
    fn busy(&self) -> Option<String> {
        (**self).busy()
    }
    fn step(&mut self) {
        (**self).step()
    }
    fn send(&mut self, event: egui::Event) {
        (**self).send(event)
    }
    fn frame_ms(&self) -> f64 {
        (**self).frame_ms()
    }
    fn a11y_nodes(&self) -> Vec<A11yNode> {
        (**self).a11y_nodes()
    }
    fn screenshot(&mut self) -> Option<egui::ColorImage> {
        (**self).screenshot()
    }
    fn clock_ms(&self) -> Option<f64> {
        (**self).clock_ms()
    }
    fn seed(&self) -> Option<u64> {
        (**self).seed()
    }
    fn hold_clock(&mut self, held: bool) {
        (**self).hold_clock(held)
    }
}

/// The scene changes since an observer's last take ([`SourceDriver::take_changes`]), coalesced per
/// node: each with what changed (svelters-egui: `ChangeMask` names).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes<N> {
    pub by_node: Vec<(N, String)>,
    /// Records were lost: re-read what you need.
    pub overflowed: bool,
}

/// What a [`Session`](crate::Session) runs on: frames and input (as [`Driver`]) and a scene of any
/// [`SceneSource`], with its change journal. Every [`Driver`] is one, over svelters-egui's scene;
/// implement it directly for other UI (skelter-test's `SkelterNativeDriver`). Every method runs on
/// the UI thread, between frames.
pub trait SourceDriver {
    /// The scene's node identity.
    type Node: Copy + Eq + Hash + Debug;
    /// The scene, as [`SourceDriver::read`] lends it.
    type Scene<'s>: SceneSource<Node = Self::Node>;
    /// A change subscription ([`SourceDriver::observe`]).
    type Observer;

    /// Runs `f` on the scene as it is now.
    fn read<R>(&self, f: impl for<'s> FnOnce(&Self::Scene<'s>) -> R) -> R;
    /// The number a node is in JSON (`ref.id`, `changed[].id`).
    fn node_raw(&self, n: Self::Node) -> u64;
    /// The node a JSON id names (`None`: no node has it).
    fn node_from_raw(&self, raw: u64) -> Option<Self::Node>;
    /// Starts collecting scene changes (`filter` may be ignored by scenes without one).
    fn observe(&self, filter: ChangeFilter) -> Self::Observer;
    /// The changes since `obs`'s last take.
    fn take_changes(&self, obs: &Self::Observer) -> Changes<Self::Node>;
    /// AX1: asks the scrollables around `n` to bring it into view at the next frame (scenes that
    /// cannot ignore it).
    fn scroll_into_view(&mut self, n: Self::Node, center: bool) {
        let _ = (n, center);
    }
    /// AX1: scrolls the virtual list `n` to row position `pos`.
    fn scroll_list_to(&mut self, n: Self::Node, pos: usize) -> Result<(), Error> {
        let _ = (n, pos);
        Err(Error::new("capability.absent", "this scene has no virtual lists"))
    }

    /// [`Driver::busy`].
    fn busy(&self) -> Option<String>;
    /// [`Driver::step`].
    fn step(&mut self);
    /// [`Driver::send`]: egui events in the scene's points.
    fn send(&mut self, event: egui::Event);
    /// [`Driver::frame_ms`].
    fn frame_ms(&self) -> f64;
    /// [`Driver::a11y_nodes`], in the scene's points.
    fn a11y_nodes(&self) -> Vec<A11yNode> {
        Vec::new()
    }
    /// [`Driver::screenshot`].
    fn screenshot(&mut self) -> Option<egui::ColorImage> {
        None
    }
    /// [`Driver::clock_ms`].
    fn clock_ms(&self) -> Option<f64> {
        None
    }
    /// [`Driver::seed`].
    fn seed(&self) -> Option<u64> {
        None
    }
    /// [`Driver::hold_clock`].
    fn hold_clock(&mut self, _held: bool) {}
}

/// svelters-egui's scene: the [`Driver`]'s [`SceneHandle`].
impl<D: Driver> SourceDriver for D {
    type Node = NodeId;
    type Scene<'s> = SceneRef<'s>;
    type Observer = Observer;

    fn read<R>(&self, f: impl for<'s> FnOnce(&SceneRef<'s>) -> R) -> R {
        self.scene().read(|s| f(&s))
    }
    fn node_raw(&self, n: NodeId) -> u64 {
        n.to_raw()
    }
    fn node_from_raw(&self, raw: u64) -> Option<NodeId> {
        NodeId::from_raw(raw)
    }
    fn observe(&self, filter: ChangeFilter) -> Observer {
        self.scene().observe(filter)
    }
    fn take_changes(&self, obs: &Observer) -> Changes<NodeId> {
        let set = self.scene().take_changes(obs);
        Changes { by_node: set.by_node().map(|(n, m)| (n, format!("{m:?}"))).collect(), overflowed: set.overflowed }
    }
    fn scroll_into_view(&mut self, id: NodeId, center: bool) {
        let _ = self.scene().write(ProviderId::APP, |m| {
            if let Some(mut n) = m.node(id) {
                n.scroll_into_view(center);
            }
        });
    }
    fn scroll_list_to(&mut self, id: NodeId, pos: usize) -> Result<(), Error> {
        let wrote = self
            .scene()
            .write(ProviderId::APP, |m| m.scroll_list_to(id, pos))
            .map_err(|_| Error::new("scene.busy", "the scene is being drawn"))?;
        wrote.map_err(|e| Error::new("params.invalid", e.to_string()))
    }

    fn busy(&self) -> Option<String> {
        Driver::busy(self)
    }
    fn step(&mut self) {
        Driver::step(self)
    }
    fn send(&mut self, event: egui::Event) {
        Driver::send(self, event)
    }
    fn frame_ms(&self) -> f64 {
        Driver::frame_ms(self)
    }
    fn a11y_nodes(&self) -> Vec<A11yNode> {
        Driver::a11y_nodes(self)
    }
    fn screenshot(&mut self) -> Option<egui::ColorImage> {
        Driver::screenshot(self)
    }
    fn clock_ms(&self) -> Option<f64> {
        Driver::clock_ms(self)
    }
    fn seed(&self) -> Option<u64> {
        Driver::seed(self)
    }
    fn hold_clock(&mut self, held: bool) {
        Driver::hold_clock(self, held)
    }
}

#[cfg(feature = "kittest")]
mod kittest_impl {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::NodeT;
    use svelters_egui::Embed;
    use svelters_egui::harness::{EmbedHarness, TestApp};

    fn busy(embed: &Embed) -> Option<String> {
        let s = embed.settled();
        (!s.is_settled()).then(|| format!("{:?}", s.busy))
    }

    fn a11y<S>(h: &Harness<'_, S>) -> Vec<A11yNode> {
        let root = h.root();
        let mut out = Vec::new();
        for n in std::iter::once(root).chain(h.root().children_recursive()) {
            let ak = n.accesskit_node();
            if ak.bounding_box().is_none() {
                continue;
            }
            let (node, tree) = ak.locate();
            out.push(A11yNode {
                node,
                tree,
                role: ak.role(),
                label: ak.label(),
                author_id: ak.author_id().map(str::to_string),
                rect: n.rect(),
            });
        }
        out
    }

    /// kittest advances its clock by `step_dt` per step and predicts the same `dt` in the input it
    /// sends (`RawInput::predicted_dt`; 1/4 s unless the harness was built `with_step_dt`).
    fn step_ms<S>(h: &Harness<'_, S>) -> f64 {
        f64::from(h.input().predicted_dt) * 1000.0
    }

    impl Driver for TestApp {
        fn scene(&self) -> SceneHandle {
            self.backend.scene()
        }
        fn busy(&self) -> Option<String> {
            busy(&self.embed)
        }
        fn step(&mut self) {
            self.harness.step();
        }
        fn send(&mut self, event: egui::Event) {
            self.harness.event(event);
        }
        fn frame_ms(&self) -> f64 {
            step_ms(&self.harness)
        }
        fn a11y_nodes(&self) -> Vec<A11yNode> {
            a11y(&self.harness)
        }
    }

    impl Driver for EmbedHarness {
        fn scene(&self) -> SceneHandle {
            self.embed.backend().scene()
        }
        fn busy(&self) -> Option<String> {
            busy(&self.embed)
        }
        fn step(&mut self) {
            self.harness.step();
        }
        fn send(&mut self, event: egui::Event) {
            self.harness.event(event);
        }
        fn frame_ms(&self) -> f64 {
            step_ms(&self.harness)
        }
        fn a11y_nodes(&self) -> Vec<A11yNode> {
            a11y(&self.harness)
        }
    }
}
