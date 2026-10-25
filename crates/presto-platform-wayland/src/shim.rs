//! The presto-platform API this backend implements, mirrored from lane PLATFORM's
//! `crates/presto-platform/src/api.rs` and `clipboard.rs` (in `ci/presto-base` at `ac7cc8c4`).
//!
//! presto-platform will depend on this crate (it selects the backend with a cargo feature), so this crate
//! cannot depend on presto-platform; the consolidation moves the API into a crate both depend on and
//! replaces this module with `use` of it. Items PLATFORM's API does not have, which this backend needs, are
//! marked "Addition" and listed in `plans/milestones/PRESTO-wayland.md`.

use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceRect, DeviceSize, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{FrameId, SurfaceId, WindowId};
use presto_base::input::{InputPurpose, TextInputClient};
use presto_base::time::{Clock, Span, Time};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use smallvec::SmallVec;
use std::ffi::c_void;
use std::sync::Arc;

pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    /// The deadline set with [`PlatformCx::set_timer`] passed.
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
    /// Raw relative mouse motion (device units), only while [`PlatformCx::set_mouse_capture`] is on.
    fn raw_mouse_motion(&mut self, _cx: &mut dyn PlatformCx, _window: WindowId, _delta: Vec2) {}
    /// An assistive technology attached, asked for an action, or left (see [`PlatformCx::update_a11y`]).
    fn a11y_event(&mut self, _cx: &mut dyn PlatformCx, _window: WindowId, _ev: A11yEvent) {}
    fn quit_requested(&mut self, _cx: &mut dyn PlatformCx) -> bool {
        true
    }
    fn exiting(&mut self, _cx: &mut dyn PlatformCx) {}
    /// Addition: a drag this app started ended (tab tear-off, DnD source).
    fn drag_ended(&mut self, _cx: &mut dyn PlatformCx, _drag: DragId, _outcome: DragOutcome) {}
}

pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    /// The raw handles wgpu creates the window's surface from (create it on the main thread).
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    /// An OS layer above the window's content (here a `wl_subsurface`).
    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)>;
    /// Moves or resizes an OS layer (window points).
    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect);
    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId);
    /// A `Send` handle that makes OS-layer configuration visible. `None` when not needed.
    fn os_layer_committer(&self, id: WindowId) -> Option<OsLayerCommitter>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    /// Text input on (with where the candidate window goes) or off.
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    /// Ends the active composition: commits it (`commit`) or cancels it. Returns `false` when this backend
    /// cannot do that.
    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool;
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    fn set_mouse_capture(&mut self, id: WindowId, capture: bool);
    fn set_title(&mut self, id: WindowId, title: &str);
    fn set_visible(&mut self, id: WindowId, visible: bool);
    /// Starts an OS window move with the pointer button that is down.
    fn drag_window(&mut self, id: WindowId);
    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate);
    fn window_scale(&self, id: WindowId) -> Option<Scale>;
    fn exit(&mut self);

    /// Addition: the window's (or one of its OS layers') frame source, handed to the compositor thread.
    fn frame_source(&mut self, _id: WindowId, _layer: Option<SurfaceId>) -> Option<Box<dyn FrameSource>> {
        None
    }
    /// Addition: whether the app must draw the window's title bar and frame (no server-side decorations).
    fn client_side_decorations(&self, _id: WindowId) -> bool {
        false
    }
    /// Addition: window state requests.
    fn set_window_state(&mut self, _id: WindowId, _req: WindowStateRequest) {}
    /// Addition: start dragging `data` from `window` (the pointer button must be held).
    fn start_drag(&mut self, _window: WindowId, _data: DragData) -> Result<DragId, PlatformError> {
        Err(PlatformError::Os("start_drag is not supported".into()))
    }
    /// Addition: tab tear-off. Attach `window` to the running `drag`, with the pointer at `offset` inside it
    /// (window points); the OS then moves the window with the pointer until the drag ends.
    fn attach_window_to_drag(&mut self, _drag: DragId, _window: WindowId, _offset: Vec2) -> Result<(), PlatformError> {
        Err(PlatformError::Os("attach_window_to_drag is not supported".into()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Inner size in points.
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    /// Outer position in device pixels; `None` lets the OS place it (always, on Wayland).
    pub position: Option<(i32, i32)>,
    pub decorations: bool,
    pub transparent: bool,
    pub resizable: bool,
    pub visible: bool,
    pub maximized: bool,
    /// The app id (Wayland) / WM_CLASS (X11).
    pub app_id: Option<String>,
    /// Present through the OS compositor tree (Windows DirectComposition). Ignored here.
    pub os_compositing: bool,
    /// Addition: a popup (menu, tooltip, combo list) positioned against its parent.
    pub popup: Option<PopupDesc>,
    /// Addition: a transient child (dialog) of this window.
    pub parent: Option<WindowId>,
}

impl Default for WindowDesc {
    fn default() -> Self {
        WindowDesc {
            title: String::new(),
            size: Size::new(800.0, 600.0),
            min_size: None,
            max_size: None,
            position: None,
            decorations: true,
            transparent: false,
            resizable: true,
            visible: true,
            maximized: false,
            app_id: None,
            os_compositing: true,
            popup: None,
            parent: None,
        }
    }
}

impl WindowDesc {
    pub fn new(title: impl Into<String>, size: Size) -> Self {
        WindowDesc { title: title.into(), size, ..Default::default() }
    }
}

/// Addition: a popup's placement: `anchor` is a rect in the parent's window points; the popup opens on
/// `gravity`'s side of it and the OS may flip or slide it to stay on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupDesc {
    pub parent: WindowId,
    pub anchor: Rect,
    pub gravity: PopupGravity,
    /// Take an explicit grab (menus): the popup closes when the user clicks outside it.
    pub grab: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PopupGravity {
    #[default]
    Below,
    Above,
    Right,
    Left,
}

/// Addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowStateRequest {
    Maximize,
    Unmaximize,
    Minimize,
    Fullscreen,
    Unfullscreen,
    /// Ask the OS to raise and focus the window (Wayland: `xdg_activation_v1`).
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum SurfaceTarget {
    /// A window (or Wayland subsurface): wgpu's `SurfaceTargetUnsafe::RawHandle`.
    Raw { window: RawWindowHandle, display: RawDisplayHandle },
    /// An `IDCompositionVisual` (Windows).
    CompositionVisual(*mut c_void),
    /// A `CAMetalLayer` (macOS).
    CoreAnimationLayer(*mut c_void),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OsLayerDesc {
    pub bounds: Rect,
    /// The layer's pixels carry alpha that blends with what is below.
    pub transparent: bool,
}

impl OsLayerDesc {
    pub fn new(bounds: Rect) -> Self {
        OsLayerDesc { bounds, transparent: false }
    }
}

/// Makes OS-layer changes visible from any thread.
#[derive(Clone)]
pub struct OsLayerCommitter(pub(crate) Arc<dyn Fn() + Send + Sync>);

impl OsLayerCommitter {
    pub fn commit(&self) {
        (self.0)()
    }
}

impl std::fmt::Debug for OsLayerCommitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OsLayerCommitter")
    }
}

/// Text input state for one window: where the candidate window goes and what the field holds.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ImeState {
    /// The caret or composition rectangle (window points).
    pub area: Rect,
    pub purpose: InputPurpose,
    /// Per-character rectangles of the composition (window points). Empty: `area` stands in.
    pub composition_rects: Vec<Rect>,
    /// Text around the caret and the selection in it (UTF-16 offsets into that text).
    pub surrounding: Option<(String, std::ops::Range<usize>)>,
    /// Addition: the editor's input hints.
    pub hints: ImeHints,
}

impl ImeState {
    /// text-input-v3 caps surrounding text at 4000 bytes.
    pub const SURROUNDING_MAX: usize = 4000;

    pub fn new(area: Rect, purpose: InputPurpose) -> Self {
        ImeState { area, purpose, ..Default::default() }
    }

    /// Addition: the state for `client` with the caret or clause at `area` (its surrounding text converted from
    /// the client's UTF-8 offsets to UTF-16).
    pub fn from_client(client: &dyn TextInputClient, area: Rect) -> Self {
        let surrounding = client.surrounding_text(Self::SURROUNDING_MAX).map(|(text, sel)| {
            let u16_at = |b: usize| text.get(..b.min(text.len())).map_or(0, |s| s.encode_utf16().count());
            let range = u16_at(sel.start)..u16_at(sel.end);
            (text, range)
        });
        ImeState { area, purpose: client.purpose(), surrounding, ..Default::default() }
    }
}

/// Addition: text-input hints beyond the purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ImeHints {
    pub multiline: bool,
    /// Turn off completion, spell checking and auto-capitalisation (code, identifiers).
    pub no_assist: bool,
    pub sensitive: bool,
}

/// A part of a custom title bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitKind {
    /// Ours: input goes to the app.
    Client,
    /// Drags the window, double-click maximises.
    Caption,
    Minimize,
    Maximize,
    Close,
    SystemMenu,
    /// Addition: resize edges and corners (Wayland CSD draws its own frame).
    ResizeTop,
    ResizeBottom,
    ResizeLeft,
    ResizeRight,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRegion {
    pub rect: Rect,
    pub kind: HitKind,
}

