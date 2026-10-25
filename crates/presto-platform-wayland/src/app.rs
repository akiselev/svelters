//! The connection, the globals, the event loop and the `PlatformCx`.
//!
//! [`run`] connects to the compositor (`WAYLAND_DISPLAY`), binds the globals, and runs one loop on the main
//! thread: dispatch the queue, hand queued events to the [`AppHandler`], then `poll` on the Wayland socket and
//! a wake pipe until the earliest of the app's timer and key repeat. Protocol handlers never call the app
//! directly; they queue [`Pending`] events, delivered between dispatches, so the app can call back into the
//! platform (create windows, which waits for their first configure) without re-entering the dispatcher.

use crate::data::DataState;
use crate::present::FrameGlobals;
use crate::seat::Seat;
use crate::shim::{AppHandler, DragId, DragOutcome, PlatformError, Waker};
use crate::time;
use crate::window::Window;
use presto_base::event::{InputEnvelope, InputEvent, Theme, WindowEvent};
use presto_base::ids::{InputSeq, WindowId};
use presto_base::time::Time;
use rustix::event::{PollFd, PollFlags};
use smithay_client_toolkit::activation::{ActivationHandler, ActivationState, RequestData};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::subcompositor::SubcompositorState;
use smithay_client_toolkit::{
    delegate_activation, delegate_compositor, delegate_output, delegate_registry, delegate_seat, delegate_shm,
    delegate_subcompositor, delegate_xdg_popup, delegate_xdg_shell, delegate_xdg_window, registry_handlers,
};
use std::collections::{HashMap, VecDeque};
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalList, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::commit_timing::v1::client::wp_commit_timing_manager_v1::WpCommitTimingManagerV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols::wp::fifo::v1::client::wp_fifo_manager_v1::WpFifoManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
use wayland_protocols::wp::presentation_time::client::wp_presentation::{self, WpPresentation};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::toplevel_drag::v1::client::xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1;

pub use crate::cx::Cx;

/// An event waiting for the app.
pub(crate) enum Pending {
    Input(InputEnvelope),
    Window(WindowId, WindowEvent),
    DragEnded(DragId, DragOutcome),
}

/// Messages from helper threads (the portal), delivered through the wake pipe.
#[cfg_attr(not(feature = "portal"), allow(dead_code))]
pub(crate) enum Internal {
    Theme(Theme),
}

/// The optional protocols bound directly with wayland-client.
#[derive(Default)]
pub(crate) struct Protocols {
    pub fractional_scale: Option<WpFractionalScaleManagerV1>,
    pub viewporter: Option<WpViewporter>,
    pub presentation: Option<WpPresentation>,
    pub fifo: Option<WpFifoManagerV1>,
    pub commit_timing: Option<WpCommitTimingManagerV1>,
    pub toplevel_drag: Option<XdgToplevelDragManagerV1>,
    pub cursor_shape: Option<WpCursorShapeManagerV1>,
    pub text_input: Option<ZwpTextInputManagerV3>,
    pub tablet: Option<ZwpTabletManagerV2>,
    pub gestures: Option<ZwpPointerGesturesV1>,
}

pub struct State {
    pub(crate) conn: Connection,
    pub(crate) qh: QueueHandle<State>,
    pub(crate) registry: RegistryState,
    pub(crate) compositor: CompositorState,
    pub(crate) subcompositor: Option<SubcompositorState>,
    pub(crate) outputs: OutputState,
    pub(crate) xdg: XdgShell,
    pub(crate) seat_state: SeatState,
    pub(crate) shm: Option<Shm>,
    pub(crate) activation: Option<ActivationState>,
    pub(crate) protocols: Protocols,
    pub(crate) presentation_clock: Arc<AtomicU32>,
    pub(crate) windows: HashMap<WindowId, Window>,
    /// Every surface we own (toplevels, popups, layers) → its window.
    pub(crate) surfaces: HashMap<ObjectId, WindowId>,
    pub(crate) seats: Vec<Seat>,
    pub(crate) data: DataState,
    pub(crate) pending: VecDeque<Pending>,
    pub(crate) next_window: u32,
    pub(crate) next_layer: u32,
    pub(crate) next_seq: u64,
    pub(crate) next_drag: u64,
    pub(crate) timer: Option<Time>,
    pub(crate) exit: bool,
    pub(crate) theme: Option<Theme>,
    pub(crate) finished_drags: Vec<(DragId, DragOutcome)>,
}

