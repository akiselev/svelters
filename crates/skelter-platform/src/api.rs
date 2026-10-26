//! The platform interface every backend implements: the app's [`AppHandler`] callbacks and the
//! [`PlatformCx`] it calls back into, plus the plain data both speak. Nothing here names winit or an
//! OS type, so own backends (platform.md §5) slot in behind the same interface.

use crate::clipboard::Clipboard;
use skelter_base::accesskit;
use skelter_base::cursor::CursorIcon;
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::{DeviceRect, Point, Rect, Scale, Size, Vec2};
use skelter_base::ids::{SurfaceId, WindowId};
use skelter_base::input::{InputPurpose, TextInputClient};
use skelter_base::time::{Clock, Time};
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
    /// An item of the native menu bar ([`PlatformCx::set_menu_bar`]) was chosen: its [`MenuNode::Item`] id.
    fn menu_activated(&mut self, _cx: &mut dyn PlatformCx, _id: &str) {}
    /// A drag started with [`PlatformCx::start_drag`] ended.
    fn drag_ended(&mut self, _cx: &mut dyn PlatformCx, _drag: DragId, _outcome: DragOutcome) {}
    /// The loop is about to exit.
    fn exiting(&mut self, _cx: &mut dyn PlatformCx) {}
}

/// What the app asks of the platform. Main thread only.
pub trait PlatformCx {
    /// Creates a window; the error says why the OS refused.
    fn try_create_window(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError>;
    /// [`PlatformCx::try_create_window`] for apps that cannot run without the window: panics with
    /// the OS's reason.
    fn create_window(&mut self, desc: WindowDesc) -> WindowId {
        match self.try_create_window(desc) {
            Ok(id) => id,
            Err(e) => panic!("skelter-platform: the OS refused to create a window: {e}"),
        }
    }
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
    /// Starts an OS layer resize (architecture §7.6): holds the layer's state so that the bounds set
    /// before [`PlatformCx::end_os_layer_resize`] and the compositor's first present at the new size
    /// show together (Wayland: the subsurface goes to sync mode). Returns `false` when the backend
    /// holds nothing because its layers never show a buffer stretched (DirectComposition shows a swap
    /// chain at its own size; skelter's CAMetalLayers pin their drawable to the top-left): the caller
    /// then presents the new size first and moves the layer after.
    fn begin_os_layer_resize(&mut self, _id: WindowId, _layer: SurfaceId) -> bool {
        false
    }
    /// Ends the resize [`PlatformCx::begin_os_layer_resize`] started: what it held becomes visible.
    fn end_os_layer_resize(&mut self, _id: WindowId, _layer: SurfaceId) {}
    /// A `Send` handle that makes OS-layer configuration visible (DirectComposition's `Commit`), for
    /// the compositor thread to call after configuring a layer's surface. `None` when not needed.
    fn os_layer_committer(&self, id: WindowId) -> Option<OsLayerCommitter>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    /// Text input on (with where the candidate window goes) or off.
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>);
    /// Gives the window a full text-input client (architecture §11.3): the IME reads and edits the
    /// document through it (reconversion, accents, dictation, per-character candidate placement)
    /// instead of reporting composition as `ImeEvent`s. Key presses the IME passes through still
    /// arrive as key events with their text. `None` turns text input off. Backends without a full
    /// IME bridge fall back to [`PlatformCx::set_ime`] with the client's caret and purpose.
    fn set_text_input(&mut self, id: WindowId, client: Option<Box<dyn TextInputClient>>) {
        let state = client.map(|c| {
            let area = c.selected_range().and_then(|r| c.bounds_for_range(r)).unwrap_or_default();
            ImeState::new(area, c.purpose())
        });
        self.set_ime(id, state);
    }
    /// Sets the app's native menu bar (macOS) from `menu` (`None` removes the app's menus). Returns
    /// `false` where the OS has no native menu bar; the app then draws its own. Chosen items arrive
    /// as [`AppHandler::menu_activated`].
    fn set_menu_bar(&mut self, _menu: Option<&MenuDesc>) -> bool {
        false
    }
    /// Ends the active composition: commits it (`commit`) or cancels it. Returns `false` when this
    /// backend cannot do that (winit has no API for it outside the Windows shim).
    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool;
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    /// Wake [`AppHandler::timer`] at `at` (replaces the previous deadline; `None` clears it).
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    /// The monitor showing most of the window, where the OS tells (`None`: the app falls back to the
    /// window's position). [`WindowEvent::MonitorChanged`] says when the answer may have changed.
    fn monitor_of(&self, _id: WindowId) -> Option<Monitor> {
        None
    }
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
    /// An OS layer's own frame source, where the layer gets frame callbacks of its own (a desync
    /// Wayland subsurface). `None`: the layer is paced by its window's [`PlatformCx::frame_source`].
    fn layer_frame_source(&mut self, _id: WindowId, _layer: SurfaceId) -> Option<Arc<dyn crate::frame::FrameSource>> {
        None
    }
    /// A popup (menu, tooltip, combo list) placed against its parent by [`PopupDesc`]; the OS may flip
    /// or slide it to stay on screen. [`PlatformError::Unsupported`] where the backend has none (the
    /// app then draws the popup inside its window).
    fn create_popup(&mut self, _desc: WindowDesc, _popup: PopupDesc) -> Result<WindowId, PlatformError> {
        Err(PlatformError::Unsupported("popup windows"))
    }
    /// A toplevel that is a transient child (dialog) of `parent`: stays above it, minimises with it.
    fn create_child_window(&mut self, _desc: WindowDesc, _parent: WindowId) -> Result<WindowId, PlatformError> {
        Err(PlatformError::Unsupported("child windows"))
    }
    /// Whether the app must draw the window's title bar and frame itself because the OS draws none
    /// (Wayland without server-side decorations). The platform then resizes from a border along the
    /// window's edges by itself (6 points, 16-point corners; off while maximised, fullscreen, tiled or
    /// not resizable); the app's [`HitRegion`]s cover the title bar.
    fn client_side_decorations(&self, _id: WindowId) -> bool {
        false
    }
    /// Asks the OS to change the window's state. Returns `false` when this backend cannot (the request
    /// was not sent); `true` means it was sent, and the OS reports the result through `Resized`,
    /// `Focused` and the like.
    fn set_window_state(&mut self, _id: WindowId, _req: WindowStateRequest) -> bool {
        false
    }
    /// Input hints for the window's text input beyond [`InputPurpose`] (with `set_ime` or
    /// `set_text_input`). Ignored where the IME takes none.
    fn set_ime_hints(&mut self, _id: WindowId, _hints: ImeHints) {}
    /// Starts dragging `data` out of `window` (a pointer button must be held). The end arrives as
    /// [`AppHandler::drag_ended`]. [`PlatformError::Unsupported`] where the backend has no drag source.
    fn start_drag(&mut self, _window: WindowId, _data: DragData) -> Result<DragId, PlatformError> {
        Err(PlatformError::Unsupported("drag source"))
    }
    /// Tab tear-off: attaches `window` to the running `drag` with the pointer at `offset` inside it
    /// (window points); the OS moves the window with the pointer until the drag ends.
    fn attach_window_to_drag(&mut self, _drag: DragId, _window: WindowId, _offset: Vec2) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("window drags"))
    }
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
    /// Content extends under a transparent title bar (the OS window controls stay; macOS); the app
    /// draws its own title bar and marks it with [`HitRegion`]s. Ignored where unsupported.
    pub custom_title_bar: bool,
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
            custom_title_bar: false,
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
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        OsLayerCommitter(Arc::new(f))
    }

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
    /// Physical pixels per inch, where the OS reports the panel's physical size (EDID); `None` when
    /// it does not or the size is implausible (projectors, TVs reporting an aspect ratio). Apps then
    /// fall back to the OS's logical DPI convention (96 × scale, 72 × scale on macOS).
    pub ppi: Option<f32>,
    pub name: Option<String>,
    /// Top-left in the desktop's device pixels.
    pub position: (i32, i32),
    pub size: skelter_base::geom::DeviceSize,
    /// The usable area without task bars and docks (device pixels), where the OS tells.
    pub work_area: Option<DeviceRect>,
    pub scale: Scale,
    pub refresh_millihertz: Option<u32>,
}

