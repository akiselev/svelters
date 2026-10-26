//! The app runner: [`App`] is the platform's [`AppHandler`] and ties the tree, the script host, the
//! compositor and the per-window frame state together on the UI thread (architecture §2, §11.1).
//!
//! - **Discrete input** (keys, buttons, IME, focus) is dispatched when it arrives: pending continuous
//!   input of the window first (order is kept), then native defaults, then JS. A change runs an urgent
//!   UI frame at once when the frame state allows it, otherwise it waits for the next `BeginFrame`.
//! - **Continuous input** (pointer moves, wheel deltas) is coalesced per window and dispatched at the
//!   start of the next UI frame.
//! - **Compositor messages** ([`ToUi`]) arrive on a channel the compositor fills from its thread; the
//!   platform wakes the UI thread and [`AppHandler::wake`] drains it.
//! - **Resize** is the one synchronous handshake: the new-size frame is committed, the compositor is
//!   told, and the UI thread waits up to [`AppConfig::resize_wait`] for `ResizeDone` (architecture §7.6).
//!
//! Every input is stamped through [`LatencyRecorder`] from `os` to its termination.

use crate::backends::UiFrame;
use crate::coalesce::{Coalescer, Pushed};
use crate::latency::{LatencyConfig, LatencyRecorder, Stage, Track};
use crate::seams::{CompositorApi, Dispatched, DocumentApi, ScriptHost};
use skelter_base::accesskit::ActionRequest;
use skelter_base::commit::{Commit, FastCommit, ToUi};
use skelter_base::event::{InputEnvelope, InputEvent, PointerPhase, WindowEvent};
use skelter_base::geom::{DeviceSize, Rect, Scale, Size};
use skelter_base::ids::{FrameId, InputSeq, Revision, WindowId};
use skelter_base::input::InputPurpose;
use skelter_base::time::{Clock, Span, Time};
use skelter_frame::Urgency;
use skelter_platform::{
    A11yEvent, AppHandler, ImeState, Monitor, PlatformCx, SurfaceTarget, Waker, WindowDesc,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Windows opened at `init`.
    pub windows: Vec<WindowDesc>,
    /// How long a resize waits for `ResizeDone` (about one refresh).
    pub resize_wait: Span,
    /// Exit when the last window closes.
    pub exit_on_last_window: bool,
    pub latency: LatencyConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            windows: vec![WindowDesc::new("skelter", Size::new(800.0, 600.0))],
            resize_wait: Span::from_micros(16_667),
            exit_on_last_window: true,
            latency: LatencyConfig::default(),
        }
    }
}

/// `Compositor::spawn`'s `to_ui` callback.
pub type ToUiFn = Box<dyn Fn(ToUi) + Send>;
/// `Compositor::spawn`'s `wake_ui` callback.
pub type WakeFn = Box<dyn Fn() + Send>;

/// The compositor's way back to the UI thread: `send` queues a message and wakes the platform loop.
#[derive(Clone)]
pub struct ToUiSink {
    tx: Sender<ToUi>,
    waker: Waker,
}

impl ToUiSink {
    pub fn send(&self, m: ToUi) {
        // A closed channel means the app is gone; the compositor is shutting down too.
        let _ = self.tx.send(m);
        self.waker.wake();
    }
    /// The `to_ui` and `wake_ui` callbacks of `Compositor::spawn`.
    pub fn callbacks(&self) -> (ToUiFn, WakeFn) {
        let tx = self.tx.clone();
        let waker = self.waker.clone();
        (
            Box::new(move |m| drop(tx.send(m))),
            Box::new(move || waker.wake()),
        )
    }
}

/// Builds the compositor in `init`, once the platform's waker and clock exist.
pub type SpawnCompositor<C> = Box<dyn FnOnce(ToUiSink, Arc<dyn Clock>, &[Monitor]) -> C>;

/// A monitor's refresh interval, when the platform knows its rate.
fn refresh_of(m: &Monitor) -> Option<Span> {
    m.refresh_millihertz
        .filter(|&r| r > 0)
        .map(|r| Span(1_000_000_000_000 / r as i64))
}

