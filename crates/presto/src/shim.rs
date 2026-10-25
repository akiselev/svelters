//! Stand-ins for wave-1 APIs that are not in `ci/presto-base` yet (`plans/cloud/presto-b2/BRIEF.md`). Each
//! item names the API it stands in for; the consolidation replaces them with the real crates' items
//! and implements [`DocumentApi`], [`CompositorApi`] and [`FrameStateApi`] for `presto_core::Document`,
//! `presto_compositor::CompositorHandle` and `presto_frame::UiFrameState`.
//!
//! The three `*Api` traits are also the seams `presto-test` uses: its headless compositor implements
//! [`CompositorApi`], and a test can drive [`crate::App`] with a small document of its own.

use presto_base::accesskit::ActionRequest;
use presto_base::anim::AnimationEvent;
use presto_base::commit::{Commit, FastCommit};
use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Rect, Scale, Size, Vec2};
use presto_base::ids::{FrameId, PropNodeId, Revision, WindowId};
use presto_base::input::InputPurpose;
use presto_base::time::{Clock, Time};
use std::any::Any;
use std::sync::Arc;

// ---- presto-platform (lane PLATFORM): `AppHandler`, `PlatformCx` and their types -----------------

/// Stands in for `presto_platform::WindowDesc`.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Inner size in points.
    pub size: Size,
    pub resizable: bool,
    pub visible: bool,
}

impl Default for WindowDesc {
    fn default() -> Self {
        Self { title: String::new(), size: Size::new(800.0, 600.0), resizable: true, visible: true }
    }
}

/// Stands in for `presto_platform::SurfaceTarget` (the raw window and display handles wgpu needs).
/// Opaque here: only the compositor adapter looks inside.
pub struct SurfaceTarget(pub Box<dyn Any>);

/// Stands in for `presto_platform::ImeState`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeState {
    pub area: Rect,
    pub purpose: InputPurpose,
}

/// Stands in for `presto_platform::Clipboard`.
pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
}

/// Stands in for `presto_platform::Waker`: wakes the UI thread from any thread (`AppHandler::wake`).
#[derive(Clone)]
pub struct Waker(pub Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn wake(&self) {
        (self.0)()
    }
}

/// Stands in for `presto_platform::PlatformError`.
#[derive(Debug)]
pub struct PlatformError(pub String);

/// Stands in for `presto_platform::PlatformCx`: the subset of it `presto` calls.
pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn exit(&mut self);
}

/// Stands in for `presto_platform::AppHandler`.
pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
}

// ---- presto-core (lane CORE): `Document` ------------------------------------------------------------

/// Stands in for `presto_core::Dispatched`: what dispatching one input did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dispatched {
    /// The tree changed (native defaults or listeners): the input needs a frame.
    pub dirty: bool,
    /// JS-facing events to deliver after the native defaults; counted, delivered by the script host.
    pub js_events: usize,
    /// Native-owned writes the compositor applies without a UI frame (architecture §7.3).
    pub fast: Vec<FastCommit>,
}

/// What `presto::App` needs from `presto_core::Document`. Methods past `dispatch` and `frame` are APIs the
/// brief's `Document` lacks; the record lists them as requests to CORE.
pub trait DocumentApi {
    /// Hit test, native defaults, and the JS-facing events (brief: `Document::dispatch`).
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched;
    /// Style, layout, record, a11y; the commit for `window`, or `None` when nothing changed (brief:
    /// `Document::frame`).
    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit>;
    /// The window's tree has changes no commit carries yet (missing in the brief).
    fn needs_frame(&self, window: WindowId) -> bool;
    /// A window's root got a new size (points) and scale (missing in the brief).
    fn resize(&mut self, window: WindowId, size: Size, scale: Scale);
    /// The compositor moved a scroll offset: take it as native state (missing in the brief).
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2);
    /// An animation started, finished or was aborted on the compositor (missing in the brief).
    fn animation_event(&mut self, window: WindowId, ev: AnimationEvent);
    /// An AccessKit action from an assistive technology or a test (missing in the brief). Returns
    /// whether a node handled it.
    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool;
    /// Focus, move and theme changes of a window (missing in the brief).
    fn window_event(&mut self, window: WindowId, ev: &WindowEvent);
    /// The GPU device was reset: the next commit re-sends every resource (missing in the brief).
    fn device_reset(&mut self);
}

