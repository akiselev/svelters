//! Surface state locking and the Wayland frame source.
//!
//! **The lock.** A surface's double-buffered state that must match its buffer (the viewport destination with
//! fractional scale, the integer buffer scale) is applied on the commit that carries a buffer of the matching
//! size, and that commit is wgpu's present on the compositor thread. So the UI thread never sends that state
//! itself: it records it in the surface's [`SurfaceLock`] keyed by buffer size, and the frame source applies
//! it in [`FrameSource::before_present`] when the compositor presents a buffer of that size (the scheme of
//! Firefox's `WaylandSurfaceLock`). The lock also holds the toplevel's subsurfaces: they switch to sync mode
//! when a resize starts, so their new content lands atomically with the parent's, and back to desync after the
//! parent's first present at the new size (and not before an interactive resize ends).
//!
//! **The frame source** is presto-platform's [`FrameSource`], with its own event queue on the shared
//! connection (`wait` on the compositor's vsync thread, `before_present` on the compositor thread):
//! - frame callbacks are the vsync signal: armed with each present, or with a bare commit when `wait` finds
//!   none armed (nothing was presented since the last callback);
//! - `wp_presentation` feedback reports when each frame reached the screen and the output's refresh interval;
//! - the 100 ms rule: a frame callback outstanding for 100 ms means the compositor stopped drawing the surface
//!   (hidden, minimised, on another workspace): `wait` reports `Occluded(true)`, and `Occluded(false)` on the
//!   next callback (winit #4668 spins instead);
//! - `fifo-v1` where the compositor has it and the swapchain does not already use it: Mesa's Vulkan WSI
//!   creates its own `wp_fifo_v1` for FIFO present mode and a second one on the surface is a protocol error,
//!   so it is opt-in through [`FrameSourceConfig`].

use presto_platform::{FrameSource, FrameWait, Presented};
use crate::time;
use presto_base::geom::DeviceSize;
use presto_base::ids::FrameId;
use presto_base::time::{Span, Time};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols::wp::fifo::v1::client::wp_fifo_manager_v1::WpFifoManagerV1;
use wayland_protocols::wp::fifo::v1::client::wp_fifo_v1::WpFifoV1;
use wayland_protocols::wp::presentation_time::client::wp_presentation::WpPresentation;
use wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::{self, Kind, WpPresentationFeedback};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface;

/// A frame callback outstanding this long means the surface is not being drawn.
pub const OCCLUSION_TIMEOUT: Span = Span::from_millis(100);
/// The refresh interval assumed until presentation feedback or the output reports one.
pub const DEFAULT_INTERVAL: Span = Span::from_nanos(16_666_667);

/// Surface state to apply on the commit of a buffer of size `buffer`.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub buffer: DeviceSize,
    /// The surface's size in points (the viewport destination).
    pub logical: (i32, i32),
    /// `wl_surface.set_buffer_scale` (1 when a viewport scales the buffer).
    pub buffer_scale: i32,
    pub viewport: Option<WpViewport>,
    /// The toplevel's or popup's `xdg_surface`, whose window geometry is the logical size.
    pub xdg_surface: Option<XdgSurface>,
}

#[derive(Default)]
struct LockInner {
    pending: Option<Geometry>,
    applied: Option<DeviceSize>,
    applied_scale: i32,
    applied_logical: (i32, i32),
    children: Vec<WlSubsurface>,
    /// The children are in sync mode until the parent presents this size.
    sync_until: Option<DeviceSize>,
    /// An interactive resize is running: stay in sync mode until it ends.
    interactive: bool,
    synced: bool,
}

/// Shared between a surface's owner (UI thread) and its frame source (compositor thread).
#[derive(Default)]
pub struct SurfaceLock(Mutex<LockInner>);

impl SurfaceLock {
    /// Records the state the next buffer of `g.buffer` size must carry.
    pub fn set_geometry(&self, g: Geometry) {
        let mut l = self.0.lock().unwrap();
        let scale = if g.viewport.is_some() { 1 } else { g.buffer_scale };
        if l.applied == Some(g.buffer) && l.applied_scale == scale && l.applied_logical == g.logical {
            // Already on screen: nothing waits for a present that may never come.
            l.pending = None;
            return;
        }
        l.pending = Some(g);
    }

    /// The buffer size the compositor should render next (pending, else applied).
    pub fn target_size(&self) -> Option<DeviceSize> {
        let l = self.0.lock().unwrap();
        l.pending.as_ref().map(|g| g.buffer).or(l.applied)
    }

    pub fn add_child(&self, sub: WlSubsurface) {
        let mut l = self.0.lock().unwrap();
        if l.synced {
            sub.set_sync();
        } else {
            sub.set_desync();
        }
        l.children.push(sub);
    }

    pub fn remove_child(&self, sub: &WlSubsurface) {
        self.0.lock().unwrap().children.retain(|c| c != sub);
    }