/// A monitor's pixels per inch for fling physics: the OS's physical density when it reports one,
/// else its logical DPI convention times the scale (96 × scale, 72 × scale on macOS).
pub fn monitor_ppi(m: &Monitor) -> f32 {
    m.ppi
        .filter(|p| *p > 0.0)
        .unwrap_or_else(|| crate::backends::logical_ppi(m.scale))
}

/// The monitor holding the desktop point `at` (device pixels), else the first one.
fn monitor_at(monitors: &[Monitor], at: Option<(i32, i32)>) -> Option<&Monitor> {
    at.and_then(|(x, y)| {
        monitors.iter().find(|m| {
            x >= m.position.0
                && y >= m.position.1
                && x < m.position.0 + m.size.w as i32
                && y < m.position.1 + m.size.h as i32
        })
    })
    .or(monitors.first())
}

struct WindowState {
    frame: UiFrame,
    size: DeviceSize,
    scale: Scale,
    visible: bool,
    /// The refresh interval last sent to the compositor (the window's monitor's).
    refresh: Option<Span>,
    /// The pixel density last sent to the compositor (the window's monitor's).
    ppi: Option<f32>,
    /// The window's last known desktop position (device pixels).
    position: Option<(i32, i32)>,
    /// The IME state last sent to the platform.
    ime: Option<(Rect, InputPurpose)>,
}

pub struct App<D: DocumentApi, C: CompositorApi> {
    cfg: AppConfig,
    doc: D,
    script: Option<Box<dyn ScriptHost<D>>>,
    spawn: Option<SpawnCompositor<C>>,
    compositor: Option<C>,
    tx: Sender<ToUi>,
    rx: Receiver<ToUi>,
    /// Messages that arrived while a resize waited for its `ResizeDone`.
    deferred: VecDeque<ToUi>,
    windows: HashMap<WindowId, WindowState>,
    coalescer: Coalescer,
    latency: LatencyRecorder,
    clock: Option<Arc<dyn Clock>>,
    resize_seq: u64,
    /// The platform timer armed for the documents' wakeups.
    timer: Option<Time>,
}

impl<D: DocumentApi + 'static, C: CompositorApi + 'static> App<D, C> {
    pub fn new(cfg: AppConfig, doc: D, spawn: SpawnCompositor<C>) -> Self {
        let (tx, rx) = channel();
        let latency = LatencyRecorder::new(cfg.latency);
        Self {
            cfg,
            doc,
            script: None,
            spawn: Some(spawn),
            compositor: None,
            tx,
            rx,
            deferred: VecDeque::new(),
            windows: HashMap::new(),
            coalescer: Coalescer::new(),
            latency,
            clock: None,
            resize_seq: 0,
            timer: None,
        }
    }

    pub fn with_script(mut self, script: Box<dyn ScriptHost<D>>) -> Self {
        self.script = Some(script);
        self
    }

