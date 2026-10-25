//! The Linux shim. On Wayland it opens a side connection on winit's own `wl_display` (its own event
//! queues, winit's untouched) for what winit 0.30 lacks:
//! - OS layers as `wl_subsurface`s of winit's toplevel surface (desync, no input region), sized with
//!   `wp_viewporter` where the compositor has it;
//! - a frame source from frame callbacks on the toplevel plus `wp_presentation` feedback, reporting
//!   occlusion when a requested callback has not come for 100 ms (winit has no `Occluded` there and
//!   spins when callbacks stop, winit #4668).
//!
//! On X11 (and when the side connection cannot be made) it is the generic shim.

use super::{ShimHooks, generic};
use crate::api::{HitRegion, ImeState, OsLayerCommitter, OsLayerDesc, SurfaceTarget, WindowDesc};
use crate::clock;
use crate::frame::{FrameSource, FrameWait, Presented};
use presto_base::event::PointerSample;
use presto_base::geom::{DeviceRect, Rect};
use presto_base::ids::{FrameId, SurfaceId};
use presto_base::time::{Span, Time};
use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle,
    WaylandWindowHandle,
};
use smallvec::SmallVec;
use std::os::fd::AsRawFd;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_callback::{self, WlCallback},
    wl_compositor::WlCompositor,
    wl_region::WlRegion,
    wl_registry::WlRegistry,
    wl_subcompositor::WlSubcompositor,
    wl_subsurface::WlSubsurface,
    wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::wp::presentation_time::client::{
    wp_presentation::{self, WpPresentation},
    wp_presentation_feedback::{self, WpPresentationFeedback},
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use winit::window::{Window, WindowAttributes};

/// Frame callbacks missing this long mean the surface is hidden (platform.md §3.2).
const OCCLUSION_AFTER: Span = Span::from_millis(100);
const DEFAULT_INTERVAL: Span = Span(16_666_667);

/// Events on the side connection's main queue: only the presentation clock matters.
#[derive(Default)]
struct CtxState {
    clock_id: Option<u32>,
}

impl Dispatch<WlRegistry, GlobalListContents> for CtxState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpPresentation, ()> for CtxState {
    fn event(
        state: &mut Self,
        _: &WpPresentation,
        ev: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = ev {
            state.clock_id = Some(clk_id);
        }
    }
}

delegate_noop!(CtxState: WlCompositor);
delegate_noop!(CtxState: WlSubcompositor);
delegate_noop!(CtxState: WlSubsurface);
delegate_noop!(CtxState: WlRegion);
delegate_noop!(CtxState: WpViewporter);
delegate_noop!(CtxState: WpViewport);
delegate_noop!(CtxState: ignore WlSurface);

/// The side connection and the globals bound on it.
struct WlCtx {
    conn: Connection,
    display: NonNull<std::ffi::c_void>,
    qh: QueueHandle<CtxState>,
    compositor: WlCompositor,
    subcompositor: WlSubcompositor,
    viewporter: Option<WpViewporter>,
    presentation: Option<WpPresentation>,
    clock_id: u32,
}

// SAFETY: `display` is winit's `wl_display`, which libwayland lets any thread use; the rest are
// wayland-client handles, which are Send + Sync.
unsafe impl Send for WlCtx {}
// SAFETY: as above; `display` is only read.
unsafe impl Sync for WlCtx {}

impl WlCtx {
    fn open(display: NonNull<std::ffi::c_void>) -> Result<Arc<WlCtx>, String> {
        // SAFETY: winit's live `wl_display`; the event loop (and so the display) outlives every
        // window and frame source made from it.
        let backend = unsafe { Backend::from_foreign_display(display.as_ptr() as *mut _) };
        let conn = Connection::from_backend(backend);
        let (globals, mut queue): (_, EventQueue<CtxState>) =
            registry_queue_init(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let compositor: WlCompositor = globals
            .bind(&qh, 4..=6, ())
            .map_err(|e| format!("wl_compositor: {e}"))?;
        let subcompositor: WlSubcompositor = globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| format!("wl_subcompositor: {e}"))?;
        let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
        let presentation: Option<WpPresentation> = globals.bind(&qh, 1..=1, ()).ok();
        let mut state = CtxState::default();
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        Ok(Arc::new(WlCtx {
            conn,
            display,
            qh,
            compositor,
            subcompositor,
            viewporter,
            presentation,
            clock_id: state.clock_id.unwrap_or(libc::CLOCK_MONOTONIC as u32),
        }))
    }

    /// One side connection per process (per display), opened on first use.
    fn shared(display: NonNull<std::ffi::c_void>) -> Option<Arc<WlCtx>> {
        static CTX: Mutex<Option<Arc<WlCtx>>> = Mutex::new(None);
        let mut g = CTX.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = g.as_ref().filter(|c| c.display == display) {
            return Some(c.clone());
        }
        match WlCtx::open(display) {
            Ok(c) => {
                *g = Some(c.clone());
                Some(c)
            }
            Err(e) => {
                log::warn!(
                    "presto-platform: Wayland side connection failed ({e}); OS layers and presentation feedback are off"
                );
                None
            }
        }
    }
}

struct SubLayer {
    id: SurfaceId,
    surface: WlSurface,
    subsurface: WlSubsurface,
    viewport: Option<WpViewport>,
}

pub struct WaylandShim {
    ctx: Arc<WlCtx>,
    parent: WlSurface,
    layers: Vec<SubLayer>,
    frame_source: Option<Arc<WaylandFrameSource>>,
}

pub enum WindowShim {
    Wayland(Box<WaylandShim>),
    Other(generic::WindowShim),
}

fn wayland_handles(
    window: &Window,
) -> Option<(NonNull<std::ffi::c_void>, NonNull<std::ffi::c_void>)> {
    let RawDisplayHandle::Wayland(d) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    let RawWindowHandle::Wayland(w) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    Some((d.display, w.surface))
}

impl WaylandShim {
    fn new(window: &Window) -> Option<Self> {
        let (display, surface) = wayland_handles(window)?;
        let ctx = WlCtx::shared(display)?;
        // SAFETY: winit's live toplevel `wl_surface`, owned by winit for the window's lifetime; we
        // only send requests on it (its events stay on winit's queue).
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr() as *mut _) }
            .ok()?;
        let parent = WlSurface::from_id(&ctx.conn, id).ok()?;
        Some(WaylandShim {
            ctx,
            parent,
            layers: Vec::new(),
            frame_source: None,
        })
    }

    fn display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Wayland(WaylandDisplayHandle::new(self.ctx.display))
    }

    fn place(&self, l: &SubLayer, bounds: Rect, scale: f32) {
        l.subsurface
            .set_position(bounds.min.x.round() as i32, bounds.min.y.round() as i32);
        match &l.viewport {
            Some(v) => v.set_destination(
                bounds.width().round().max(1.0) as i32,
                bounds.height().round().max(1.0) as i32,
            ),
            None => l.surface.set_buffer_scale(scale.round().max(1.0) as i32),
        }
        l.surface.commit();
    }

    fn create_layer(
        &mut self,
        id: SurfaceId,
        desc: OsLayerDesc,
        scale: f32,
    ) -> Option<SurfaceTarget> {
        let c = &self.ctx;
        let surface = c.compositor.create_surface(&c.qh, ());
        let subsurface = c
            .subcompositor
            .get_subsurface(&surface, &self.parent, &c.qh, ());
        subsurface.set_desync();
        // Input goes to the toplevel, which hit-tests the whole window.
        let region = c.compositor.create_region(&c.qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        let viewport = c
            .viewporter
            .as_ref()
            .map(|v| v.get_viewport(&surface, &c.qh, ()));
        let l = SubLayer {
            id,
            surface,
            subsurface,
            viewport,
        };
        self.place(&l, desc.bounds, scale);
        // Subsurface position applies on the parent's next commit (the compositor's next present).
        if let Err(e) = c.conn.flush() {
            log::warn!("presto-platform: Wayland flush failed: {e}");
        }
        let ptr = NonNull::new(l.surface.id().as_ptr() as *mut std::ffi::c_void)?;
        self.layers.push(l);
        Some(SurfaceTarget::Raw {
            window: RawWindowHandle::Wayland(WaylandWindowHandle::new(ptr)),
            display: self.display_handle(),
        })
    }

    fn set_layer_bounds(&mut self, id: SurfaceId, bounds: Rect, scale: f32) {
        if let Some(l) = self.layers.iter().find(|l| l.id == id) {
            self.place(l, bounds, scale);
            let _ = self.ctx.conn.flush();
        }
    }

    fn remove_layer(&mut self, id: SurfaceId) {
        if let Some(i) = self.layers.iter().position(|l| l.id == id) {
            let l = self.layers.remove(i);
            if let Some(v) = l.viewport {
                v.destroy();
            }
            l.subsurface.destroy();
            l.surface.destroy();
            let _ = self.ctx.conn.flush();
        }
    }

    fn frame_source(&mut self, interval: Option<Span>) -> Arc<dyn FrameSource> {
        if let Some(f) = &self.frame_source {
            return f.clone();
        }
        let f = Arc::new(WaylandFrameSource::new(
            self.ctx.clone(),
            self.parent.clone(),
            interval,
        ));
        self.frame_source = Some(f.clone());
        f
    }
}