    /// The window's children are in sync mode for a resize.
    pub fn is_synced(&self) -> bool {
        self.0.lock().unwrap().synced
    }

    pub fn clear_children(&self) {
        self.0.lock().unwrap().children.clear();
    }

    /// A resize to `buffer` starts (`interactive` while the user drags an edge): children go to sync mode so
    /// their state lands with the parent's commit at the new size.
    pub fn begin_resize(&self, buffer: DeviceSize, interactive: bool) {
        let mut l = self.0.lock().unwrap();
        l.interactive = interactive;
        if l.applied == Some(buffer) && !interactive {
            // Nothing to wait for: the parent will not necessarily present again.
            Self::set_synced(&mut l, false);
            l.sync_until = None;
            return;
        }
        l.sync_until = Some(buffer);
        Self::set_synced(&mut l, true);
    }

    fn set_synced(l: &mut LockInner, synced: bool) {
        if l.synced == synced {
            return;
        }
        l.synced = synced;
        for c in &l.children {
            if synced { c.set_sync() } else { c.set_desync() }
        }
    }

    /// Before a present: applies the pending state on `surface` and returns the buffer size it is for. The
    /// frame source does not learn the presented buffer's size, so the pending state goes with the next
    /// present (the compositor presents at the new size once the resize handshake ran; see the lane record).
    fn before_present(&self, surface: &WlSurface) -> Option<DeviceSize> {
        let mut l = self.0.lock().unwrap();
        let Some(g) = l.pending.take() else { return l.applied };
        match &g.viewport {
            Some(vp) => {
                vp.set_destination(g.logical.0.max(1), g.logical.1.max(1));
                if l.applied_scale != 1 {
                    surface.set_buffer_scale(1);
                }
                l.applied_scale = 1;
            }
            None => {
                if l.applied_scale != g.buffer_scale {
                    surface.set_buffer_scale(g.buffer_scale);
                }
                l.applied_scale = g.buffer_scale;
            }
        }
        if let Some(x) = &g.xdg_surface {
            x.set_window_geometry(0, 0, g.logical.0.max(1), g.logical.1.max(1));
        }
        l.applied = Some(g.buffer);
        l.applied_logical = g.logical;
        l.applied
    }

    fn has_pending(&self) -> bool {
        self.0.lock().unwrap().pending.is_some()
    }

    /// After presenting `size`: ends the resize's sync mode once the parent committed the new size.
    fn after_present(&self, size: Option<DeviceSize>) {
        let mut l = self.0.lock().unwrap();
        if size.is_some() && l.sync_until == size && !l.interactive {
            l.sync_until = None;
            Self::set_synced(&mut l, false);
        }
    }
}

/// Opt-in protocols for a frame source (see the module docs).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameSourceConfig {
    /// Use `wp_fifo_v1` barriers (only when the swapchain is not in FIFO present mode).
    pub fifo: bool,
}

/// The globals a frame source may use, bound on the main queue.
#[derive(Clone)]
pub struct FrameGlobals {
    pub presentation: Option<WpPresentation>,
    /// `wp_presentation.clock_id`, written when the main queue receives it.
    pub presentation_clock: Arc<AtomicU32>,
    pub fifo: Option<WpFifoManagerV1>,
}

struct FeedbackData {
    frame: FrameId,
}

/// State the frame source's private queue dispatches into.
struct Queue {
    presented: Vec<Presented>,
    callback_done: Option<Time>,
    interval: Option<Span>,
    clock: Arc<AtomicU32>,
}

struct Inner {
    queue: EventQueue<Queue>,
    state: Queue,
    /// When the outstanding frame callback was committed.
    callback_since: Option<Time>,
    /// The buffer size the previous present applied, to end a resize's sync mode on the next present.
    last_presented: Option<DeviceSize>,
    visible: bool,
}

/// presto-platform's [`FrameSource`] on Wayland. `wait` runs on the compositor's vsync thread and
/// `before_present` on the compositor thread; the queue sits behind a mutex that is never held while
/// blocking on the socket.
pub struct WaylandFrameSource {
    conn: Connection,
    qh: QueueHandle<Queue>,
    surface: WlSurface,
    lock: Arc<SurfaceLock>,
    presentation: Option<WpPresentation>,
    fifo: Option<WpFifoV1>,
    inner: Mutex<Inner>,
}

impl WaylandFrameSource {
    pub fn new(conn: &Connection, surface: WlSurface, lock: Arc<SurfaceLock>, globals: &FrameGlobals, cfg: FrameSourceConfig) -> Self {
        let queue = conn.new_event_queue();
        let qh = queue.handle();
        let fifo = if cfg.fifo { globals.fifo.as_ref().map(|m| m.get_fifo(&surface, &qh, ())) } else { None };
        WaylandFrameSource {
            conn: conn.clone(),
            qh,
            surface,
            lock,
            presentation: globals.presentation.clone(),
            fifo,
            inner: Mutex::new(Inner {
                queue,
                state: Queue {
                    presented: Vec::new(),
                    callback_done: None,
                    interval: None,
                    clock: globals.presentation_clock.clone(),
                },
                callback_since: None,
                last_presented: None,
                visible: true,
            }),
        }
    }