/// The JS side of dispatch (svelters-presto's QuickJS host): delivers the JS-facing events and drains
/// microtasks (architecture §11.1, steps 3–4). A pure-Rust app has none.
pub trait ScriptHost {
    /// Deliver and drain; returns whether the tree changed.
    fn deliver(&mut self, doc: &mut dyn DocumentApi, ev: &InputEnvelope, dispatched: &Dispatched) -> bool;
}

// ---- presto-compositor (lane COMPOSITOR): `CompositorHandle` -------------------------------------

/// What `presto::App` needs from `presto_compositor::CompositorHandle`.
pub trait CompositorApi {
    /// Create the window's surface on this (the main) thread and hand it to the compositor (brief:
    /// `instance().create_surface(target)` then `add_window`).
    fn add_window(&self, window: WindowId, target: SurfaceTarget, size: DeviceSize, scale: Scale) -> Result<(), String>;
    fn remove_window(&self, window: WindowId);
    fn commit(&self, c: Commit);
    fn fast(&self, f: FastCommit);
    /// Replies `ToUi::ResizeDone { seq }`.
    fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64);
    fn set_visible(&self, window: WindowId, visible: bool);
    /// The UI has changes for `window` and wants a `BeginFrame` (missing in the brief: the
    /// `NeedsUiFrame` level of `scheduler-latency.md` §2.4).
    fn request_frame(&self, window: WindowId);
    fn shutdown(&self);
}

// ---- presto-frame (lane FRAME-ANIM): `UiFrameState` ----------------------------------------------

/// Stands in for the answer of `UiFrameState::on_urgent_input`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    /// Run the UI phases now, for a frame presenting at `target`.
    RunNow { target: Time },
    /// Leave it to the next `BeginFrame`.
    Wait,
}

/// What `presto::App` needs from `presto_frame::UiFrameState` (one per window).
pub trait FrameStateApi {
    /// A `BeginFrame` arrived; `false` = a commit is still unapplied, so don't start (back-pressure).
    fn on_begin_frame(&mut self, now: Time, frame: FrameId, target: Time, deadline: Time) -> bool;
    fn on_urgent_input(&mut self, now: Time) -> Urgency;
    fn commit_sent(&mut self, now: Time, revision: Revision);
    fn commit_applied(&mut self, now: Time, revision: Revision);
    /// A frame ran and produced no commit.
    fn frame_skipped(&mut self, now: Time);
    /// Presentation feedback (for the urgent rule's estimate of the next vblank).
    fn presented(&mut self, frame: FrameId, at: Time);
}

/// The UI-thread state of `scheduler-latency.md` §2.4 without the estimator: Idle → Running →
/// Committed → Idle, never starting while a commit is unapplied. Urgent input runs at once whenever no
/// commit is in flight, targeting one interval after the last presentation (or now, after idle).
/// `presto_frame::UiFrameState` replaces it with the `now + est_ui_input ≤ D_ui(T)` rule.
pub struct ShimUiFrameState {
    interval: presto_base::time::Span,
    in_flight: Option<Revision>,
    last_present: Option<Time>,
}

impl ShimUiFrameState {
    pub fn new(interval: presto_base::time::Span) -> Self {
        Self { interval, in_flight: None, last_present: None }
    }
}

impl FrameStateApi for ShimUiFrameState {
    fn on_begin_frame(&mut self, _now: Time, _frame: FrameId, _target: Time, _deadline: Time) -> bool {
        self.in_flight.is_none()
    }
    fn on_urgent_input(&mut self, now: Time) -> Urgency {
        if self.in_flight.is_some() {
            return Urgency::Wait;
        }
        let target = match self.last_present {
            // Idle for two intervals or more: the next latchable vblank is as soon as we commit.
            Some(p) if now - p < self.interval + self.interval => {
                let mut t = p + self.interval;
                while t < now {
                    t = t + self.interval;
                }
                t
            }
            _ => now,
        };
        Urgency::RunNow { target }
    }
    fn commit_sent(&mut self, _now: Time, revision: Revision) {
        self.in_flight = Some(revision);
    }
    fn commit_applied(&mut self, _now: Time, revision: Revision) {
        if self.in_flight.is_some_and(|r| r <= revision) {
            self.in_flight = None;
        }
    }
    fn frame_skipped(&mut self, _now: Time) {}
    fn presented(&mut self, _frame: FrameId, at: Time) {
        self.last_present = Some(at);
    }
}
