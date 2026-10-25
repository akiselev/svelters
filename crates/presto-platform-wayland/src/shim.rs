//! The presto-platform API this backend implements, as the b1 brief writes it
//! (`plans/cloud/presto-b1/BRIEF.md`, "presto-platform (lane PLATFORM)").
//!
//! Lane PLATFORM defines these in `presto-platform`; until its skeleton is in `ci/presto-base` this module
//! carries the brief's signatures so the backend compiles against them. The consolidation replaces the
//! module with `use presto_platform::*`. Items the brief does not name, which this backend needs, are marked
//! "Addition" and listed in `plans/milestones/PRESTO-wayland.md` for the consolidation to add to the trait.

use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{FrameId, SurfaceId, WindowId};
use presto_base::input::{InputPurpose, TextInputClient};
use presto_base::time::{Clock, Span, Time};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use smallvec::SmallVec;
use std::ops::Range;
use std::sync::Arc;

pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    /// Cross-thread wake (compositor, jobs).
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    /// The deadline set with [`PlatformCx::set_timer`] passed.
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
    /// Addition: a drag this app started ended (tab tear-off, DnD source).
    fn drag_ended(&mut self, _cx: &mut dyn PlatformCx, _drag: DragId, _outcome: DragOutcome) {}
}

pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    /// Raw handles for wgpu; create the surface on the main thread.
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    /// `Send + Clone`; `wake()` from any thread.
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    fn exit(&mut self);

    /// Addition: move an OS layer within its window (window points) or change its size.
    fn set_os_layer_rect(&mut self, _id: WindowId, _layer: SurfaceId, _rect: Rect) {}
    /// Addition: remove an OS layer.
    fn remove_os_layer(&mut self, _id: WindowId, _layer: SurfaceId) {}
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
        Err(PlatformError::Unsupported("start_drag"))
    }
    /// Addition: tab tear-off. Attach `window` to the running `drag`, with the pointer at `offset` inside it
    /// (window points); the OS then moves the window with the pointer until the drag ends.
    fn attach_window_to_drag(&mut self, _drag: DragId, _window: WindowId, _offset: Vec2) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("attach_window_to_drag"))
    }
}

/// Runs the platform's event loop until [`PlatformCx::exit`].
pub type RunFn = fn(Box<dyn AppHandler>) -> Result<(), PlatformError>;

#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Content size in points.
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    /// Ask for OS decorations (title bar, frame). Where the OS has none the app draws them
    /// ([`PlatformCx::client_side_decorations`]).
    pub decorations: bool,
    pub transparent: bool,
    pub resizable: bool,
    /// Addition: the application id (Wayland `app_id`, the desktop file name).
    pub app_id: Option<String>,
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
            decorations: true,
            transparent: false,
            resizable: true,
            app_id: None,
            popup: None,
            parent: None,
        }
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

/// An OS layer: a child surface composited by the OS above the window's content (viewports, canvases).
#[derive(Clone, Debug, PartialEq)]
pub struct OsLayerDesc {
    /// Position and size in window points.
    pub rect: Rect,
    pub opaque: bool,
}

/// Raw handles for `wgpu::SurfaceTargetUnsafe::RawHandle`. The handles point at OS objects owned by the
/// platform, valid until the window (or layer) is closed.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceTarget {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

/// The text-input state the platform's IME bridge reports to the OS for the focused editor.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ImeState {
    /// The caret or the active clause, in window points (candidate window placement).
    pub area: Rect,
    pub purpose: InputPurpose,
    /// Addition: text around the caret (at most [`ImeState::SURROUNDING_MAX`] UTF-8 bytes) and the selection
    /// in it (byte offsets), for reconversion and predictive input.
    pub surrounding: Option<(String, Range<usize>)>,
    /// Addition: the editor's input hints.
    pub hints: ImeHints,
}

impl ImeState {
    /// text-input-v3 caps a request at 4000 bytes.
    pub const SURROUNDING_MAX: usize = 4000;

    /// The state for `client` with the caret or clause at `area`.
    pub fn from_client(client: &dyn TextInputClient, area: Rect) -> Self {
        ImeState {
            area,
            purpose: client.purpose(),
            surrounding: client.surrounding_text(Self::SURROUNDING_MAX),
            hints: ImeHints::default(),
        }
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

pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: String) -> Result<(), PlatformError>;
    /// Addition: the primary selection (X11 and Wayland middle-click paste).
    fn get_primary(&mut self) -> Option<String> {
        None
    }
    /// Addition.
    fn set_primary(&mut self, _text: String) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("primary selection"))
    }
}

/// Wakes the platform's event loop from any thread; the loop then calls [`AppHandler::wake`].
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

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

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: Option<String>,
    /// Position in the global (logical) desktop space, where the OS reports one.
    pub position: Point,
    /// Size in device pixels (the current mode).
    pub size: DeviceSize,
    pub scale: Scale,
    pub refresh_millihertz: Option<u32>,
}

/// A region of a window with OS meaning (custom title bars).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRegion {
    pub rect: Rect,
    pub kind: HitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// Drag moves the window; double-click maximises; secondary click opens the window menu.
    Caption,
    Minimize,
    Maximize,
    Close,
    ResizeTop,
    ResizeBottom,
    ResizeLeft,
    ResizeRight,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
    /// Ordinary content inside an otherwise special area (a button in the title bar).
    Client,
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

#[derive(Debug)]
pub enum PlatformError {
    /// No display server (Wayland: `WAYLAND_DISPLAY` unset or the connection failed).
    NoDisplay(String),
    /// A required protocol is missing (`wl_compositor`, `xdg_wm_base`).
    MissingProtocol(&'static str),
    /// The connection broke.
    Connection(String),
    Unsupported(&'static str),
    Io(std::io::Error),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::NoDisplay(e) => write!(f, "no display: {e}"),
            PlatformError::MissingProtocol(p) => write!(f, "the compositor lacks {p}"),
            PlatformError::Connection(e) => write!(f, "display connection: {e}"),
            PlatformError::Unsupported(what) => write!(f, "unsupported: {what}"),
            PlatformError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<std::io::Error> for PlatformError {
    fn from(e: std::io::Error) -> Self {
        PlatformError::Io(e)
    }
}
