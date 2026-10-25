//! Windows: our `NSWindow` subclass, its delegate, and the per-window state the platform keeps (layer,
//! OS layers, cursor, hit regions, text input, accessibility).

use crate::a11y::{self, A11y};
use crate::app::{self, Pending};
use crate::layer;
use crate::monitor;
use crate::text_input::SimpleIme;
use crate::view::SkelterView;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSView, NSWindow, NSWindowDelegate, NSWindowOcclusionState,
    NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::CAMetalLayer;
use skelter_base::cursor::CursorIcon;
use skelter_base::event::{KeyEvent, WindowEvent};
use skelter_base::geom::{DeviceSize, Scale};
use skelter_base::ids::{SurfaceId, WindowId};
use skelter_base::input::TextInputClient;
use skelter_platform::{HitRegion, WindowDesc};
use std::collections::HashMap;

/// A key press while the input method handles it (`interpretKeyEvents`).
pub(crate) struct PendingKey {
    pub event: KeyEvent,
    /// The IME already answered for it (delivered it, or turned it into text or a composition).
    pub consumed: bool,
}

pub(crate) struct WindowState {
    pub ns_window: Retained<SkelterWindow>,
    pub view: Retained<SkelterView>,
    _delegate: Retained<WindowDelegate>,
    pub layer: Retained<CAMetalLayer>,
    pub os_layers: HashMap<SurfaceId, Retained<CAMetalLayer>>,
    pub cursor: CursorIcon,
    pub hit_regions: Vec<HitRegion>,
    /// Text input without a client ([`PlatformCx::set_ime`](crate::PlatformCx::set_ime)).
    pub ime: Option<SimpleIme>,
    /// A full text-input client ([`set_text_input_client`](crate::set_text_input_client)); lent out
    /// (`None`) while the IME calls it.
    pub client: Option<Box<dyn TextInputClient>>,
    pub has_client: bool,
    pub key: Option<PendingKey>,
    pub a11y: A11y,
    pub last_size: Option<(DeviceSize, f32)>,
    pub live_resize: bool,
    /// The window's frame sources, for occlusion reports.
    pub frame_sources: Vec<crate::display_link::OcclusionHandle>,
    /// OS layers inside an open resize transaction (`begin_os_layer_resize`).
    pub layer_resizes: std::collections::HashSet<SurfaceId>,
}

impl WindowState {
    pub(crate) fn new(mtm: MainThreadMarker, id: WindowId, desc: &WindowDesc) -> Self {
        let mut style = if desc.decorations {
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
        } else {
            NSWindowStyleMask::Borderless
        };
        if desc.resizable {
            style |= NSWindowStyleMask::Resizable;
        }
        let content = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(desc.size.w as f64, desc.size.h as f64),
        );
        let window = SkelterWindow::new(mtm, content, style);
        // SAFETY: we keep the window alive ourselves (Retained) and close it explicitly.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&desc.title));
        window.setAcceptsMouseMovedEvents(true);
        if desc.custom_title_bar {
            set_custom_title_bar(&window, true);
        }
        if desc.transparent {
            window.setOpaque(false);
            window.setBackgroundColor(Some(&NSColor::clearColor()));
        }
        if let Some(min) = desc.min_size {
            window.setContentMinSize(NSSize::new(min.w as f64, min.h as f64));
        }
        if let Some(max) = desc.max_size {
            window.setContentMaxSize(NSSize::new(max.w as f64, max.h as f64));
        }

        let scale = window.backingScaleFactor();
        let layer = layer::new_window_layer(!desc.transparent, scale);
        let view = SkelterView::new(mtm, id, content);
        // Layer-hosting: the layer first, then wantsLayer.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        window.setContentView(Some(&view));
        window.makeFirstResponder(Some(&view));
        let delegate = WindowDelegate::new(mtm, id);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        match desc.position {
            // The outer top-left in the desktop's device pixels, at the primary screen's scale.
            Some((x, y)) => {
                let s = objc2_app_kit::NSScreen::mainScreen(mtm)
                    .map(|s| s.backingScaleFactor())
                    .unwrap_or(1.0);
                let ph = monitor::primary_height(mtm);
                window.setFrameTopLeftPoint(NSPoint::new(x as f64 / s, ph - y as f64 / s));
            }
            None => window.center(),
        }

        let view_ref: &NSView = &view;
        let a11y = A11y::new(&view_ref.retain(), id);
        WindowState {
            ns_window: window,
            view,
            _delegate: delegate,
            layer,
            os_layers: HashMap::new(),
            cursor: CursorIcon::Default,
            hit_regions: Vec::new(),
            ime: None,
            client: None,
            has_client: false,
            key: None,
            a11y,
            last_size: None,
            live_resize: false,
            frame_sources: Vec::new(),
            layer_resizes: Default::default(),
        }
    }

    pub(crate) fn scale(&self) -> f64 {
        self.ns_window.backingScaleFactor()
    }

    /// The content size in device pixels and the scale.
    pub(crate) fn device_size(&self) -> (DeviceSize, Scale) {
        size_of(&self.view, self.scale())
    }

    /// Closes the window; its delegate is detached first, so no callback reaches the platform.
    pub(crate) fn destroy(self) {
        self.ns_window.setDelegate(None);
        self.ns_window.orderOut(None);
        self.ns_window.close();
    }
}