    pub fn doc(&self) -> &D {
        &self.doc
    }
    pub fn doc_mut(&mut self) -> &mut D {
        &mut self.doc
    }
    pub fn compositor(&self) -> Option<&C> {
        self.compositor.as_ref()
    }
    pub fn latency(&self) -> &LatencyRecorder {
        &self.latency
    }
    pub fn latency_mut(&mut self) -> &mut LatencyRecorder {
        &mut self.latency
    }
    pub fn windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.keys().copied()
    }
    pub fn window_size(&self, w: WindowId) -> Option<(DeviceSize, Scale)> {
        self.windows.get(&w).map(|s| (s.size, s.scale))
    }

    /// Nothing is waiting on the UI side: no coalesced input, no unread compositor message, no visible
    /// window with uncommitted changes.
    pub fn is_idle(&self) -> bool {
        self.deferred.is_empty()
            && self.coalescer.windows().next().is_none()
            && self
                .windows
                .iter()
                .all(|(w, s)| !s.visible || (!s.frame.is_dirty() && !self.doc.needs_frame(*w)))
    }

    fn now(&self) -> Time {
        self.clock.as_ref().map_or(Time::ZERO, |c| c.now())
    }

    /// Open a window: create it on the platform, its surface on this thread, and hand both to the
    /// compositor.
    pub fn open_window(
        &mut self,
        cx: &mut dyn PlatformCx,
        desc: WindowDesc,
    ) -> Result<WindowId, String> {
        let size_pt = desc.size;
        let position = desc.position;
        let id = cx.create_window(desc);
        let scale = cx.window_scale(id).unwrap_or(Scale(1.0));
        let size = DeviceSize {
            w: (size_pt.w * scale.0).round() as u32,
            h: (size_pt.h * scale.0).round() as u32,
        };
        if let Some(c) = &self.compositor
            && let Err(e) = c.add_window(id, cx.surface_target(id), size, scale)
        {
            cx.close_window(id);
            return Err(e);
        }
        self.windows.insert(
            id,
            WindowState {
                frame: UiFrame::new(),
                ime: None,
                refresh: None,
                ppi: None,
                position: None,
                size,
                scale,
                visible: true,
            },
        );
        self.doc.resize(id, size_pt, scale);
        if let Some(blink) = self.doc.caret_blink()
            && let Some(c) = &self.compositor
        {
            c.set_caret_blink(id, blink);
        }
        let position = self.windows.get(&id).and_then(|w| w.position).or(position);
        self.sync_monitor(cx, id, position);
        self.request_frame(id);
        Ok(id)
    }

    /// Tells the compositor the refresh interval and pixel density of the monitor the window is on, when
    /// they changed: the platform's answer ([`PlatformCx::monitor_of`]), else the monitor at `position`
    /// (else the window's last known one, else the first monitor).
    fn sync_monitor(&mut self, cx: &mut dyn PlatformCx, w: WindowId, position: Option<(i32, i32)>) {
        let held = cx.monitor_of(w);
        let monitors = if held.is_some() {
            Vec::new()
        } else {
            cx.monitors()
        };
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        if position.is_some() {
            ws.position = position;
        }
        let Some(m) = held.as_ref().or_else(|| monitor_at(&monitors, ws.position)) else {
            return;
        };
        let interval = refresh_of(m);
        let ppi = monitor_ppi(m);
        if let Some(interval) = interval.filter(|i| ws.refresh != Some(*i)) {
            ws.refresh = Some(interval);
            if let Some(c) = &self.compositor {
                c.set_refresh(w, interval);
            }
        }
        if ws.ppi != Some(ppi) {
            ws.ppi = Some(ppi);
            if let Some(c) = &self.compositor {
                c.set_ppi(w, ppi);
            }
        }
    }

    pub fn close_window(&mut self, cx: &mut dyn PlatformCx, id: WindowId) {
        if self.windows.remove(&id).is_none() {
            return;
        }
        if let Some(c) = &self.compositor {
            c.remove_window(id);
        }
        self.coalescer.take(id);
        self.latency.drop_window(id);
        self.doc.close_window(id);
        cx.close_window(id);
        if self.windows.is_empty() && self.cfg.exit_on_last_window {
            cx.exit();
        }
    }

    fn request_frame(&mut self, w: WindowId) {
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        if ws.frame.mark_dirty()
            && ws.visible
            && let Some(c) = &self.compositor
        {
            c.request_ui_frame(w);
        }
    }

    /// Arm the platform timer for the earliest future wakeup a visible window's document asked for.
    fn arm_timer(&mut self, cx: &mut dyn PlatformCx) {
        let now = self.now();
        let next = self
            .windows
            .iter()
            .filter(|(_, s)| s.visible)
            .filter_map(|(w, _)| self.doc.next_wakeup(*w))
            .filter(|t| *t > now)
            .min();
        if next != self.timer {
            self.timer = next;
            cx.set_timer(next);
        }
    }

    /// Ask for a frame for every window whose document has changes no commit carries yet (state the
    /// host changed outside dispatch, an island asked for a pass).
    pub fn request_dirty_frames(&mut self) {
        let windows: Vec<WindowId> = self.windows.keys().copied().collect();
        for w in windows {
            self.request_if_dirty(w);
        }
    }

    fn request_if_dirty(&mut self, w: WindowId) {
        if self.doc.needs_frame(w) || self.coalescer.has_pending(w) {
            self.request_frame(w);
        }
    }

    /// Native defaults and JS for one input; stamps `dispatch_end` and `js_end`. Returns whether the
    /// tree changed. An input that changed nothing and wrote no fast commit terminates here.
    fn dispatch_one(&mut self, ev: &InputEnvelope) -> bool {
        let start = self.now();
        let d: Dispatched = self.doc.dispatch(ev);
        let t = self.now();
        self.latency.stamp(ev.seq, Stage::DispatchEnd, t);
        self.latency
            .slice(Track::Ui, "dispatch", ev.window, start, t);
        for f in &d.fast {
            self.latency.fast_sent(ev.seq, fast_revision(f), t);
            if let Some(c) = &self.compositor {
                c.fast(*f);
            }
        }
        let mut dirty = d.dirty;
        if d.js_events > 0
            && let Some(script) = self.script.as_mut()
        {
            dirty |= script.deliver(&mut self.doc, ev, &d);
            let t2 = self.now();
            self.latency.stamp(ev.seq, Stage::JsEnd, t2);
            self.latency.slice(Track::Ui, "js", ev.window, t, t2);
        }
        if !dirty && d.fast.is_empty() {
            self.latency.no_visual_change(ev.seq);
        }
        dirty
    }

    /// Dispatch a window's coalesced continuous input, oldest first.
    fn flush_coalesced(&mut self, w: WindowId) {
        for ev in self.coalescer.take(w) {
            self.dispatch_one(&ev);
        }
    }

    /// An urgent frame for `w` when the frame state allows one now, else a `BeginFrame` request.
    fn urgent(&mut self, cx: &mut dyn PlatformCx, w: WindowId) {
        let now = self.now();
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        if let Some(args) = self.compositor.as_ref().and_then(|c| c.frame_args(w)) {
            ws.frame.state.set_args(args);
        }
        match ws.frame.urgent(now) {
            (Urgency::RunNow { target }, _) => {
                self.latency.counters_mut(w).urgent += 1;
                self.run_frame(cx, w, None, target);
            }
            (Urgency::Wait, notify) => {
                if notify
                    && ws.visible
                    && let Some(c) = &self.compositor
                {
                    c.request_ui_frame(w);
                }
            }
        }
    }

    /// Run the UI phases for `w` (the frame state is already Running) and commit. `frame` is the
    /// `BeginFrame` answered (`None` = urgent).
    fn run_frame(
        &mut self,
        cx: &mut dyn PlatformCx,
        w: WindowId,
        frame: Option<FrameId>,
        target: Time,
    ) {
        let start = self.now();
        self.flush_coalesced(w);
        self.latency
            .stamp_pending(w, Stage::DispatchEnd, Stage::UiFrameStart, start);
        let commit = self.doc.frame(w, target);
        let now = self.now();
        self.latency.slice(Track::Ui, "ui_frame", w, start, now);
        match commit {
            Some(c) => self.send_commit(cx, w, frame, c),
            None => {
                self.latency.frame_without_commit(w);
                self.skip_frame(w, frame);
            }
        }
    }

    fn skip_frame(&mut self, w: WindowId, frame: Option<FrameId>) {
        self.latency.counters_mut(w).no_damage += 1;
        if let Some(ws) = self.windows.get_mut(&w) {
            ws.frame.skipped();
        }
        if let Some(f) = frame
            && let Some(c) = &self.compositor
        {
            c.ui_frame_skipped(w, f);
        }
    }

    fn send_commit(
        &mut self,
        cx: &mut dyn PlatformCx,
        w: WindowId,
        frame: Option<FrameId>,
        mut c: Commit,
    ) {
        let now = self.now();
        c.window = w;
        c.frame = frame;
        let seqs = self.latency.commit_sent(w, c.revision, now);
        if let Some(&max) = seqs.iter().max() {
            c.input_seq = c.input_seq.max(max);
        }
        if let Some(cursor) = c.cursor {
            cx.set_cursor(w, cursor);
        }
        // The IME follows focus: the document's focused editable node, else the commit's bare area.
        let ime = self
            .doc
            .ime_state(w)
            .or_else(|| c.ime_area.map(|a| (a, InputPurpose::Text)));
        if let Some(ws) = self.windows.get_mut(&w)
            && ws.ime != ime
        {
            ws.ime = ime;
            cx.set_ime(w, ime.map(|(area, purpose)| ImeState::new(area, purpose)));
        }
        if let Some(u) = &c.a11y {
            cx.update_a11y(w, u.clone());
        }
        // Grafted subtrees (egui islands) after the host tree that carries their graft nodes.
        for u in std::mem::take(&mut c.a11y_subtrees) {
            cx.update_a11y(w, u);
        }
        let report = self
            .windows
            .get_mut(&w)
            .and_then(|ws| ws.frame.committed(now, c.revision));
        if report.is_some_and(|r| r.late) {
            self.latency.counters_mut(w).ui_missed += 1;
        }
        if let Some(comp) = &self.compositor {
            // Fast commits queued while the frame ran (a caret placed in prepare) go first.
            for f in self.doc.take_fast_commits() {
                comp.fast(f);
            }
            comp.commit(c);
        }
        if let Some(menu) = self.doc.take_menu_bar() {
            let native = cx.set_menu_bar(menu.as_ref());
            self.doc.menu_bar_native(native && menu.is_some());
        }
        if let Some(comp) = &self.compositor
            && let Some(r) = report
            && let Some(f) = r.frame
        {
            comp.ui_cost(w, f, r.cost, r.input);
        }
        // The frame clock clears its request on a commit: ask again when the tree is still dirty.
        self.request_if_dirty(w);
    }

    /// The window was hidden (its in-flight inputs are dropped) or shown again (one frame if dirty).
    fn occlusion(&mut self, window: WindowId, occluded: bool) {
        let Some(ws) = self.windows.get_mut(&window) else {
            return;
        };
        ws.visible = !occluded;
        let dirty = ws.frame.is_dirty();
        if occluded {
            self.coalescer.take(window);
            self.latency.drop_window(window);
        } else if (dirty || self.doc.needs_frame(window))
            && let Some(c) = &self.compositor
        {
            // The compositor kept the damage; one UI frame brings the tree up to date.
            c.request_ui_frame(window);
        }
    }

    fn handle(&mut self, cx: &mut dyn PlatformCx, m: ToUi) {
        let now = self.now();
        match m {
            ToUi::BeginFrame {
                window,
                frame,
                target,
                deadline,
            } => {
                let Some(ws) = self.windows.get_mut(&window) else {
                    return;
                };
                if !ws.frame.begin(now, frame, target, deadline) {
                    // A commit is unapplied: asked again once it is (CommitApplied below).
                    return;
                }
                if self.doc.needs_frame(window) || self.coalescer.has_pending(window) {
                    self.run_frame(cx, window, Some(frame), target);
                } else {
                    self.skip_frame(window, Some(frame));
                }
            }
            ToUi::CommitApplied { window, revision } => {
                self.latency.commit_applied(window, revision, now);
                let again = self
                    .windows
                    .get_mut(&window)
                    .is_some_and(|ws| ws.frame.applied(revision));
                if again && let Some(c) = &self.compositor {
                    c.request_ui_frame(window);
                }
                self.request_if_dirty(window);
            }
            ToUi::Presented {
                window,
                frame,
                at,
                estimated,
                inputs,
            } => {
                self.latency
                    .presented(window, frame, at, estimated, &inputs);
            }
            ToUi::Animation { window, event } => {
                self.doc.animation_event(window, event);
                self.request_if_dirty(window);
            }
            ToUi::ScrollMoved {
                window,
                node,
                offset,
            } => {
                self.doc.scroll_moved(window, node, offset);
                self.request_if_dirty(window);
            }
            // A late reply to a resize that already timed out: nothing waits for it.
            ToUi::ResizeDone { .. } => {}
            // The platform's frame source saw the window covered or shown (the compositor knows).
            ToUi::Occluded { window, occluded } => self.occlusion(window, occluded),
            ToUi::FrameTimings {
                window,
                frame,
                draw,
                submit,
                gpu_done,
                present,
                estimated: _,
            } => {
                let mut stages = vec![
                    (Stage::Draw, draw),
                    (Stage::Submit, submit),
                    (Stage::Present, present),
                ];
                if let Some(g) = gpu_done {
                    stages.push((Stage::GpuDone, g));
                }
                self.latency.frame_stages(window, frame, &stages);
            }
            ToUi::DeviceReset => {
                self.doc.device_reset();
                let ws: Vec<_> = self.windows.keys().copied().collect();
                for w in ws {
                    self.request_frame(w);
                }
            }
        }
    }

    /// Drain every compositor message.
    pub fn pump(&mut self, cx: &mut dyn PlatformCx) {
        while let Some(m) = self.deferred.pop_front() {
            self.handle(cx, m);
        }
        while let Ok(m) = self.rx.try_recv() {
            self.handle(cx, m);
        }
        let now = self.now();
        self.latency.sweep(now);
        self.flush_fast();
    }

    /// Send the document's queued fast commits (carets, kinds' scrolls) to the compositor.
    fn flush_fast(&mut self) {
        let fast = self.doc.take_fast_commits();
        if let Some(c) = &self.compositor {
            for f in fast {
                c.fast(f);
            }
        }
    }

    /// The resize handshake: commit the new-size frame, tell the compositor, wait for its `ResizeDone`.
    fn resize(&mut self, cx: &mut dyn PlatformCx, w: WindowId, size: DeviceSize, scale: Scale) {
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        ws.size = size;
        ws.scale = scale;
        let pts = Size::new(size.w as f32 / scale.0, size.h as f32 / scale.0);
        self.doc.resize(w, pts, scale);
        self.urgent(cx, w);
        let Some(comp) = &self.compositor else { return };
        self.resize_seq += 1;
        let seq = self.resize_seq;
        comp.resize(w, size, scale, seq);
        let wait = std::time::Duration::from_nanos(self.cfg.resize_wait.0.max(0) as u64);
        let deadline = std::time::Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(ToUi::ResizeDone { window, seq: s, .. }) if window == w && s == seq => break,
                Ok(m) => self.deferred.push_back(m),
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                    self.latency.counters_mut(w).resize_timeouts += 1;
                    break;
                }
            }
        }
        self.pump(cx);
    }

    /// An AccessKit action (from the platform adapter or a test): handled by the tree like discrete
    /// input, with an urgent frame when it changed something.
    pub fn accessibility_action(
        &mut self,
        cx: &mut dyn PlatformCx,
        w: WindowId,
        req: &ActionRequest,
    ) -> bool {
        let handled = self.doc.accessibility_action(w, req);
        if self.doc.needs_frame(w) {
            self.urgent(cx, w);
        }
        self.flush_fast();
        handled
    }
}

