//! The presto-platform contract this backend implements, as `plans/cloud/presto-b1/BRIEF.md` gives it
//! (lane PLATFORM). presto-platform's skeleton is not in `ci/presto-base` yet, so the signatures live
//! here; the consolidation replaces this module with `presto_platform::*`.
//!
//! The brief fixes `AppHandler`, `PlatformCx` and `run`; the data types it names but does not spell
//! out (`WindowDesc`, `SurfaceTarget`, `OsLayerDesc`, `ImeState`, `Clipboard`, `Waker`, `Monitor`,
//! `HitRegion`, `PlatformError`, `FrameSource`) are this backend's proposal. Items marked
//! "addition" are not in the brief; `plans/milestones/PRESTO-win32.md` lists them for PLATFORM.

use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceRect, Rect, Scale, Size};
use presto_base::ids::{SurfaceId, WindowId};
use presto_base::input::{InputPurpose, TextInputClient};
use presto_base::time::{Clock, Span, Time};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    /// A cross-thread wake (compositor, jobs).
    fn wake(&mut self, cx: &mut dyn PlatformCx);
    /// The deadline set with [`PlatformCx::set_timer`] passed.
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);
    /// Addition: the focused text field of `window`, which the IME bridge (TSF, `WM_IME_REQUEST`)
    /// reads and edits synchronously while it handles an OS callback.
    fn text_input(&mut self, window: WindowId) -> Option<&mut dyn TextInputClient> {
        let _ = window;
        None
    }
    /// Addition: a drag started with [`PlatformCx::start_drag`] ended.
    fn drag_finished(&mut self, cx: &mut dyn PlatformCx, result: DragResult) {
        let _ = (cx, result);
    }
}

pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    /// Raw handles for wgpu, main thread.
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;
    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    /// Send + Clone; `wake()` from any thread.
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    fn exit(&mut self);

    /// Addition: move, restack or resize an OS layer (window points).
    fn update_os_layer(&mut self, id: WindowId, layer: SurfaceId, desc: OsLayerDesc);
    /// Addition.
    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId);
    /// Addition: the window's frame source, moved to the compositor thread (`platform.md` §3.1).
    fn frame_source(&mut self, id: WindowId) -> Option<Box<dyn FrameSource>>;
    /// Addition: start an OS drag; it runs after the current handler call returns, with dispatch
    /// continuing during the drag, and ends in [`AppHandler::drag_finished`].
    fn start_drag(&mut self, id: WindowId, data: DragData);
    /// Addition: keep pointer events coming to `id` outside its bounds (tab tear-off).
    fn set_pointer_capture(&mut self, id: WindowId, capture: bool);
    /// Addition.
    fn set_title(&mut self, id: WindowId, title: &str);
    /// Addition.
    fn set_visible(&mut self, id: WindowId, visible: bool);
    /// Addition.
    fn set_window_state(&mut self, id: WindowId, state: WindowState);
    /// Addition: the composition in `id`'s text field is committed or cancelled by the app.
    fn end_composition(&mut self, id: WindowId, commit: bool);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Decorations {
    /// The OS title bar and borders.
    #[default]
    System,
    /// The app draws its title bar; `set_hit_test_regions` says where the caption and buttons are.
    /// The OS keeps the resize borders and shadow.
    Custom,
    /// No title bar and no borders (popups, tooltips, splash).
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowKind {
    #[default]
    Normal,
    /// An owned popup (menu, combo list): no taskbar button, never activated.
    Popup,
    Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowDesc {
    pub title: String,
    /// Content size in points.
    pub size: Size,
    pub min_size: Option<Size>,
    /// Outer position in physical pixels of the virtual screen; `None` = the OS's choice.
    pub position: Option<(i32, i32)>,
    pub resizable: bool,
    pub decorations: Decorations,
    pub kind: WindowKind,
    pub visible: bool,
    /// Per-pixel alpha with the desktop (the surface's alpha mode decides).
    pub transparent: bool,
    /// The owner window (popups, dialogs, tool windows).
    pub parent: Option<WindowId>,
    pub state: WindowState,
}

impl Default for WindowDesc {
    fn default() -> Self {
        Self {
            title: String::new(),
            size: Size::new(800.0, 600.0),
            min_size: None,
            position: None,
            resizable: true,
            decorations: Decorations::System,
            kind: WindowKind::Normal,
            visible: true,
            transparent: false,
            parent: None,
            state: WindowState::Normal,
        }
    }
}

/// What wgpu needs to create a surface, on the main thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceTarget {
    /// No such window.
    None,
    /// A Win32 window (`raw-window-handle`'s `Win32WindowHandle`).
    Win32 { hwnd: isize, hinstance: isize },
    /// An `IDCompositionVisual` for wgpu's `SurfaceTargetUnsafe::CompositionVisual` (DX12). The
    /// pointer stays valid while the window or layer lives; wgpu sets the content, the platform
    /// commits the tree.
    CompositionVisual(*mut std::ffi::c_void),
}

/// An OS compositor layer above or below a window's main surface (viewports, canvases).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OsLayerDesc {
    /// Bounds in window points.
    pub bounds: Rect,
    /// Stacking among the window's layers; the main surface is at 0, negative is below it.
    pub z: i32,
    pub opaque: bool,
    /// Clip to the bounds (the visual's own clip, so a resizing viewport never draws outside).
    pub clip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImeState {
    /// The caret or active clause in window points (candidate window placement before the IME asks
    /// the text field itself).
    pub area: Rect,
    pub purpose: InputPurpose,
}

pub trait Clipboard {
    fn get_text(&mut self) -> Result<Option<String>, PlatformError>;
    fn set_text(&mut self, text: &str) -> Result<(), PlatformError>;
    /// Files on the clipboard (Explorer copy).
    fn get_files(&mut self) -> Result<Vec<PathBuf>, PlatformError>;
}

/// Wakes the platform's event loop from any thread; the app sees [`AppHandler::wake`]. Wakes that
/// arrive before the loop handled the last one coalesce.
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

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Waker")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    /// The OS's device name (`\\.\DISPLAY1`).
    pub name: String,
    /// Bounds in physical pixels of the virtual screen.
    pub rect: DeviceRect,
    /// The work area (without the taskbar), physical pixels.
    pub work_area: DeviceRect,
    pub scale: Scale,
    /// Refresh rate in millihertz, when the OS reports one.
    pub refresh_millihz: Option<u32>,
    pub primary: bool,
}