impl Monitor {
    /// Pixels per inch along the diagonal of a panel of `pixels` that measures `mm` (width, height in
    /// millimetres). `None` for a missing size (0) or an implausible one (outside 40..=1000 ppi: a
    /// projector, or an EDID holding the aspect ratio in centimetres).
    pub fn ppi_from_mm(pixels: skelter_base::geom::DeviceSize, mm: (f32, f32)) -> Option<f32> {
        let (wmm, hmm) = mm;
        if !(wmm > 0.0 && hmm > 0.0) || pixels.w == 0 || pixels.h == 0 {
            return None;
        }
        let px = (pixels.w as f32).hypot(pixels.h as f32);
        let inches = wmm.hypot(hmm) / 25.4;
        Some(px / inches).filter(|p| (40.0..=1000.0).contains(p))
    }
}

/// A native menu bar: its top-level menus, in order (skelter-widgets' `MenuModel` converts into it).
/// On macOS the platform puts the standard app menu (About, Services, Hide, Quit) first.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MenuDesc {
    pub menus: Vec<MenuNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuNode {
    Submenu {
        /// The caption without mnemonic markers.
        label: String,
        enabled: bool,
        items: Vec<MenuNode>,
    },
    Item {
        /// What [`AppHandler::menu_activated`] reports.
        id: String,
        label: String,
        /// The accelerator as authored (`Ctrl+Shift+S`, `Primary+O`; `Primary` is Cmd on macOS).
        shortcut: String,
        enabled: bool,
        /// `Some(checked)` for check and radio items (the app keeps radio groups exclusive).
        checked: Option<bool>,
    },
    Separator,
}