/// The revision a fast commit was written at (canvas views carry none: they never conflict with JS).
fn fast_revision(f: &FastCommit) -> Revision {
    match *f {
        FastCommit::ScrollOffset { revision, .. }
        | FastCommit::Transform { revision, .. }
        | FastCommit::Opacity { revision, .. }
        | FastCommit::Caret { revision, .. } => revision,
        FastCommit::CanvasView { .. } => Revision(0),
    }
}

impl<D: DocumentApi + 'static, C: CompositorApi + 'static> AppHandler for App<D, C> {
    fn init(&mut self, cx: &mut dyn PlatformCx) {
        let clock = cx.clock();
        self.clock = Some(clock.clone());
        if let Some(spawn) = self.spawn.take() {
            let sink = ToUiSink {
                tx: self.tx.clone(),
                waker: cx.waker(),
            };
            self.compositor = Some(spawn(sink, clock, &cx.monitors()));
        }
        for desc in std::mem::take(&mut self.cfg.windows) {
            if let Err(e) = self.open_window(cx, desc) {
                eprintln!("skelter: cannot open window: {e}");
            }
        }
        // The kinds' clipboard: the OS one for the display the first window lives on (a Wayland
        // clipboard needs the connection), else an in-process one (headless, or no window yet).
        let display = self
            .windows
            .keys()
            .next()
            .and_then(|w| match cx.surface_target(*w) {
                SurfaceTarget::Raw { display, .. } => Some(display),
                _ => None,
            });
        let clipboard: Box<dyn skelter_platform::Clipboard> = match display {
            Some(d) => Box::new(skelter_platform::SystemClipboard::new(Some(d))),
            None => Box::new(skelter_platform::MemoryClipboard::default()),
        };
        self.doc.install_clipboard(clipboard);
    }

    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        self.input_one(cx, ev);
        self.flush_fast();
        self.arm_timer(cx);
    }

    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        self.window_event_one(cx, window, ev);
        self.flush_fast();
        self.arm_timer(cx);
    }

    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        self.pump(cx);
        self.arm_timer(cx);
    }

    fn timer(&mut self, cx: &mut dyn PlatformCx, _now: Time) {
        self.timer = None;
        self.pump(cx);
        self.request_dirty_frames();
        self.arm_timer(cx);
    }

    fn menu_activated(&mut self, cx: &mut dyn PlatformCx, id: &str) {
        self.doc.menu_activated(id);
        let windows: Vec<WindowId> = self.windows.keys().copied().collect();
        for w in windows {
            if self.doc.needs_frame(w) {
                self.urgent(cx, w);
            }
        }
        if let Some(menu) = self.doc.take_menu_bar() {
            let native = cx.set_menu_bar(menu.as_ref());
            self.doc.menu_bar_native(native && menu.is_some());
        }
        self.flush_fast();
    }

    fn a11y_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        self.a11y_event_one(cx, window, ev);
        self.arm_timer(cx);
    }

    fn exiting(&mut self, _cx: &mut dyn PlatformCx) {
        if let Some(c) = self.compositor.take() {
            c.shutdown();
        }
    }
}