impl State {
    fn new(conn: Connection, globals: &GlobalList, qh: QueueHandle<State>) -> Result<Self, PlatformError> {
        let compositor =
            CompositorState::bind(globals, &qh).map_err(|_| PlatformError::Os("the compositor lacks wl_compositor".into()))?;
        let xdg = XdgShell::bind(globals, &qh).map_err(|_| PlatformError::Os("the compositor lacks xdg_wm_base".into()))?;
        let subcompositor = SubcompositorState::bind(compositor.wl_compositor().clone(), globals, &qh).ok();
        let protocols = Protocols {
            fractional_scale: globals.bind(&qh, 1..=1, ()).ok(),
            viewporter: globals.bind(&qh, 1..=1, ()).ok(),
            presentation: globals.bind(&qh, 1..=2, ()).ok(),
            fifo: globals.bind(&qh, 1..=1, ()).ok(),
            commit_timing: globals.bind(&qh, 1..=1, ()).ok(),
            toplevel_drag: globals.bind(&qh, 1..=1, ()).ok(),
            cursor_shape: globals.bind(&qh, 1..=1, ()).ok(),
            text_input: globals.bind(&qh, 1..=1, ()).ok(),
            tablet: globals.bind(&qh, 1..=1, ()).ok(),
            gestures: globals.bind(&qh, 1..=3, ()).ok(),
        };
        Ok(State {
            registry: RegistryState::new(globals),
            outputs: OutputState::new(globals, &qh),
            seat_state: SeatState::new(globals, &qh),
            shm: Shm::bind(globals, &qh).ok(),
            activation: ActivationState::bind(globals, &qh).ok(),
            data: DataState::bind(globals, &qh),
            compositor,
            subcompositor,
            xdg,
            protocols,
            presentation_clock: Arc::new(AtomicU32::new(time::CLOCK_MONOTONIC_ID)),
            windows: HashMap::new(),
            surfaces: HashMap::new(),
            seats: Vec::new(),
            pending: VecDeque::new(),
            next_window: 1,
            next_layer: 1,
            next_seq: 1,
            next_drag: 1,
            timer: None,
            exit: false,
            theme: None,
            finished_drags: Vec::new(),
            conn,
            qh,
        })
    }

    pub(crate) fn frame_globals(&self) -> FrameGlobals {
        FrameGlobals {
            presentation: self.protocols.presentation.clone(),
            presentation_clock: self.presentation_clock.clone(),
            fifo: self.protocols.fifo.clone(),
            commit_timing: self.protocols.commit_timing.clone(),
        }
    }

    /// Queues an input event for `window`, stamped with the next sequence number.
    pub(crate) fn push_input(&mut self, window: WindowId, os_time: Time, event: InputEvent) {
        let seq = InputSeq(self.next_seq);
        self.next_seq += 1;
        self.pending.push_back(Pending::Input(InputEnvelope { seq, window, os_time, recv_time: time::now(), event }));
    }

    pub(crate) fn push_window(&mut self, window: WindowId, ev: WindowEvent) {
        self.pending.push_back(Pending::Window(window, ev));
    }

    pub(crate) fn window_of(&self, surface: &wl_surface::WlSurface) -> Option<WindowId> {
        self.surfaces.get(&surface.id()).copied()
    }

