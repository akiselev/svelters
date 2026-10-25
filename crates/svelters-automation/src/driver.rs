//! The [`Driver`]: what a [`Session`](crate::Session) needs from whatever runs the frames.
//!
//! svelters implements it for its kittest harnesses (feature `kittest`): [`TestApp`] and
//! [`EmbedHarness`]. cadatomic's `KittestDrive` wraps one; its windowed app implements it over its
//! own frame loop (AU2 adds the IPC `Driver`).

use svelters_egui::scene::SceneHandle;

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
