//! The platform interface every backend implements: the app's [`AppHandler`] callbacks and the
//! [`PlatformCx`] it calls back into, plus the plain data both speak. Nothing here names winit or an
//! OS type, so own backends (platform.md §5) slot in behind the same interface.

use crate::clipboard::Clipboard;
use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceRect, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{SurfaceId, WindowId};
use presto_base::input::InputPurpose;
use presto_base::time::{Clock, Time};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::c_void;
use std::sync::Arc;

/// The app's side of the event loop. Every callback runs on the main thread.
pub trait AppHandler: 'static {
    /// The loop is running: create the first windows here.
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    /// A [`Waker::wake`] from another thread (compositor, jobs). Wakes coalesce: many calls before the
    /// loop gets to it arrive as one.
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    /// The deadline set with [`PlatformCx::set_timer`] passed.
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
    /// Raw relative mouse motion (device units), only while [`PlatformCx::set_mouse_capture`] is on.
    fn raw_mouse_motion(&mut self, _cx: &mut dyn PlatformCx, _window: WindowId, _delta: Vec2) {}
    /// An assistive technology attached, asked for an action, or left (see [`PlatformCx::update_a11y`]).
    fn a11y_event(&mut self, _cx: &mut dyn PlatformCx, _window: WindowId, _ev: A11yEvent) {}
    /// The user asked to quit the app (macOS Cmd-Q, the Dock's Quit). Return `false` to veto.
    fn quit_requested(&mut self, _cx: &mut dyn PlatformCx) -> bool {
        true
    }
    /// The loop is about to exit.
    fn exiting(&mut self, _cx: &mut dyn PlatformCx) {}
}

/// What the app asks of the platform. Main thread only.
pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    /// The raw handles wgpu creates the window's surface from (create it on the main thread). On
    /// Windows with DirectComposition on, it is the window's content visual.
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    /// An OS layer above the window's content (DComp child visual, CAMetalLayer sublayer,
    /// `wl_subsurface`), for viewports that present on their own. `None` where the backend has none (X11).
    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)>;
    /// Moves or resizes an OS layer (window points).
    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect);
    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId);
    /// A `Send` handle that makes OS-layer configuration visible (DirectComposition's `Commit`), for
    /// the compositor thread to call after configuring a layer's surface. `None` when not needed.
    fn os_layer_committer(&self, id: WindowId) -> Option<OsLayerCommitter>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    /// Text input on (with where the candidate window goes) or off.
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    /// Ends the active composition: commits it (`commit`) or cancels it. Returns `false` when this
    /// backend cannot do that (winit has no API for it outside the Windows shim).
    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool;
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    /// Wake [`AppHandler::timer`] at `at` (replaces the previous deadline; `None` clears it).
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    /// Regions of a custom title bar (window points); later regions win. Windows answers
    /// `WM_NCHITTEST` from them (snap layouts); elsewhere a caption press starts a window drag.
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    /// Locks and hides the pointer and turns raw mouse input on (a 3D view's mouse look), or back off.
    fn set_mouse_capture(&mut self, id: WindowId, capture: bool);
    fn set_title(&mut self, id: WindowId, title: &str);
    fn set_visible(&mut self, id: WindowId, visible: bool);
    /// Starts an OS window move from the current pointer press.
    fn drag_window(&mut self, id: WindowId);
    /// Feeds the window's accessibility tree (a `Commit.a11y` update) to the OS adapter.
    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate);
    fn window_scale(&self, id: WindowId) -> Option<Scale>;
    /// What the compositor paces this window's frames by (`Send + Sync`, for the compositor
    /// thread). `None` where the platform has none (headless).
    fn frame_source(&mut self, id: WindowId) -> Option<Arc<dyn crate::frame::FrameSource>>;
    fn exit(&mut self);
}

