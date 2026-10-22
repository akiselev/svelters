//! [`EguiBackend`]: the `svl_backend` over the scene, and the frame driver all runners share.
//!
//! This file holds the backend's state and its accessors. Its behaviour is split by concern
//! (ARCH-EMBED §9; each file adds an `impl EguiBackend` block):
//!
//! | File | Content |
//! |---|---|
//! | `frame.rs` | the frame ([`EguiBackend::ui`]): before and after the pass, closes, checkpoint, rAF, remeasure, repaint |
//! | `windows.rs` | presented windows as viewports ([`EguiBackend::root_window`]), the per-window walk |
//! | `walk.rs` | layout and paint of one window, the outbox, the pass guard |
//! | `ops.rs` | `impl Backend for EguiBackend` (the host ops) |
//! | `scene/access.rs` | read access for tests and devtools ([`EguiBackend::with_scene`]) |
//!
//! EM1 rebuilds the runners on `Embed` (`embed/`), which takes over this state per root and slot.

use crate::scene::{Handle, RootIx, SceneHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use svelters_abi::*;

/// The thread-safe half: `wake` may come from any thread.
#[derive(Default)]
pub(crate) struct Shared {
    pub wake: AtomicBool,
    /// Frames drawn, readable from any thread (idle measurements).
    pub frames: AtomicU64,
    pub ctx: Mutex<Option<egui::Context>>,
    /// Where pending JS work (events in the outbox, rAF, wakes, deferred writes) asks for a repaint
    /// (EM2): the viewport whose pass runs `logic` next. The root viewport, or a deferred
    /// viewport's own while its pass runs with the embed's begin-pass plugin (`embed/current.rs`).
    /// Shared with the waker, which may run on any thread.
    pub logic_vp: Mutex<Option<egui::ViewportId>>,
}

/// Counters for tests and measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames the backend drew (first passes).
    pub frames: u64,
    /// Checkpoints it ran.
    pub checkpoints: u64,
    /// Layout runs, and leaves measured by them.
    pub layouts: u64,
    pub measured: u64,
    /// Events and prop changes delivered to the host.
    pub events: u64,
    pub changes: u64,
}

pub(crate) struct Inner {
    /// The scene (ARCH-SCENE): columns, roots and per-root layout queues, behind its handle.
    pub scene: SceneHandle,
    pub host: Cell<*const svl_host>,
    pub shared: Arc<Shared>,
    pub deadline: Cell<Option<Instant>>,
    pub frame_pending: Cell<bool>,
    /// Events or prop changes were posted: the host needs a checkpoint.
    pub checkpoint_pending: Cell<bool>,
    pub in_frame: Cell<bool>,
    pub pass_active: Cell<bool>,
    pub first_frame_done: Cell<bool>,
    pub quitting: Cell<bool>,
    /// Presented windows in presentation order (the first live one is the root viewport).
    pub windows: RefCell<Vec<Handle>>,
    /// The user asked to close these windows (seen during a pass); decided before the next one.
    pub close_requests: RefCell<Vec<Handle>>,
    /// `close` was called on these windows.
    pub closes: RefCell<Vec<Handle>>,
    /// Title last sent per window viewport.
    pub titles: RefCell<std::collections::HashMap<Handle, String>>,
    pub stats: Cell<Stats>,
    /// TR2: the native animator (keyframes sent once, sampled per frame).
    pub anim: RefCell<crate::anim::Animator>,
    /// EM2 repaint routing (ARCH-EMBED §5.2, §5.5): scene roots changed by host ops during a frame,
    /// drained by `route_repaints` at the end of `Embed::logic`.
    pub touched: RefCell<Vec<RootIx>>,
    /// The viewport each embed slot's root is drawn in (`None`: hidden); roots not listed (windows)
    /// are drawn in the root viewport. Kept by `Embed` (`create_slot`, `set_viewport`, `set_visible`).
    pub root_vp: RefCell<std::collections::HashMap<RootIx, Option<egui::ViewportId>>>,
    /// DS1: the installed theme (`set_theme`), its generation (bumped per install), and whether
    /// the context still needs its Style (applied at the next logic).
    pub theme: RefCell<Option<crate::theme::Theme>>,
    pub theme_generation: Cell<u64>,
    pub theme_pending: Cell<bool>,
}