pub(crate) fn size_of(view: &NSView, scale: f64) -> (DeviceSize, Scale) {
    let b = view.bounds().size;
    let size = DeviceSize {
        w: (b.width * scale).round().max(0.0) as u32,
        h: (b.height * scale).round().max(0.0) as u32,
    };
    (size, Scale(scale as f32))
}

pub(crate) fn finish_creation(window: &SkelterWindow, visible: bool, maximized: bool) {
    if visible {
        window.makeKeyAndOrderFront(None);
    }
    if maximized && !window.isZoomed() {
        window.zoom(None);
    }
}

/// Content under a transparent title bar (the window controls stay), for app-drawn title bars.
pub(crate) fn set_custom_title_bar(window: &NSWindow, on: bool) {
    let mut style = window.styleMask();
    if !style.contains(NSWindowStyleMask::Titled) {
        return;
    }
    style.set(NSWindowStyleMask::FullSizeContentView, on);
    window.setStyleMask(style);
    window.setTitlebarAppearsTransparent(on);
    window.setTitleVisibility(if on {
        NSWindowTitleVisibility::Hidden
    } else {
        NSWindowTitleVisibility::Visible
    });
}

pub(crate) fn discard_marked_text(view: &NSView) {
    if let Some(cx) = view.inputContext() {
        cx.discardMarkedText();
    }
}

pub(crate) fn invalidate_character_coordinates(view: &NSView) {
    if let Some(cx) = view.inputContext() {
        cx.invalidateCharacterCoordinates();
    }
}

define_class!(
    // SAFETY: NSWindow allows subclassing; SkelterWindow adds no ivars and does not implement Drop.
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "SkelterWindow"]
    pub(crate) struct SkelterWindow;

    impl SkelterWindow {
        /// Borderless windows can take keyboard focus too.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            true
        }

        /// VoiceOver asks the window for the focused element; the content view's adapter knows it.
        #[unsafe(method(accessibilityFocusedUIElement))]
        fn accessibility_focused(&self) -> *mut AnyObject {
            match self.contentView() {
                // SAFETY: SkelterView implements accessibilityFocusedUIElement.
                Some(view) => unsafe { msg_send![&*view, accessibilityFocusedUIElement] },
                None => std::ptr::null_mut(),
            }
        }
    }
);

impl SkelterWindow {
    fn new(mtm: MainThreadMarker, content: NSRect, style: NSWindowStyleMask) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSWindow's designated initialiser.
        unsafe {
            msg_send![super(this), initWithContentRect: content, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false]
        }
    }
}

pub(crate) struct DelegateIvars {
    window: WindowId,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; WindowDelegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SkelterWindowDelegate"]
    #[ivars = DelegateIvars]
    pub(crate) struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        /// Closing is the app's decision: it gets CloseRequested and calls close_window.
        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _sender: &NSWindow) -> bool {
            app::post(Pending::Window(
                self.ivars().window,
                WindowEvent::CloseRequested,
            ));
            false
        }

        #[unsafe(method(windowDidMove:))]
        fn did_move(&self, _note: &NSNotification) {
            let id = self.ivars().window;
            let Some(window) = app::with_window(id, |w| w.ns_window.clone()) else {
                return;
            };
            let content = window.contentRectForFrameRect(window.frame());
            let g = monitor::to_global(content, monitor::primary_height(self.mtm()));
            app::post(Pending::Window(
                id,
                WindowEvent::Moved {
                    x: g.min.x.round() as i32,
                    y: g.min.y.round() as i32,
                },
            ));
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn did_become_key(&self, _note: &NSNotification) {
            let id = self.ivars().window;
            a11y::focus_changed(id, true);
            app::post(Pending::Window(id, WindowEvent::Focused(true)));
        }

        #[unsafe(method(windowDidResignKey:))]
        fn did_resign_key(&self, _note: &NSNotification) {
            let id = self.ivars().window;
            a11y::focus_changed(id, false);
            app::post(Pending::Window(id, WindowEvent::Focused(false)));
        }

        #[unsafe(method(windowDidChangeOcclusionState:))]
        fn did_change_occlusion(&self, _note: &NSNotification) {
            let id = self.ivars().window;
            let Some((window, sources)) =
                app::with_window(id, |w| (w.ns_window.clone(), w.frame_sources.clone()))
            else {
                return;
            };
            let visible = window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible);
            let live: Vec<_> = sources.into_iter().filter(|s| s.notify(!visible)).collect();
            app::with_window(id, |w| w.frame_sources = live);
            app::post(Pending::Window(id, WindowEvent::Occluded(!visible)));
        }
    }
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, window: WindowId) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { window });
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}