/// The topmost region under `p` (later regions are on top); `Client` where none is.
pub fn hit_test(regions: &[HitRegion], p: Point) -> HitKind {
    regions.iter().rev().find(|r| r.rect.contains(p)).map_or(HitKind::Client, |r| r.kind)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: DeviceSize,
    pub work_area: Option<DeviceRect>,
    pub scale: Scale,
    pub refresh_millihertz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum A11yEvent {
    Activated,
    Action(accesskit::ActionRequest),
    Deactivated,
}

#[derive(Clone)]
pub struct Waker(pub(crate) Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Waker(Arc::new(f))
    }
    pub fn wake(&self) {
        (self.0)()
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waker")
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PlatformError {
    EventLoop(String),
    Os(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::EventLoop(s) => write!(f, "event loop: {s}"),
            PlatformError::Os(s) => write!(f, "os: {s}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// The two selections; `Primary` exists only on Linux desktops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipboardKind {
    Clipboard,
    Primary,
}

pub trait Clipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String>;
    /// `false` when the selection does not exist here or the OS refused.
    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool;
    fn has_primary(&self) -> bool {
        false
    }
}

/// Addition: a drag's payload: MIME type → bytes. Within the app the payload is usually one id under
/// presto's own MIME type.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DragData {
    pub items: Vec<(String, Vec<u8>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragOutcome {
    /// Dropped on a target that accepted it.
    Dropped,
    Cancelled,
}

/// Addition: the per-surface frame signal source the compositor thread waits on (`platform.md` §4: frame
/// callbacks and presentation feedback on Wayland, the DXGI waitable or compositor clock on Windows, the
/// display link on macOS). `Send`: created on the main thread, used on the compositor thread.
pub trait FrameSource: Send {
    /// Ask for (or stop asking for) vsync signals.
    fn subscribe(&mut self, on: bool);
    /// Call right before presenting `frame` at `size` (device pixels); `target` is the intended display time.
    fn before_present(&mut self, frame: FrameId, target: Option<Time>, size: DeviceSize);
    /// Call right after the present call returns.
    fn after_present(&mut self, frame: FrameId);
    /// Collect signals that arrived, without blocking.
    fn poll(&mut self, now: Time) -> SmallVec<[FrameSignal; 4]>;
    /// Block until a signal may be ready or `until` passes.
    fn wait(&mut self, until: Option<Time>);
}

/// What a [`FrameSource`] reports; maps one to one onto presto-frame's `Signal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameSignal {
    Vsync { target: Time, interval: Span },
    Presented { frame: FrameId, at: Time, estimated: bool },
    /// The OS stopped (or resumed) asking for frames: hidden, minimised, fully covered.
    Visible(bool),
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::input::Utf16Range;

    struct Field(&'static str, std::ops::Range<usize>);

    impl TextInputClient for Field {
        fn text_for_range(&self, _: Utf16Range) -> Option<String> {
            None
        }
        fn selected_range(&self) -> Option<Utf16Range> {
            None
        }
        fn marked_range(&self) -> Option<Utf16Range> {
            None
        }
        fn replace_text_in_range(&mut self, _: Option<Utf16Range>, _: &str) {}
        fn replace_and_mark_text_in_range(&mut self, _: Option<Utf16Range>, _: &str, _: Option<Utf16Range>) {}
        fn unmark_text(&mut self) {}
        fn bounds_for_range(&self, _: Utf16Range) -> Option<Rect> {
            None
        }
        fn character_index_for_point(&self, _: Point) -> Option<usize> {
            None
        }
        fn surrounding_text(&self, _: usize) -> Option<(String, std::ops::Range<usize>)> {
            Some((self.0.to_owned(), self.1.clone()))
        }
        fn purpose(&self) -> InputPurpose {
            InputPurpose::Code
        }
    }

    #[test]
    fn from_client_converts_byte_offsets_to_utf16() {
        // "é😀x": é is 2 bytes / 1 unit, 😀 4 bytes / 2 units.
        let s = ImeState::from_client(&Field("é😀x", 2..6), Rect::default());
        assert_eq!(s.surrounding, Some(("é😀x".to_owned(), 1..3)));
        assert_eq!(s.purpose, InputPurpose::Code);
    }

    #[test]
    fn hit_test_defaults_to_client() {
        let regions = [HitRegion { rect: Rect::new(0.0, 0.0, 10.0, 10.0), kind: HitKind::Caption }];
        assert_eq!(hit_test(&regions, Point::new(5.0, 5.0)), HitKind::Caption);
        assert_eq!(hit_test(&regions, Point::new(50.0, 5.0)), HitKind::Client);
    }
}
