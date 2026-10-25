//! The macOS shim:
//! - the quit veto: winit's app delegate has no `applicationShouldTerminate:`, so Cmd-Q and the
//!   Dock's Quit terminate at once; we add that method to winit's delegate class, cancel the
//!   termination and ask the app ([`crate::AppHandler::quit_requested`]);
//! - OS layers as `CAMetalLayer` sublayers of winit's NSView layer, laid out from window points;
//! - OS timestamps from `NSEvent.timestamp` (seconds of system uptime).

use super::ShimHooks;
use crate::api::{HitRegion, ImeState, OsLayerCommitter, OsLayerDesc, SurfaceTarget, WindowDesc};
use crate::clock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::sel;
use objc2_app_kit::{NSApplication, NSScreen, NSView};
use objc2_foundation::{CGPoint, CGRect, CGSize, MainThreadMarker, NSProcessInfo};
use objc2_quartz_core::{CALayer, CAMetalLayer, CATransaction};
use presto_base::event::PointerSample;
use presto_base::geom::{DeviceRect, Rect};
use presto_base::ids::SurfaceId;
use presto_base::time::{Span, Time};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use smallvec::SmallVec;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::OnceLock;
use winit::platform::macos::MonitorHandleExtMacOS;
use winit::window::{Window, WindowAttributes};

struct Layer {
    id: SurfaceId,
    layer: Retained<CAMetalLayer>,
    bounds: Rect,
}

pub struct WindowShim {
    view: Option<Retained<NSView>>,
    layers: Vec<Layer>,
    scale: f32,
}

fn view_of(window: &Window) -> Option<Retained<NSView>> {
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: `ns_view` is winit's live NSView for this window; retaining keeps it alive for as
    // long as the shim holds it.
    unsafe { Retained::retain(h.ns_view.as_ptr() as *mut NSView) }
}

/// Runs `f` inside a transaction without implicit animations.
fn without_animation(f: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    f();
    CATransaction::commit();
}

impl WindowShim {
    pub fn install(window: &Window, _desc: &WindowDesc, _hooks: ShimHooks) -> Self {
        WindowShim {
            view: view_of(window),
            layers: Vec::new(),
            scale: window.scale_factor() as f32,
        }
    }

    fn host_layer(&self) -> Option<Retained<CALayer>> {
        let view = self.view.as_ref()?;
        view.setWantsLayer(true);
        // SAFETY: reading the view's backing layer on the main thread.
        unsafe { view.layer() }
    }

    /// The frame of `bounds` (window points, top-left origin) in the host layer's coordinates.
    fn frame_in(host: &CALayer, bounds: Rect) -> CGRect {
        let y = if host.isGeometryFlipped() {
            bounds.min.y as f64
        } else {
            host.bounds().size.height - bounds.max.y as f64
        };
        CGRect::new(
            CGPoint::new(bounds.min.x as f64, y),
            CGSize::new(bounds.width() as f64, bounds.height() as f64),
        )
    }

    fn relayout(&self) {
        let Some(host) = self.host_layer() else {
            return;
        };
        without_animation(|| {
            for l in &self.layers {
                l.layer.setFrame(Self::frame_in(&host, l.bounds));
                l.layer.setContentsScale(self.scale as f64);
            }
        });
    }