    /// Blocks until the socket is readable (then reads it into every queue on the connection) or `until`.
    fn read_socket(&self, until: Time) {
        let _ = self.conn.flush();
        let Some(guard) = self.conn.prepare_read() else { return };
        let ready = {
            let fd = guard.connection_fd();
            let mut fds = [rustix::event::PollFd::new(&fd, rustix::event::PollFlags::IN)];
            let ns = (until - time::now()).0.max(0);
            let ts = rustix::event::Timespec { tv_sec: ns / 1_000_000_000, tv_nsec: ns % 1_000_000_000 };
            matches!(rustix::event::poll(&mut fds, Some(&ts)), Ok(n) if n > 0)
        };
        if ready {
            let _ = guard.read();
        }
    }
}

impl FrameSource for WaylandFrameSource {
    fn name(&self) -> &'static str {
        "wayland frame callbacks"
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let deadline = time::now() + timeout;
        loop {
            let now = time::now();
            let mut wake_at = deadline;
            {
                let mut g = self.inner.lock().unwrap();
                let inner = &mut *g;
                let _ = inner.queue.dispatch_pending(&mut inner.state);
                if let Some(done) = inner.state.callback_done.take() {
                    inner.callback_since = None;
                    if !inner.visible {
                        inner.visible = true;
                        return FrameWait::Occluded(false);
                    }
                    let interval = inner.state.interval.unwrap_or(DEFAULT_INTERVAL);
                    return FrameWait::Vsync { target: done + interval, interval, estimated: inner.state.interval.is_none() };
                }
                match inner.callback_since {
                    Some(since) => {
                        if inner.visible && now - since >= OCCLUSION_TIMEOUT {
                            inner.visible = false;
                            return FrameWait::Occluded(true);
                        }
                        if inner.visible {
                            wake_at = wake_at.min(since + OCCLUSION_TIMEOUT);
                        }
                    }
                    None if !self.lock.has_pending() => {
                        // Nothing armed a callback (no present since the last one): a bare commit carries it.
                        self.surface.frame(&self.qh, ());
                        self.surface.commit();
                        inner.callback_since = Some(now);
                        wake_at = wake_at.min(now + OCCLUSION_TIMEOUT);
                    }
                    None => {}
                }
            }
            if now >= deadline {
                return FrameWait::TimedOut;
            }
            self.read_socket(wake_at);
        }
    }

    fn before_present(&self, frame: FrameId) {
        let mut g = self.inner.lock().unwrap();
        // The previous present landed: a resize waiting for it leaves sync mode.
        self.lock.after_present(g.last_presented.take());
        g.last_presented = self.lock.before_present(&self.surface);
        if g.callback_since.is_none() {
            self.surface.frame(&self.qh, ());
            g.callback_since = Some(time::now());
        }
        if let Some(p) = &self.presentation {
            p.feedback(&self.surface, &self.qh, FeedbackData { frame });
        }
        if let Some(f) = &self.fifo {
            f.set_barrier();
            f.wait_barrier();
        }
    }

    fn take_presented(&self) -> Vec<Presented> {
        let mut g = self.inner.lock().unwrap();
        let inner = &mut *g;
        let _ = inner.queue.dispatch_pending(&mut inner.state);
        std::mem::take(&mut inner.state.presented)
    }

    fn interval(&self) -> Option<Span> {
        self.inner.lock().unwrap().state.interval
    }
}

impl Dispatch<WlCallback, ()> for Queue {
    fn event(state: &mut Self, _: &WlCallback, event: wl_callback::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_callback::Event::Done { callback_data } = event {
            state.callback_done = Some(time::from_event_ms(callback_data, time::now()));
        }
    }
}

impl Dispatch<WpPresentationFeedback, FeedbackData> for Queue {
    fn event(
        state: &mut Self,
        _: &WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        data: &FeedbackData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation_feedback::Event::Presented { tv_sec_hi, tv_sec_lo, tv_nsec, refresh, flags, .. } = event {
            if refresh > 0 {
                state.interval = Some(Span::from_nanos(refresh as i64));
            }
            let clock = state.clock.load(Ordering::Relaxed);
            let hw = match flags {
                WEnum::Value(k) => k.contains(Kind::HwClock),
                WEnum::Unknown(bits) => bits & Kind::HwClock.bits() != 0,
            };
            let (at, estimated) = match time::from_presentation(clock, tv_sec_hi, tv_sec_lo, tv_nsec) {
                Some(t) => (t, !hw),
                None => (time::now(), true),
            };
            state.presented.push(Presented { frame: data.frame, at, estimated });
        }
    }
}

impl Dispatch<WpFifoV1, ()> for Queue {
    fn event(_: &mut Self, _: &WpFifoV1, _: <WpFifoV1 as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