/// A popup's placement: `anchor` is a rect in the parent's window points; the popup opens on
/// `gravity`'s side of it, and the OS may flip or slide it to stay on screen.
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

impl PopupDesc {
    /// The popup's top-left (parent window points) for a popup of `size` when the OS places nothing
    /// itself: against the anchor's edge on `gravity`'s side, aligned with its start.
    pub fn origin(&self, size: Size) -> Point {
        let a = self.anchor;
        match self.gravity {
            PopupGravity::Below => Point::new(a.min.x, a.max.y),
            PopupGravity::Above => Point::new(a.min.x, a.min.y - size.h),
            PopupGravity::Right => Point::new(a.max.x, a.min.y),
            PopupGravity::Left => Point::new(a.min.x - size.w, a.min.y),
        }
    }
}

/// A window state change [`PlatformCx::set_window_state`] asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowStateRequest {
    Maximize,
    Unmaximize,
    Minimize,
    Fullscreen,
    Unfullscreen,
    /// Raise and focus the window (Wayland: `xdg_activation_v1`).
    Activate,
}

/// Text-input hints beyond the [`InputPurpose`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ImeHints {
    pub multiline: bool,
    /// Turn off completion, spell checking and auto-capitalisation (code, identifiers).
    pub no_assist: bool,
    pub sensitive: bool,
}

/// A drag's payload: MIME type → bytes. Within the app the payload is usually one id under skelter's
/// own MIME type.
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
    /// This backend does not offer what was asked (named).
    Unsupported(&'static str),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::EventLoop(s) => write!(f, "event loop: {s}"),
            PlatformError::Os(s) => write!(f, "os: {s}"),
            PlatformError::Unsupported(what) => write!(f, "unsupported on this platform: {what}"),
        }
    }
}

impl std::error::Error for PlatformError {}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::geom::DeviceSize;

    #[test]
    fn ppi_from_physical_size() {
        // A 27" 4K panel: 597 x 336 mm.
        let p = Monitor::ppi_from_mm(DeviceSize { w: 3840, h: 2160 }, (597.0, 336.0)).unwrap();
        assert!((p - 163.2).abs() < 0.5, "{p}");
        // A 24" 1080p panel: 531 x 299 mm, about 92 ppi.
        let p = Monitor::ppi_from_mm(DeviceSize { w: 1920, h: 1080 }, (531.0, 299.0)).unwrap();
        assert!((p - 91.8).abs() < 0.5, "{p}");
        // Unknown, and an EDID holding the aspect ratio (16 x 9 mm).
        assert_eq!(Monitor::ppi_from_mm(DeviceSize { w: 1920, h: 1080 }, (0.0, 0.0)), None);
        assert_eq!(Monitor::ppi_from_mm(DeviceSize { w: 1920, h: 1080 }, (16.0, 9.0)), None);
        assert_eq!(Monitor::ppi_from_mm(DeviceSize { w: 0, h: 0 }, (300.0, 200.0)), None);
    }

    #[test]
    fn popup_origin_sits_on_the_gravity_side() {
        let anchor = Rect::new(10.0, 20.0, 50.0, 30.0);
        let p = |gravity| PopupDesc { parent: WindowId(1), anchor, gravity, grab: false }.origin(Size::new(100.0, 40.0));
        assert_eq!(p(PopupGravity::Below), Point::new(10.0, 30.0));
        assert_eq!(p(PopupGravity::Above), Point::new(10.0, -20.0));
        assert_eq!(p(PopupGravity::Right), Point::new(50.0, 20.0));
        assert_eq!(p(PopupGravity::Left), Point::new(-90.0, 20.0));
    }
}
