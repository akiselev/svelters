//! Starting an app on a platform backend (architecture §3, platform.md §5). The own backends come
//! first, each behind its default-on cargo feature: Win32 on Windows, AppKit on macOS, Wayland on a
//! Linux/BSD session with `WAYLAND_DISPLAY`. skelter-platform's winit backend runs everywhere else
//! (X11, a backend left out by its feature) and when an own backend fails to start, i.e. it returned an
//! error before it called the app's `init`. `SKELTER_BACKEND=winit` forces winit, `SKELTER_BACKEND=native`
//! turns the fallback off.

use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::Vec2;
use skelter_base::ids::WindowId;
use skelter_base::time::Time;
use skelter_platform::{A11yEvent, AppHandler, DragId, DragOutcome, PlatformCx, PlatformError};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The backend [`run`] picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Win32,
    Wayland,
    MacOs,
    Winit,
}

/// The own backend for this OS and session, if one is built in and applies.
pub fn native_backend() -> Option<Backend> {
    if std::env::var("SKELTER_BACKEND").as_deref() == Ok("winit") {
        return None;
    }
    #[cfg(all(target_os = "windows", feature = "win32"))]
    return Some(Backend::Win32);
    #[cfg(all(target_os = "macos", feature = "macos"))]
    return Some(Backend::MacOs);
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd"
        ),
        feature = "wayland"
    ))]
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()) {
        return Some(Backend::Wayland);
    }
    #[allow(unreachable_code)]
    None
}

/// Runs `handler` on the chosen backend until it exits; returns the backend that ran.
pub fn run(handler: Box<dyn AppHandler>) -> Result<Backend, PlatformError> {
    let shared = Rc::new(Shared {
        handler: RefCell::new(handler),
        started: Cell::new(false),
    });
    if let Some(b) = native_backend() {
        match run_native(b, Box::new(Forward(shared.clone()))) {
            Ok(()) => return Ok(b),
            Err(e)
                if shared.started.get()
                    || std::env::var("SKELTER_BACKEND").as_deref() == Ok("native") =>
            {
                return Err(e);
            }
            Err(e) => eprintln!("skelter: the {b:?} backend did not start ({e}); using winit"),
        }
    }
    skelter_platform::run(Box::new(Forward(shared)))?;
    Ok(Backend::Winit)
}

#[allow(unused_variables)]
fn run_native(b: Backend, handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    match b {
        #[cfg(all(target_os = "windows", feature = "win32"))]
        Backend::Win32 => skelter_platform_win32::run(handler),
        #[cfg(all(target_os = "macos", feature = "macos"))]
        Backend::MacOs => skelter_platform_macos::run(handler),
        #[cfg(all(
            any(
                target_os = "linux",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                target_os = "openbsd"
            ),
            feature = "wayland"
        ))]
        Backend::Wayland => skelter_platform_wayland::run(handler),
        _ => Err(PlatformError::EventLoop(format!(
            "the {b:?} backend is not built in"
        ))),
    }
}

/// The app, shared so a backend that fails to start hands it back for the fallback.
struct Shared {
    handler: RefCell<Box<dyn AppHandler>>,
    started: Cell<bool>,
}

/// Forwards every callback to the shared app.
struct Forward(Rc<Shared>);

impl AppHandler for Forward {
    fn init(&mut self, cx: &mut dyn PlatformCx) {
        self.0.started.set(true);
        self.0.handler.borrow_mut().init(cx);
    }
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        self.0.handler.borrow_mut().input(cx, ev);
    }
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        self.0.handler.borrow_mut().window_event(cx, window, ev);
    }
    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        self.0.handler.borrow_mut().wake(cx);
    }
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time) {
        self.0.handler.borrow_mut().timer(cx, now);
    }
    fn raw_mouse_motion(&mut self, cx: &mut dyn PlatformCx, window: WindowId, delta: Vec2) {
        self.0
            .handler
            .borrow_mut()
            .raw_mouse_motion(cx, window, delta);
    }
    fn a11y_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        self.0.handler.borrow_mut().a11y_event(cx, window, ev);
    }
    fn quit_requested(&mut self, cx: &mut dyn PlatformCx) -> bool {
        self.0.handler.borrow_mut().quit_requested(cx)
    }
    fn menu_activated(&mut self, cx: &mut dyn PlatformCx, id: &str) {
        self.0.handler.borrow_mut().menu_activated(cx, id);
    }
    fn drag_ended(&mut self, cx: &mut dyn PlatformCx, drag: DragId, outcome: DragOutcome) {
        self.0.handler.borrow_mut().drag_ended(cx, drag, outcome);
    }
    fn exiting(&mut self, cx: &mut dyn PlatformCx) {
        self.0.handler.borrow_mut().exiting(cx);
    }
}