    pub fn window_event(&mut self, ev: &winit::event::WindowEvent) {
        match ev {
            winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = *scale_factor as f32;
                self.relayout();
            }
            // Unflipped hosts measure from the bottom edge, which moves with the height.
            winit::event::WindowEvent::Resized(_) if !self.layers.is_empty() => self.relayout(),
            _ => {}
        }
    }

    pub fn surface_target(&self) -> Option<SurfaceTarget> {
        None
    }

    pub fn create_layer(
        &mut self,
        _window: &Window,
        id: SurfaceId,
        desc: OsLayerDesc,
        scale: f32,
    ) -> Option<SurfaceTarget> {
        let host = self.host_layer()?;
        // SAFETY: creating a layer on the main thread.
        let layer = unsafe { CAMetalLayer::new() };
        without_animation(|| {
            layer.setFrame(Self::frame_in(&host, desc.bounds));
            layer.setContentsScale(scale as f64);
            layer.setOpaque(!desc.transparent);
            host.addSublayer(&layer);
        });
        let ptr = Retained::as_ptr(&layer) as *mut c_void;
        self.scale = scale;
        self.layers.push(Layer {
            id,
            layer,
            bounds: desc.bounds,
        });
        Some(SurfaceTarget::CoreAnimationLayer(ptr))
    }

    pub fn set_layer_bounds(&mut self, id: SurfaceId, bounds: Rect, _scale: f32) {
        let Some(host) = self.host_layer() else {
            return;
        };
        let Some(l) = self.layers.iter_mut().find(|l| l.id == id) else {
            return;
        };
        l.bounds = bounds;
        without_animation(|| l.layer.setFrame(Self::frame_in(&host, bounds)));
    }

    pub fn remove_layer(&mut self, id: SurfaceId) {
        if let Some(i) = self.layers.iter().position(|l| l.id == id) {
            let l = self.layers.remove(i);
            without_animation(|| l.layer.removeFromSuperlayer());
        }
    }

    pub fn committer(&self) -> Option<OsLayerCommitter> {
        None
    }

    pub fn commit_layers(&mut self) {}

    pub fn set_timer(&mut self, _at: Option<Time>) {}

    pub fn take_history(&mut self) -> SmallVec<[PointerSample; 4]> {
        SmallVec::new()
    }

    /// `NSApp.currentEvent.timestamp` (uptime seconds) on presto's clock.
    pub fn event_time(&self) -> Option<Time> {
        let mtm = MainThreadMarker::new()?;
        let ev = NSApplication::sharedApplication(mtm).currentEvent()?;
        // SAFETY: plain property reads.
        let (ts, uptime) = unsafe { (ev.timestamp(), NSProcessInfo::processInfo().systemUptime()) };
        let age = uptime - ts;
        (0.0..10.0)
            .contains(&age)
            .then(|| clock::time_from_os((ts * 1e9) as u64, (uptime * 1e9) as u64))
    }

    pub fn set_hit_regions(&mut self, _regions: &[HitRegion]) {}

    pub fn set_ime(&mut self, _state: Option<&ImeState>) {}

    /// One display link per process, started on first use (it follows the active displays).
    pub fn frame_source(
        &mut self,
        _interval: Option<Span>,
    ) -> Option<Arc<dyn crate::frame::FrameSource>> {
        static SOURCE: std::sync::Mutex<Option<Arc<crate::frame::macos::DisplayLinkSource>>> =
            std::sync::Mutex::new(None);
        let mut g = SOURCE.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = crate::frame::macos::DisplayLinkSource::new().map(Arc::new);
        }
        g.clone().map(|s| s as Arc<dyn crate::frame::FrameSource>)
    }

    /// winit's NSTextInputClient has no composition control (platform.md §6).
    pub fn end_composition(&mut self, _commit: bool) -> Option<bool> {
        None
    }
}

static QUIT: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// `NSApplicationTerminateReply::NSTerminateCancel`.
const NS_TERMINATE_CANCEL: usize = 0;

extern "C" fn should_terminate(_this: *mut AnyObject, _cmd: Sel, _sender: *mut AnyObject) -> usize {
    if let Some(q) = QUIT.get() {
        q();
    }
    NS_TERMINATE_CANCEL
}

/// Adds `applicationShouldTerminate:` to winit's app delegate class (registered when the event
/// loop was built), so a quit request reaches the app instead of terminating.
pub fn install_app_hooks(quit: Box<dyn Fn() + Send + Sync>) {
    if QUIT.set(quit).is_err() {
        return;
    }
    let name = c"WinitApplicationDelegate";
    // SAFETY: Objective-C runtime calls with a NUL-terminated class name, a selector from `sel!`,
    // an implementation whose signature matches the type encoding "Q@:@" (NSUInteger return,
    // self, _cmd, sender), on a class winit registered before this runs.
    unsafe {
        let cls = objc2::ffi::objc_getClass(name.as_ptr()) as *mut objc2::ffi::objc_class;
        if cls.is_null() {
            log::warn!(
                "presto-platform: winit's app delegate class is missing; Cmd-Q cannot be vetoed"
            );
            return;
        }
        let imp: unsafe extern "C" fn() = std::mem::transmute(
            should_terminate as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> usize,
        );
        let added = objc2::ffi::class_addMethod(
            cls,
            sel!(applicationShouldTerminate:).as_ptr(),
            Some(imp),
            c"Q@:@".as_ptr(),
        );
        if added == objc2::ffi::NO {
            log::warn!(
                "presto-platform: winit's app delegate already answers applicationShouldTerminate:; Cmd-Q is not routed"
            );
        }
    }
}

pub fn prepare_attributes(a: WindowAttributes, _desc: &WindowDesc) -> WindowAttributes {
    a
}

/// `NSScreen.visibleFrame` (points, bottom-left origin on the primary screen) in device pixels with
/// a top-left origin.
pub fn monitor_work_area(m: &winit::monitor::MonitorHandle) -> Option<DeviceRect> {
    let mtm = MainThreadMarker::new()?;
    let ptr = m.ns_screen()?;
    // SAFETY: winit returns a live NSScreen; retaining keeps it valid while we read it.
    let screen: Retained<NSScreen> = unsafe { Retained::retain(ptr as *mut NSScreen) }?;
    // SAFETY: reading the screen list on the main thread.
    let primary = unsafe { NSScreen::screens(mtm).firstObject() }?;
    let s = screen.backingScaleFactor();
    let vf = screen.visibleFrame();
    let top = primary.frame().size.height - (vf.origin.y + vf.size.height);
    Some(DeviceRect {
        x: (vf.origin.x * s).round() as i32,
        y: (top * s).round() as i32,
        w: (vf.size.width * s).round().max(0.0) as u32,
        h: (vf.size.height * s).round().max(0.0) as u32,
    })
}