/// What a region of a custom title bar is, for the OS's hit test (snap layouts, drag, resize).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitKind {
    /// Inside the caption but app content (a tab, a search box): normal client input.
    Client,
    /// Drags the window; double-click maximises.
    Caption,
    /// The window icon's system menu.
    SystemMenu,
    Minimize,
    /// The maximise button: Windows 11 shows the snap-layout flyout over it.
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRegion {
    /// Window points.
    pub rect: Rect,
    pub kind: HitKind,
}

/// What a frame source woke for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameWake {
    /// A frame boundary (vblank or the compositor clock's tick); `estimated` when it came from a
    /// sleep rather than the OS.
    Frame { at: Time, estimated: bool },
    /// The timeout passed first.
    Timeout,
}

/// The compositor thread's frame clock source (`platform.md` §4): it blocks until the OS's next
/// frame boundary.
pub trait FrameSource: Send {
    fn wait(&mut self, timeout: Option<Span>) -> FrameWake;
    /// The current refresh interval.
    fn interval(&self) -> Span;
    /// Ask for the faster clock during gestures and animations (Windows 11's compositor boost).
    fn set_boost(&mut self, on: bool);
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DragData {
    pub text: Option<String>,
    pub files: Vec<PathBuf>,
    /// Allow the target to move instead of copy.
    pub allow_move: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragResult {
    Copied,
    Moved,
    Cancelled,
}

#[derive(Debug)]
pub enum PlatformError {
    /// An OS call failed (`HRESULT` or `GetLastError` code).
    Os { call: &'static str, code: i32 },
    /// The OS or this backend lacks the feature.
    Unsupported(&'static str),
    /// The clipboard is held by another process.
    ClipboardBusy,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Os { call, code } => write!(f, "{call} failed: 0x{code:08X}"),
            PlatformError::Unsupported(what) => write!(f, "unsupported: {what}"),
            PlatformError::ClipboardBusy => f.write_str("the clipboard is held by another process"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl PlatformError {
    pub(crate) fn win(call: &'static str, e: windows_core::Error) -> Self {
        PlatformError::Os { call, code: e.code().0 }
    }
}