/// How a window starts.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Inner size in points.
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    /// Outer position in device pixels; `None` lets the OS place it.
    pub position: Option<(i32, i32)>,
    pub decorations: bool,
    pub transparent: bool,
    pub resizable: bool,
    pub visible: bool,
    pub maximized: bool,
    /// The app id (Wayland) / WM_CLASS (X11).
    pub app_id: Option<String>,
    /// Present through the OS compositor tree: on Windows the window gets no redirection bitmap and
    /// its content is a DirectComposition visual (wgpu DX12 only), which `create_os_layer` needs.
    /// Off: a plain window swap chain and no OS layers. Ignored elsewhere.
    pub os_compositing: bool,
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
        }
    }
}

impl WindowDesc {
    pub fn new(title: impl Into<String>, size: Size) -> Self {
        WindowDesc { title: title.into(), size, ..Default::default() }
    }
}

/// Where wgpu presents, as raw OS handles. Valid while the window (or layer) lives; use on the
/// main thread.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum SurfaceTarget {
    /// A window (or Wayland subsurface): wgpu's `SurfaceTargetUnsafe::RawHandle`.
    Raw { window: RawWindowHandle, display: RawDisplayHandle },
    /// An `IDCompositionVisual` (Windows): wgpu's `SurfaceTargetUnsafe::CompositionVisual`.
    CompositionVisual(*mut c_void),
    /// A `CAMetalLayer` (macOS): wgpu's `SurfaceTargetUnsafe::CoreAnimationLayer`.
    CoreAnimationLayer(*mut c_void),
    /// The window is gone.
    None,
}

/// An OS layer request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OsLayerDesc {
    /// Window points.
    pub bounds: Rect,
    /// The layer's pixels carry alpha that blends with what is below.
    pub transparent: bool,
}

impl OsLayerDesc {
    pub fn new(bounds: Rect) -> Self {
        OsLayerDesc { bounds, transparent: false }
    }
}

/// Makes OS-layer changes visible from any thread (DirectComposition `Commit`, through the main
/// thread).
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
    /// Per-character rectangles of the composition (window points), for per-character candidate
    /// placement (Windows `IMR_QUERYCHARPOSITION`). Empty: `area` stands in.
    pub composition_rects: Vec<Rect>,
    /// Text around the caret and the selection in it (UTF-16 offsets into that text), for IMEs that
    /// read context (Windows `IMR_DOCUMENTFEED`).
    pub surrounding: Option<(String, std::ops::Range<usize>)>,
}

impl ImeState {
    pub fn new(area: Rect, purpose: InputPurpose) -> Self {
        ImeState { area, purpose, ..Default::default() }
    }
}

/// A part of a custom title bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitKind {
    /// Ours: input goes to the app.
    Client,
    /// Drags the window, double-click maximises.
    Caption,
    Minimize,
    /// Windows 11 shows snap layouts over it.
    Maximize,
    Close,
    SystemMenu,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRegion {
    /// Window points.
    pub rect: Rect,
    pub kind: HitKind,
}

/// Which kind a point falls in: the last region containing it, else `Client`.
pub fn hit_test(regions: &[HitRegion], p: Point) -> HitKind {
    regions.iter().rev().find(|r| r.rect.contains(p)).map_or(HitKind::Client, |r| r.kind)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: Option<String>,
    /// Top-left in the desktop's device pixels.
    pub position: (i32, i32),
    pub size: presto_base::geom::DeviceSize,
    /// The usable area without task bars and docks (device pixels), where the OS tells.
    pub work_area: Option<DeviceRect>,
    pub scale: Scale,
    pub refresh_millihertz: Option<u32>,
}

/// What an assistive technology did.
#[derive(Clone, Debug, PartialEq)]
pub enum A11yEvent {
    /// An AT attached: send the full tree with [`PlatformCx::update_a11y`].
    Activated,
    Action(accesskit::ActionRequest),
    Deactivated,
}

/// Wakes the main thread from any thread: [`AppHandler::wake`] runs once per batch of wakes.
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
    /// The OS event loop could not start (no display, already running).
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