    fn handle_internal(&mut self, msg: Internal) {
        match msg {
            Internal::Theme(theme) => {
                if self.theme != Some(theme) {
                    self.theme = Some(theme);
                    let ids: Vec<_> = self.windows.keys().copied().collect();
                    for id in ids {
                        self.push_window(id, WindowEvent::ThemeChanged(theme));
                    }
                }
            }
        }
    }
}

/// The wake pipe: [`Waker`]s and helper threads write a byte; the loop drains it.
struct WakePipe {
    read: OwnedFd,
    write: Arc<OwnedFd>,
    app_wake: Arc<AtomicBool>,
}

impl WakePipe {
    fn new() -> std::io::Result<Self> {
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK)?;
        Ok(WakePipe { read, write: Arc::new(write), app_wake: Arc::new(AtomicBool::new(false)) })
    }

    fn waker(&self) -> Waker {
        let write = self.write.clone();
        let flag = self.app_wake.clone();
        Waker::new(move || {
            flag.store(true, Ordering::Release);
            // A full pipe already wakes the loop: EAGAIN is fine.
            let _ = rustix::io::write(&*write, &[1]);
        })
    }

    fn internal_notifier(&self) -> impl Fn() + Send + Sync + 'static {
        let write = self.write.clone();
        move || {
            let _ = rustix::io::write(&*write, &[2]);
        }
    }

    fn drain(&self) -> bool {
        let mut buf = [0u8; 64];
        while matches!(rustix::io::read(&self.read, &mut buf), Ok(n) if n > 0) {}
        self.app_wake.swap(false, Ordering::AcqRel)
    }
}

/// Runs the Wayland backend until the app calls `exit` or the last window's connection breaks.
pub fn run(mut handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    let conn = Connection::connect_to_env().map_err(|e| PlatformError::EventLoop(format!("no Wayland display: {e}")))?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    let qh = queue.handle();
    let mut state = State::new(conn.clone(), &globals, qh)?;
    // Seats, outputs and the presentation clock arrive in the first roundtrip.
    queue.roundtrip(&mut state).map_err(|e| PlatformError::EventLoop(e.to_string()))?;

    let wake = WakePipe::new().map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    let waker = wake.waker();
    let (internal_tx, internal_rx) = mpsc::channel::<Internal>();
    #[cfg(feature = "portal")]
    crate::portal::watch_theme(internal_tx.clone(), Box::new(wake.internal_notifier()));
    #[cfg(not(feature = "portal"))]
    let _ = (&internal_tx, wake.internal_notifier());

    handler.init(&mut Cx::new(&mut state, &mut queue, &waker));
    loop {
        queue.dispatch_pending(&mut state).map_err(|e| PlatformError::EventLoop(e.to_string()))?;
        deliver(&mut *handler, &mut state, &mut queue, &waker);
        if state.exit {
            break;
        }
        let now = time::now();
        if let Some(t) = state.timer.filter(|&t| t <= now) {
            state.timer = None;
            handler.timer(&mut Cx::new(&mut state, &mut queue, &waker), t.max(now));
            continue;
        }
        if crate::seat::tick_repeat(&mut state, now) {
            continue;
        }

        queue.flush().map_err(|e| PlatformError::EventLoop(e.to_string()))?;
        let deadline = [state.timer, crate::seat::next_repeat(&state)].into_iter().flatten().min();
        let timeout = deadline.map(|d| {
            let ns = (d - time::now()).0.max(0);
            rustix::event::Timespec { tv_sec: ns / 1_000_000_000, tv_nsec: ns % 1_000_000_000 }
        });
        let Some(guard) = queue.prepare_read() else { continue };
        let (wl_ready, wake_ready) = {
            let wl_fd = guard.connection_fd();
            let mut fds = [PollFd::new(&wl_fd, PollFlags::IN), PollFd::new(&wake.read, PollFlags::IN)];
            match rustix::event::poll(&mut fds, timeout.as_ref()) {
                Ok(_) => (!fds[0].revents().is_empty(), !fds[1].revents().is_empty()),
                Err(rustix::io::Errno::INTR) => (false, false),
                Err(e) => return Err(PlatformError::EventLoop(e.to_string())),
            }
        };
        if wl_ready {
            if let Err(e) = guard.read() {
                use wayland_client::backend::WaylandError;
                match e {
                    WaylandError::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock => {}
                    e => return Err(PlatformError::EventLoop(e.to_string())),
                }
            }
        } else {
            drop(guard);
        }
        if wake_ready {
            let app_wake = wake.drain();
            while let Ok(msg) = internal_rx.try_recv() {
                state.handle_internal(msg);
            }
            if app_wake {
                queue.dispatch_pending(&mut state).map_err(|e| PlatformError::EventLoop(e.to_string()))?;
                deliver(&mut *handler, &mut state, &mut queue, &waker);
                handler.wake(&mut Cx::new(&mut state, &mut queue, &waker));
            }
        }
    }
    handler.exiting(&mut Cx::new(&mut state, &mut queue, &waker));
    // Windows close (and their surfaces go away) before the connection.
    state.windows.clear();
    let _ = queue.flush();
    Ok(())
}

