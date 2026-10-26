//! AccessKit platform adapters, used directly (no `accesskit_winit`): UIA on Windows (subclassing
//! winit's HWND), NSAccessibility on macOS (subclassing winit's NSView), AT-SPI on Linux. Each one
//! reports activation, actions and deactivation through a sink; the app answers with full trees
//! and updates (`Commit.a11y`) through [`A11yAdapter::update`].

use crate::api::A11yEvent;
use skelter_base::accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use std::sync::Arc;
use winit::window::Window;

/// Where adapter callbacks go (they run on whichever thread the OS calls them on).
pub type A11ySink = Arc<dyn Fn(A11yEvent) + Send + Sync>;

struct Handler(A11ySink);

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // The tree lives on the UI side; it arrives through `update` once the app hears this.
        (self.0)(A11yEvent::Activated);
        None
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        (self.0)(A11yEvent::Action(request))
    }
}

impl DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        (self.0)(A11yEvent::Deactivated)
    }
}

/// One window's adapter. Create it before the window is first shown.
pub struct A11yAdapter {
    #[cfg(target_os = "windows")]
    inner: accesskit_windows::SubclassingAdapter,
    #[cfg(target_os = "macos")]
    inner: accesskit_macos::SubclassingAdapter,
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    inner: accesskit_unix::Adapter,
}

impl A11yAdapter {
    /// `None` on platforms without an adapter, or when the window has no native handle.
    #[allow(unused_variables, unreachable_code)]
    pub fn new(window: &Window, sink: A11ySink) -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else { return None };
            let hwnd = accesskit_windows::HWND(h.hwnd.get() as *mut _);
            let inner = accesskit_windows::SubclassingAdapter::new(hwnd, Handler(sink.clone()), Handler(sink));
            return Some(A11yAdapter { inner });
        }
        #[cfg(target_os = "macos")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else { return None };
            // SAFETY: `ns_view` is winit's live NSView for this window; the adapter is dropped with
            // the window's state, before winit releases the view.
            let inner = unsafe { accesskit_macos::SubclassingAdapter::new(h.ns_view.as_ptr(), Handler(sink.clone()), Handler(sink)) };
            return Some(A11yAdapter { inner });
        }
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
        {
            let inner = accesskit_unix::Adapter::new(Handler(sink.clone()), Handler(sink.clone()), Handler(sink));
            let mut a = A11yAdapter { inner };
            // AT-SPI's extents add the window's position: known from the start, not only after the
            // first move or resize (without a window manager there may be none)
            a.set_window_bounds(window);
            return Some(a);
        }
        None
    }

    /// AT-SPI: the window's outer and inner rectangles (screen pixels), which screen extents add.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    fn set_window_bounds(&mut self, window: &Window) {
        let outer_pos = window.outer_position().unwrap_or_default().cast::<f64>();
        let outer_size = window.outer_size().cast::<f64>();
        let inner_pos = window.inner_position().unwrap_or_default().cast::<f64>();
        let inner_size = window.inner_size().cast::<f64>();
        self.inner.set_root_window_bounds(
            skelter_base::accesskit::Rect::from_origin_size((outer_pos.x, outer_pos.y), (outer_size.width, outer_size.height)),
            skelter_base::accesskit::Rect::from_origin_size((inner_pos.x, inner_pos.y), (inner_size.width, inner_size.height)),
        );
    }

    /// Applies an update if an assistive technology is attached (otherwise drops it: the full tree
    /// goes out again on the next [`A11yEvent::Activated`]).
    pub fn update(&mut self, update: TreeUpdate) {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if let Some(events) = self.inner.update_if_active(|| update) {
            events.raise();
        }
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
        self.inner.update_if_active(|| update);
    }

    /// Keeps the adapter's focus and (AT-SPI) window bounds in step with the window.
    #[allow(unused_variables)]
    pub fn window_event(&mut self, window: &Window, ev: &winit::event::WindowEvent) {
        #[cfg(target_os = "macos")]
        if let winit::event::WindowEvent::Focused(f) = ev {
            if let Some(events) = self.inner.update_view_focus_state(*f) {
                events.raise();
            }
        }
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
        match ev {
            winit::event::WindowEvent::Focused(f) => self.inner.update_window_focus_state(*f),
            winit::event::WindowEvent::Moved(_) | winit::event::WindowEvent::Resized(_) => self.set_window_bounds(window),
            _ => {}
        }
    }
}