impl Drop for WaylandShim {
    fn drop(&mut self) {
        for id in self.layers.iter().map(|l| l.id).collect::<Vec<_>>() {
            self.remove_layer(id);
        }
    }
}

impl WindowShim {
    pub fn install(window: &Window, desc: &WindowDesc, hooks: ShimHooks) -> Self {
        match WaylandShim::new(window) {
            Some(w) => WindowShim::Wayland(Box::new(w)),
            None => WindowShim::Other(generic::WindowShim::install(window, desc, hooks)),
        }
    }

    pub fn window_event(&mut self, ev: &winit::event::WindowEvent) {
        if let WindowShim::Other(g) = self {
            g.window_event(ev)
        }
    }

    pub fn surface_target(&self) -> Option<SurfaceTarget> {
        None
    }

    pub fn create_layer(
        &mut self,
        window: &Window,
        id: SurfaceId,
        desc: OsLayerDesc,
        scale: f32,
    ) -> Option<SurfaceTarget> {
        match self {
            WindowShim::Wayland(w) => w.create_layer(id, desc, scale),
            WindowShim::Other(g) => g.create_layer(window, id, desc, scale),
        }
    }

    pub fn set_layer_bounds(&mut self, id: SurfaceId, bounds: Rect, scale: f32) {
        if let WindowShim::Wayland(w) = self {
            w.set_layer_bounds(id, bounds, scale)
        }
    }