fn deliver(handler: &mut dyn AppHandler, state: &mut State, queue: &mut EventQueue<State>, waker: &Waker) {
    while let Some(p) = state.pending.pop_front() {
        let mut cx = Cx::new(state, queue, waker);
        match p {
            Pending::Input(env) => handler.input(&mut cx, env),
            Pending::Window(id, ev) => handler.window_event(&mut cx, id, ev),
            Pending::DragEnded(id, outcome) => {
                cx.state_mut().finished_drags.push((id, outcome));
                handler.wake(&mut cx);
            }
        }
        if state.exit {
            return;
        }
    }
}

// sctk handlers that the window and seat modules don't own.

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, factor: i32) {
        if let Some(id) = self.window_of(surface) {
            crate::window::integer_scale_changed(self, id, surface, factor);
        }
    }

    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        // Frame callbacks run on the frame sources' own queues (present.rs), never on this one.
    }

    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}

    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        crate::seat::add_seat(self, qh, seat);
    }

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        crate::seat::add_capability(self, qh, &seat, capability);
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        crate::seat::remove_capability(self, &seat, capability);
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        crate::seat::remove_seat(self, &seat);
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        self.shm.as_mut().expect("wl_shm events arrive only when it is bound")
    }
}

impl ActivationHandler for State {
    type RequestData = RequestData;
    fn new_token(&mut self, token: String, data: &RequestData) {
        if let (Some(a), Some(surface)) = (&self.activation, data.surface.as_ref()) {
            a.activate::<State>(surface, token);
        }
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(State);
delegate_subcompositor!(State);
delegate_output!(State);
delegate_seat!(State);
delegate_shm!(State);
delegate_activation!(State);
delegate_xdg_shell!(State);
delegate_xdg_window!(State);
delegate_xdg_popup!(State);
delegate_registry!(State);

impl Dispatch<WpPresentation, ()> for State {
    fn event(state: &mut Self, _: &WpPresentation, event: wp_presentation::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            state.presentation_clock.store(clk_id, Ordering::Relaxed);
        }
    }
}

/// Dispatch for globals and objects without events.
macro_rules! no_events {
    ($($iface:ty),* $(,)?) => {$(
        impl Dispatch<$iface, ()> for State {
            fn event(_: &mut Self, _: &$iface, _: <$iface as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}

no_events!(
    WpFractionalScaleManagerV1,
    WpViewporter,
    wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport,
    WpFifoManagerV1,
    WpCommitTimingManagerV1,
    XdgToplevelDragManagerV1,
    wayland_protocols::xdg::toplevel_drag::v1::client::xdg_toplevel_drag_v1::XdgToplevelDragV1,
    WpCursorShapeManagerV1,
    wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
    ZwpTextInputManagerV3,
    ZwpTabletManagerV2,
    ZwpPointerGesturesV1,
    wayland_client::protocol::wl_region::WlRegion,
);
