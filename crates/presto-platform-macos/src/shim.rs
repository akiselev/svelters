//! The platform trait from `plans/cloud/presto-b1/BRIEF.md` (lane PLATFORM owns it in
//! `presto-platform`). Until PLATFORM's skeleton is in `ci/presto-base`, this backend codes against the
//! brief's signatures here; the consolidation replaces this module with `presto_platform`'s types.
//!
//! Missing from the brief and defined here (each is a proposal for `presto-platform`): the field lists of
//! [`WindowDesc`], [`OsLayerDesc`], [`ImeState`], [`Monitor`], [`HitRegion`]; [`SurfaceTarget`]'s
//! variants; the [`Clipboard`] and [`FrameSource`] traits; [`Waker`]; [`PlatformError`].

use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{Point, Rect, Scale, Size};
use presto_base::ids::{SurfaceId, WindowId};
use presto_base::input::InputPurpose;
use presto_base::time::{Clock, Span, Time};
use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;
use std::sync::Arc;

/// The application: the platform calls it on the main thread, one call at a time.
pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    /// A cross-thread wake ([`Waker::wake`]); wakes coalesce.
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    /// The deadline set with [`PlatformCx::set_timer`] passed.
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
}

/// What the application can ask of the platform while it handles a call.
pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    /// The window's surface target for wgpu; create the surface on the main thread.
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    fn create_os_layer(
        &mut self,
        id: WindowId,
        desc: OsLayerDesc,
    ) -> Option<(SurfaceId, SurfaceTarget)>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    fn exit(&mut self);
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Content size in points.
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    /// Top-left of the content area in global points (origin at the primary monitor's top-left);
    /// `None` centres the window.
    pub position: Option<Point>,
    pub resizable: bool,
    /// The OS title bar and frame; `false` is a borderless window.
    pub decorations: bool,
    /// Content extends under a transparent title bar (the window controls stay); the app draws its own
    /// title bar and marks it with [`HitRegion`]s.
    pub custom_title_bar: bool,
    pub transparent: bool,
    pub visible: bool,
}

impl Default for WindowDesc {
    fn default() -> Self {
        WindowDesc {
            title: String::new(),
            size: Size::new(800.0, 600.0),
            min_size: None,
            max_size: None,
            position: None,
            resizable: true,
            decorations: true,
            custom_title_bar: false,
            transparent: false,
            visible: true,
        }
    }
}

/// An OS layer (architecture §9.5): a separate GPU surface stacked above the window's content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OsLayerDesc {
    /// Window points, top-left origin.
    pub rect: Rect,
    pub opaque: bool,
}

/// What wgpu needs to create a surface. Raw pointers: use on the main thread only.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceTarget {
    /// A `CAMetalLayer` (wgpu `SurfaceTargetUnsafe::CoreAnimationLayer`).
    CoreAnimationLayer(NonNull<c_void>),
}

/// The text-input state of a window without a full text-input client (winit's model): IME on, the
/// caret area for the candidate window, the field's purpose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeState {
    /// Window points.
    pub area: Rect,
    pub purpose: InputPurpose,
}

pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
}

/// Wakes the platform's main loop from any thread; the app gets [`AppHandler::wake`].
#[derive(Clone)]
pub struct Waker(pub Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn wake(&self) {
        (self.0)()
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Waker")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: String,
    /// Global points, origin at the primary monitor's top-left.
    pub bounds: Rect,
    /// `bounds` minus the menu bar and the Dock.
    pub work_area: Rect,
    pub scale: Scale,
    pub refresh_hz: Option<f32>,
    pub primary: bool,
}

/// A region of a window with OS meaning (custom title bars).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRegion {
    /// Window points, top-left origin.
    pub rect: Rect,
    pub kind: HitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// Drags the window; a double click zooms or minimises, as the user's title-bar setting says.
    Caption,
    /// Ordinary content (punches a hole in a caption region).
    Client,
    Minimize,
    Maximize,
    Close,
}

/// One display refresh, as a frame source reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VsyncTick {
    /// When the source fired.
    pub now: Time,
    /// When the frame being prepared will reach the display.
    pub target: Time,
    pub interval: Span,
}

/// A per-window vsync source the compositor drives (architecture §10). Created on the main thread,
/// then used from the compositor thread.
pub trait FrameSource: Send {
    /// Start or stop ticks; a stopped source costs nothing.
    fn set_active(&self, active: bool);
}

#[derive(Debug)]
pub enum PlatformError {
    /// [`run`](crate::run) was called off the main thread.
    NotMainThread,
    /// The event loop is already running.
    AlreadyRunning,
    /// An OS call failed.
    Os(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::NotMainThread => f.write_str("the platform must run on the main thread"),
            PlatformError::AlreadyRunning => {
                f.write_str("the platform's event loop is already running")
            }
            PlatformError::Os(e) => write!(f, "OS error: {e}"),
        }
    }
}

impl std::error::Error for PlatformError {}