/// The egui backend. Cheap to clone (one shared state); the clone given to [`BackendVtable::new`]
/// and the ones kept by runners see the same scene.
#[derive(Clone)]
pub struct EguiBackend(pub(crate) Rc<Inner>);

impl Default for EguiBackend {
    fn default() -> Self {
        EguiBackend::new()
    }
}

impl EguiBackend {
    pub fn new() -> EguiBackend {
        crate::dialogs::link(); // WG4b: keeps `svelters:app/egui` registered
        let shared = Arc::new(Shared::default());
        let scene = SceneHandle::default();
        // Rust writes, deferred writes and their announcements need a logic pass: ask for a frame.
        let wake = shared.clone();
        scene.set_wake(move || {
            // the next logic runs them: the root viewport's (deferred viewports may not run logic)
            if let Some(ctx) = wake.ctx.lock().unwrap().as_ref() {
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        });
        EguiBackend(Rc::new(Inner {
            scene,
            host: Cell::new(std::ptr::null()),
            shared,
            deadline: Cell::new(None),
            frame_pending: Cell::new(false),
            checkpoint_pending: Cell::new(false),
            in_frame: Cell::new(false),
            pass_active: Cell::new(false),
            first_frame_done: Cell::new(false),
            quitting: Cell::new(false),
            windows: RefCell::new(Vec::new()),
            close_requests: RefCell::new(Vec::new()),
            closes: RefCell::new(Vec::new()),
            titles: RefCell::new(Default::default()),
            stats: Cell::new(Stats::default()),
            anim: RefCell::new(Default::default()),
            touched: RefCell::new(Vec::new()),
            root_vp: RefCell::new(Default::default()),
            theme: RefCell::new(None),
            theme_generation: Cell::new(0),
            theme_pending: Cell::new(false),
        }))
    }

    /// The `svl_backend` vtable over this backend (it keeps a clone).
    pub fn vtable(&self) -> BackendVtable {
        BackendVtable::new(self.clone())
    }

    /// A thread-safe counter of the frames drawn (for idle measurements from another thread).
    pub fn frame_counter(&self) -> Arc<dyn Fn() -> u64 + Send + Sync> {
        let shared = self.0.shared.clone();
        Arc::new(move || shared.frames.load(Ordering::Relaxed))
    }

    /// TR2/SV37: the motion settings (slow-motion factor, reduced motion), effective from the next frame.
    pub fn set_motion(&self, cfg: crate::anim::MotionConfig) {
        self.0.anim.borrow_mut().config = crate::anim::MotionConfig { slow: if cfg.slow > 0.0 { cfg.slow } else { 1.0 }, ..cfg };
        if let Some(ctx) = self.ctx() {
            ctx.request_repaint();
        }
    }

    /// TR2: animations the native animator is still playing (not yet finished).
    pub fn animations_running(&self) -> usize {
        self.0.anim.borrow().running_count()
    }

    pub fn motion(&self) -> crate::anim::MotionConfig {
        self.0.anim.borrow().config
    }

    pub fn stats(&self) -> Stats {
        self.0.stats.get()
    }

    /// True once the host asked the app to quit (`quit`); runners that own no OS window (kittest,
    /// an embedding app) read it.
    pub fn quit_requested(&self) -> bool {
        self.0.quitting.get()
    }

    pub(crate) fn host(&self) -> Option<HostRef<'static>> {
        let p = self.0.host.get();
        // SAFETY: the host attached through `run`/`start` outlives its attachment (it detaches
        // with `start(NULL)` before it is dropped).
        (!p.is_null()).then(|| unsafe { HostRef::from_raw(p) })
    }

    /// See `Shared::logic_vp`.
    pub(crate) fn logic_vp(&self) -> egui::ViewportId {
        self.0.shared.logic_vp.lock().unwrap().unwrap_or(egui::ViewportId::ROOT)
    }

    pub(crate) fn set_logic_vp(&self, vp: egui::ViewportId) {
        *self.0.shared.logic_vp.lock().unwrap() = Some(vp);
    }

    pub(crate) fn ctx(&self) -> Option<egui::Context> {
        self.0.shared.ctx.lock().unwrap().clone()
    }

    pub(crate) fn bump(&self, f: impl FnOnce(&mut Stats)) {
        let mut s = self.0.stats.get();
        f(&mut s);
        self.0.stats.set(s);
    }
}