impl<D: DocumentApi + 'static, C: CompositorApi + 'static> App<D, C> {
    fn input_one(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        let w = ev.window;
        if !self.windows.contains_key(&w) {
            return;
        }
        self.latency.begin(&ev);
        if !ev.event.is_discrete() {
            // Hover changes at event time; the move itself reaches JS with the frame (§11.1).
            if let InputEvent::Pointer(p) = &ev.event
                && p.phase == PointerPhase::Move
                && self.doc.hover(w, p.pos)
            {
                self.request_frame(w);
            }
            let seq: InputSeq = ev.seq;
            if let Pushed::Merged { old } = self.coalescer.push(ev) {
                self.latency.merged(old, seq);
            }
            if let Some(c) = &self.compositor {
                c.high_rate_input(w);
            }
            self.request_frame(w);
            return;
        }
        self.flush_coalesced(w);
        let dirty = self.dispatch_one(&ev);
        if dirty || self.doc.needs_frame(w) {
            self.urgent(cx, w);
        }
    }

    fn window_event_one(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::Resized { size, scale } => self.resize(cx, window, size, scale),
            WindowEvent::ScaleChanged { scale } => {
                // A scale change usually means another monitor.
                self.sync_monitor(cx, window, None);
                if let Some(size) = self.windows.get(&window).map(|s| s.size) {
                    self.resize(cx, window, size, scale);
                }
            }
            WindowEvent::Occluded(occluded) => {
                if let Some(c) = &self.compositor {
                    c.set_visible(window, !occluded);
                }
                self.occlusion(window, occluded);
            }
            WindowEvent::CloseRequested => self.close_window(cx, window),
            WindowEvent::Moved { x, y } => {
                // A move can take the window to another monitor with another refresh rate.
                self.sync_monitor(cx, window, Some((x, y)));
                self.doc.window_event(window, &ev);
                self.request_if_dirty(window);
            }
            WindowEvent::MonitorChanged => self.sync_monitor(cx, window, None),
            WindowEvent::Focused(_) | WindowEvent::ThemeChanged(_) => {
                self.doc.window_event(window, &ev);
                self.request_if_dirty(window);
            }
        }
    }

    fn a11y_event_one(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        match ev {
            A11yEvent::Action(req) => {
                self.accessibility_action(cx, window, &req);
            }
            A11yEvent::Activated => {
                self.doc.accessibility_activated(window);
                self.request_if_dirty(window);
            }
            A11yEvent::Deactivated => {}
        }
    }
}
