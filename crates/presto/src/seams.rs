//! The seams [`crate::App`] drives: the tree ([`DocumentApi`]), the compositor ([`CompositorApi`]) and
//! the JS side of dispatch ([`ScriptHost`]). [`crate::backends`] implements them for
//! `presto_core::Document` and `presto_compositor::CompositorHandle`; `presto-test` implements
//! [`CompositorApi`] with its GPU-less compositor and tests can drive the app with a small document.

use presto_base::accesskit::ActionRequest;
use presto_base::anim::AnimationEvent;
use presto_base::commit::{Commit, FastCommit};
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Scale, Size, Vec2};
use presto_base::ids::{FrameId, PropNodeId, WindowId};
use presto_base::time::{Span, Time};
use presto_platform::SurfaceTarget;

/// What dispatching one input did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dispatched {
    /// The tree changed (native defaults or listeners): the input needs a frame.
    pub dirty: bool,
    /// JS-facing events waiting for the script host.
    pub js_events: usize,
    /// Native-owned writes the compositor applies without a UI frame (architecture §7.3).
    pub fast: Vec<FastCommit>,
}

/// The tree of an app, as the runner needs it.
pub trait DocumentApi {
    /// Hit test and native defaults; the JS-facing events wait for the script host.
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched;
    /// Style, layout, record, a11y; the commit for `window`, or `None` when nothing changed.
    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit>;
    /// The window's tree has changes no commit carries yet.
    fn needs_frame(&self, window: WindowId) -> bool;
    /// A window opened or got a new size (points) and scale.
    fn resize(&mut self, window: WindowId, size: Size, scale: Scale);
    fn close_window(&mut self, window: WindowId);
    /// The compositor moved a scroll offset: take it as native state.
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2);
    /// An animation started, finished or was aborted on the compositor.
    fn animation_event(&mut self, window: WindowId, ev: AnimationEvent);
    /// An AccessKit action (an assistive technology or a test). Returns whether a node handled it.
    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool;
    /// An assistive technology attached: the next commit carries the whole tree.
    fn accessibility_activated(&mut self, window: WindowId);
    /// Focus, move and theme changes of a window.
    fn window_event(&mut self, window: WindowId, ev: &WindowEvent);
    /// The GPU device was reset: the next commit re-sends every resource.
    fn device_reset(&mut self);
}

/// The JS side of dispatch (svelters-presto's QuickJS host): delivers the JS-facing events and drains
/// microtasks (architecture §11.1, steps 3–4). A pure-Rust app has none.
pub trait ScriptHost<D> {
    /// Deliver and drain; returns whether the tree changed.
    fn deliver(&mut self, doc: &mut D, ev: &InputEnvelope, dispatched: &Dispatched) -> bool;
}

/// The compositor, as the runner needs it.
pub trait CompositorApi {
    /// Create the window's surface on this (the main) thread from the platform's raw handles and hand
    /// it to the compositor.
    fn add_window(
        &self,
        window: WindowId,
        target: SurfaceTarget,
        size: DeviceSize,
        scale: Scale,
    ) -> Result<(), String>;
    fn remove_window(&self, window: WindowId);
    fn commit(&self, c: Commit);
    fn fast(&self, f: FastCommit);
    /// Replies `ToUi::ResizeDone { seq }`.
    fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64);
    fn set_visible(&self, window: WindowId, visible: bool);
    /// The UI has changes for `window`: send a `BeginFrame`.
    fn request_ui_frame(&self, window: WindowId);
    /// Continuous input arrived (keeps variable-refresh displays up).
    fn high_rate_input(&self, window: WindowId);
    /// How long the UI phases of `frame` took (the frame clock's estimator).
    fn ui_cost(&self, window: WindowId, frame: FrameId, cost: Span);
    /// The UI answered `BeginFrame` `frame` with nothing to commit (`UiFrameSkipped`).
    fn ui_frame_skipped(&self, window: WindowId, frame: FrameId);
    fn shutdown(&self);
}