    pub fn remove_layer(&mut self, id: SurfaceId) {
        if let WindowShim::Wayland(w) = self {
            w.remove_layer(id)
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

    pub fn event_time(&self) -> Option<Time> {
        None
    }

    pub fn set_hit_regions(&mut self, _regions: &[HitRegion]) {}

    pub fn set_ime(&mut self, _state: Option<&ImeState>) {}

    pub fn end_composition(&mut self, _commit: bool) -> Option<bool> {
        None
    }

    /// The window's frame source: frame callbacks and presentation feedback on Wayland; `None` on X11.
    pub fn frame_source(&mut self, interval: Option<Span>) -> Option<Arc<dyn FrameSource>> {
        match self {
            WindowShim::Wayland(w) => Some(w.frame_source(interval)),
            WindowShim::Other(_) => None,
        }
    }
}

/// Events on a frame source's own queue.
#[derive(Default)]
struct FsState {
    clock_id: u32,
    /// When the last requested frame callback fired.
    done: Option<Time>,
    presented: Vec<Presented>,
    refresh: Option<Span>,
}

impl Dispatch<WlCallback, ()> for FsState {
    fn event(
        state: &mut Self,
        _: &WlCallback,
        ev: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = ev {
            state.done = Some(clock::now());
        }
    }
}

impl Dispatch<WpPresentationFeedback, FrameId> for FsState {
    fn event(
        state: &mut Self,
        _: &WpPresentationFeedback,
        ev: wp_presentation_feedback::Event,
        frame: &FrameId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match ev {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                ..
            } => {
                let secs = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
                let os_ns = secs * 1_000_000_000 + tv_nsec as u64;
                let at = match os_clock_ns(state.clock_id) {
                    Some(now) => clock::time_from_os(os_ns, now),
                    None => clock::now(),
                };
                if refresh > 0 {
                    state.refresh = Some(Span(refresh as i64));
                }
                state.presented.push(Presented {
                    frame: *frame,
                    at,
                    estimated: false,
                });
            }
            wp_presentation_feedback::Event::Discarded => {}
            _ => {}
        }
    }
}

fn os_clock_ns(clock_id: u32) -> Option<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `clock_gettime` writes into a local timespec.
    let r = unsafe { libc::clock_gettime(clock_id as libc::clockid_t, &mut ts) };
    (r == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
}

struct FsInner {
    queue: EventQueue<FsState>,
    state: FsState,
    occluded: bool,
    /// An unoccluded report was returned; the vsync that came with it is next.
    vsync_pending: Option<Time>,
}

/// Frame callbacks and presentation feedback for one toplevel, on its own queue.
pub struct WaylandFrameSource {
    ctx: Arc<WlCtx>,
    surface: WlSurface,
    qh: QueueHandle<FsState>,
    inner: Mutex<FsInner>,
    /// When the in-flight frame callback was requested (presto ns; 0 = none).
    requested: AtomicU64,
    interval_ns: AtomicU32,
}

impl WaylandFrameSource {
    fn new(ctx: Arc<WlCtx>, surface: WlSurface, interval: Option<Span>) -> Self {
        let queue = ctx.conn.new_event_queue::<FsState>();
        let qh = queue.handle();
        let state = FsState {
            clock_id: ctx.clock_id,
            ..Default::default()
        };
        let interval = interval
            .unwrap_or(DEFAULT_INTERVAL)
            .0
            .clamp(1_000_000, u32::MAX as i64) as u32;
        WaylandFrameSource {
            ctx,
            surface,
            qh,
            inner: Mutex::new(FsInner {
                queue,
                state,
                occluded: false,
                vsync_pending: None,
            }),
            requested: AtomicU64::new(0),
            interval_ns: AtomicU32::new(interval),
        }
    }

    fn interval_span(&self) -> Span {
        Span(self.interval_ns.load(Ordering::Relaxed) as i64)
    }

    /// Reads and dispatches events until `until`; returns early once anything was dispatched.
    fn pump(inner: &mut FsInner, conn: &Connection, until: Time) {
        let _ = conn.flush();
        if inner.queue.dispatch_pending(&mut inner.state).unwrap_or(0) > 0 {
            return;
        }
        let Some(guard) = inner.queue.prepare_read() else {
            let _ = inner.queue.dispatch_pending(&mut inner.state);
            return;
        };
        let ms = (until - clock::now())
            .as_millis_f64()
            .ceil()
            .clamp(0.0, i32::MAX as f64) as i32;
        let mut pfd = libc::pollfd {
            fd: guard.connection_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: polling one valid fd (the connection's) for at most `ms`.
        let n = unsafe { libc::poll(&mut pfd, 1, ms) };
        if n > 0 {
            if let Err(e) = guard.read() {
                log::warn!("presto-platform: Wayland read failed: {e}");
            }
        } else {
            drop(guard);
        }
        let _ = inner.queue.dispatch_pending(&mut inner.state);
    }
}

impl FrameSource for WaylandFrameSource {
    fn name(&self) -> &'static str {
        "wayland-frame-callback"
    }

    fn before_present(&self, frame: FrameId) {
        self.surface.frame(&self.qh, ());
        if let Some(p) = &self.ctx.presentation {
            p.feedback(&self.surface, &self.qh, frame);
        }
        let _ = self.requested.compare_exchange(
            0,
            clock::now().0.max(1),
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        // The requests must precede the present's commit on the wire.
        if let Err(e) = self.ctx.conn.flush() {
            log::warn!("presto-platform: Wayland flush failed: {e}");
        }
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = clock::now() + timeout;
        loop {
            let interval = inner.state.refresh.unwrap_or_else(|| self.interval_span());
            if let Some(t) = inner
                .vsync_pending
                .take()
                .or_else(|| inner.state.done.take())
            {
                self.requested.store(0, Ordering::SeqCst);
                if inner.occluded {
                    inner.occluded = false;
                    inner.vsync_pending = Some(t);
                    return FrameWait::Occluded(false);
                }
                return FrameWait::Vsync {
                    target: t + interval,
                    interval,
                    estimated: inner.state.refresh.is_none(),
                };
            }
            let now = clock::now();
            let req = self.requested.load(Ordering::SeqCst);
            if req == 0 {
                // Nothing presented, so no callback is coming: tick at the refresh interval.
                if inner.occluded || now + interval > deadline {
                    drop(inner);
                    crate::frame::sleep(deadline - now);
                    return FrameWait::TimedOut;
                }
                drop(inner);
                crate::frame::sleep(interval);
                let t = clock::now();
                return FrameWait::Vsync {
                    target: t + interval,
                    interval,
                    estimated: true,
                };
            }
            let starve_at = Time(req) + OCCLUSION_AFTER;
            if !inner.occluded && now >= starve_at {
                inner.occluded = true;
                return FrameWait::Occluded(true);
            }
            if now >= deadline {
                return FrameWait::TimedOut;
            }
            let until = if inner.occluded {
                deadline
            } else {
                deadline.min(starve_at)
            };
            let conn = self.ctx.conn.clone();
            Self::pump(&mut inner, &conn, until);
        }
    }

    fn take_presented(&self) -> Vec<Presented> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let FsInner { queue, state, .. } = &mut *inner;
        let _ = queue.dispatch_pending(state);
        std::mem::take(&mut state.presented)
    }

    fn interval(&self) -> Option<Span> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        Some(inner.state.refresh.unwrap_or_else(|| self.interval_span()))
    }
}

pub fn install_app_hooks(quit: Box<dyn Fn() + Send + Sync>) {
    drop(quit);
}

pub fn prepare_attributes(a: WindowAttributes, _desc: &WindowDesc) -> WindowAttributes {
    a
}

pub fn monitor_work_area(_m: &winit::monitor::MonitorHandle) -> Option<DeviceRect> {
    None
}
