//! The app runner: [`App`] is the platform's [`AppHandler`] and ties the tree, the script host, the
//! compositor and the per-window frame state together on the UI thread (architecture §2, §11.1).
//!
//! - **Discrete input** (keys, buttons, IME, focus) is dispatched when it arrives: pending continuous
//!   input of the window first (order is kept), then native defaults, then JS. A change runs an urgent
//!   UI frame at once when the frame state allows it, otherwise it waits for the next `BeginFrame`.
//! - **Continuous input** (pointer moves, wheel deltas) is coalesced per window and dispatched at the
//!   start of the next UI frame.
//! - **Compositor messages** ([`ToUi`]) arrive on a channel the compositor fills from its thread; the
//!   platform wakes the UI thread and [`App::wake`] drains it.
//! - **Resize** is the one synchronous handshake: the new-size frame is committed, the compositor is
//!   told, and the UI thread waits up to [`AppConfig::resize_wait`] for `ResizeDone` (architecture §7.6).
//!
//! Every input is stamped through [`LatencyRecorder`] from `os` to its termination.

use crate::coalesce::{Coalescer, Pushed};
use crate::latency::{LatencyConfig, LatencyRecorder, Stage, Track};
use crate::shim::{
    AppHandler, CompositorApi, Dispatched, DocumentApi, FrameStateApi, ImeState, PlatformCx,
    ScriptHost, ShimUiFrameState, Urgency, Waker, WindowDesc,
};
use presto_base::accesskit::ActionRequest;
use presto_base::commit::{Commit, FastCommit, ToUi};
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Scale, Size};
use presto_base::ids::{FrameId, InputSeq, Revision, WindowId};
use presto_base::input::InputPurpose;
use presto_base::time::{Clock, Span, Time};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Windows opened at `init`.
    pub windows: Vec<WindowDesc>,
    /// The display interval assumed until presentation feedback says otherwise.
    pub interval: Span,
    /// How long a resize waits for `ResizeDone` (about one refresh).
    pub resize_wait: Span,
    /// Exit when the last window closes.
    pub exit_on_last_window: bool,
    pub latency: LatencyConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            windows: vec![WindowDesc::default()],
            interval: Span::from_micros(16_667),
            resize_wait: Span::from_micros(16_667),
            exit_on_last_window: true,
            latency: LatencyConfig::default(),
        }
    }
}

/// The compositor's way back to the UI thread: `to_ui` queues a message, `wake_ui` wakes the platform
/// loop. Hand [`ToUiSink::callbacks`] to `Compositor::spawn`.
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

/// `Compositor::spawn`'s `to_ui` callback.
pub type ToUiFn = Box<dyn Fn(ToUi) + Send>;
/// `Compositor::spawn`'s `wake_ui` callback.
pub type WakeFn = Box<dyn Fn() + Send>;
/// Builds the compositor once the platform's waker exists (in `init`).
pub type SpawnCompositor<C> = Box<dyn FnOnce(ToUiSink) -> C>;
/// Builds a window's UI frame state.
pub type MakeFrameState = Box<dyn FnMut(WindowId) -> Box<dyn FrameStateApi>>;

struct WindowState {
    frame: Box<dyn FrameStateApi>,
    size: DeviceSize,
    scale: Scale,
    visible: bool,
    /// A `BeginFrame` was asked for and has not arrived.
    frame_requested: bool,
}

pub struct App<D: DocumentApi, C: CompositorApi> {
    cfg: AppConfig,
    doc: D,
    script: Option<Box<dyn ScriptHost>>,
    spawn: Option<SpawnCompositor<C>>,
    compositor: Option<C>,
    make_frame_state: MakeFrameState,
    tx: Sender<ToUi>,
    rx: Receiver<ToUi>,
    /// Messages that arrived while a resize waited for its `ResizeDone`.
    deferred: VecDeque<ToUi>,
    windows: HashMap<WindowId, WindowState>,
    coalescer: Coalescer,
    latency: LatencyRecorder,
    clock: Option<Arc<dyn Clock>>,
    resize_seq: u64,
}

impl<D: DocumentApi + 'static, C: CompositorApi + 'static> App<D, C> {
    pub fn new(cfg: AppConfig, doc: D, spawn: SpawnCompositor<C>) -> Self {
        let (tx, rx) = channel();
        let interval = cfg.interval;
        let latency = LatencyRecorder::new(cfg.latency);
        Self {
            cfg,
            doc,
            script: None,
            spawn: Some(spawn),
            compositor: None,
            make_frame_state: Box::new(move |_| Box::new(ShimUiFrameState::new(interval))),
            tx,
            rx,
            deferred: VecDeque::new(),
            windows: HashMap::new(),
            coalescer: Coalescer::new(),
            latency,
            clock: None,
            resize_seq: 0,
        }
    }

    pub fn with_script(mut self, script: Box<dyn ScriptHost>) -> Self {
        self.script = Some(script);
        self
    }

    pub fn with_frame_state(mut self, make: MakeFrameState) -> Self {
        self.make_frame_state = make;
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

    /// Nothing is waiting: no coalesced input, no frame asked for, no window with uncommitted changes,
    /// no compositor message unread.
    pub fn is_idle(&self) -> bool {
        self.deferred.is_empty()
            && self.coalescer.windows().next().is_none()
            && self
                .windows
                .iter()
                .all(|(w, s)| !s.frame_requested && !self.doc.needs_frame(*w))
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
        let id = cx.create_window(desc);
        // The platform reports the real scale with the first `Resized`; until then, 1.
        let scale = Scale(1.0);
        let size = DeviceSize {
            w: size_pt.w.round() as u32,
            h: size_pt.h.round() as u32,
        };
        let target = cx.surface_target(id);
        if let Some(c) = &self.compositor {
            c.add_window(id, target, size, scale)?;
        }
        let frame = (self.make_frame_state)(id);
        self.windows.insert(
            id,
            WindowState {
                frame,
                size,
                scale,
                visible: true,
                frame_requested: false,
            },
        );
        self.doc.resize(id, size_pt, scale);
        self.request_frame(id);
        Ok(id)
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
        cx.close_window(id);
        if self.windows.is_empty() && self.cfg.exit_on_last_window {
            if let Some(c) = &self.compositor {
                c.shutdown();
            }
            cx.exit();
        }
    }

    fn request_frame(&mut self, w: WindowId) {
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        if ws.frame_requested || !ws.visible {
            return;
        }
        ws.frame_requested = true;
        if let Some(c) = &self.compositor {
            c.request_frame(w);
        }
    }

    fn request_if_dirty(&mut self, w: WindowId) {
        if self.doc.needs_frame(w) || self.coalescer.has_pending(w) {
            self.request_frame(w);
        }
    }

    /// Native defaults and JS for one input; stamps `dispatch_end` and `js_end`. Returns whether
    /// anything must reach the screen (a tree change, or a fast commit the compositor applies itself).
    fn dispatch_one(&mut self, ev: &InputEnvelope) -> bool {
        let start = self.now();
        let d: Dispatched = self.doc.dispatch(ev);
        let t = self.now();
        self.latency.stamp(ev.seq, Stage::DispatchEnd, t);
        self.latency
            .slice(Track::Ui, "dispatch", ev.window, start, t);
        if !d.fast.is_empty() {
            for f in &d.fast {
                self.latency.fast_sent(ev.seq, fast_revision(f), t);
            }
            if let Some(c) = &self.compositor {
                for f in d.fast.iter().copied() {
                    c.fast(f);
                }
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

    /// Run the UI phases for `w` and commit. `frame` is the `BeginFrame` answered (`None` = urgent).
    fn run_frame(
        &mut self,
        cx: &mut dyn PlatformCx,
        w: WindowId,
        frame: Option<FrameId>,
        target: Time,
        deadline: Time,
    ) {
        let start = self.now();
        self.flush_coalesced(w);
        self.latency
            .stamp_pending(w, Stage::DispatchEnd, Stage::UiFrameStart, start);
        if let Some(ws) = self.windows.get_mut(&w) {
            ws.frame_requested = false;
        }
        let commit = self.doc.frame(w, target);
        let now = self.now();
        self.latency.slice(Track::Ui, "ui_frame", w, start, now);
        match commit {
            Some(c) => self.send_commit(cx, w, frame, c, deadline),
            None => {
                self.latency.frame_without_commit(w);
                self.latency.counters_mut(w).no_damage += 1;
                if let Some(ws) = self.windows.get_mut(&w) {
                    ws.frame.frame_skipped(now);
                }
            }
        }
    }

    fn send_commit(
        &mut self,
        cx: &mut dyn PlatformCx,
        w: WindowId,
        frame: Option<FrameId>,
        mut c: Commit,
        deadline: Time,
    ) {
        let now = self.now();
        c.window = w;
        c.frame = frame;
        let seqs = self.latency.commit_sent(w, c.revision, now);
        if let Some(&max) = seqs.iter().max() {
            c.input_seq = c.input_seq.max(max);
        }
        if now > deadline {
            self.latency.counters_mut(w).ui_missed += 1;
        }
        if let Some(cursor) = c.cursor {
            cx.set_cursor(w, cursor);
        }
        if let Some(area) = c.ime_area {
            // The commit carries the area only; the purpose comes with focus (CORE request in the record).
            cx.set_ime(
                w,
                Some(ImeState {
                    area,
                    purpose: InputPurpose::Text,
                }),
            );
        }
        let revision = c.revision;
        if let Some(ws) = self.windows.get_mut(&w) {
            ws.frame.commit_sent(now, revision);
        }
        if let Some(comp) = &self.compositor {
            comp.commit(c);
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
                ws.frame_requested = false;
                if !ws.frame.on_begin_frame(now, frame, target, deadline) {
                    // A commit is unapplied: ask again once it is (CommitApplied below).
                    return;
                }
                if self.doc.needs_frame(window) || self.coalescer.has_pending(window) {
                    self.run_frame(cx, window, Some(frame), target, deadline);
                } else {
                    self.latency.counters_mut(window).no_damage += 1;
                    if let Some(ws) = self.windows.get_mut(&window) {
                        ws.frame.frame_skipped(now);
                    }
                }
            }
            ToUi::CommitApplied { window, revision } => {
                self.latency.commit_applied(window, revision, now);
                if let Some(ws) = self.windows.get_mut(&window) {
                    ws.frame.commit_applied(now, revision);
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
                if let Some(ws) = self.windows.get_mut(&window) {
                    ws.frame.presented(frame, at);
                }
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
    }

    /// The resize handshake: commit the new-size frame, tell the compositor, wait for its `ResizeDone`.
    fn resize(&mut self, cx: &mut dyn PlatformCx, w: WindowId, size: DeviceSize, scale: Scale) {
        let now = self.now();
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        ws.size = size;
        ws.scale = scale;
        let pts = Size::new(size.w as f32 / scale.0, size.h as f32 / scale.0);
        self.doc.resize(w, pts, scale);
        if let Urgency::RunNow { target } = ws.frame.on_urgent_input(now) {
            self.run_frame(cx, w, None, target, target);
        }
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
            let now = self.now();
            let urgency = self
                .windows
                .get_mut(&w)
                .map(|ws| ws.frame.on_urgent_input(now));
            match urgency {
                Some(Urgency::RunNow { target }) => {
                    self.run_frame(cx, w, None, target, target.max(now))
                }
                Some(Urgency::Wait) => self.request_frame(w),
                None => {}
            }
        }
        handled
    }

    fn sink(&self, waker: Waker) -> ToUiSink {
        ToUiSink {
            tx: self.tx.clone(),
            waker,
        }
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
        self.clock = Some(cx.clock());
        if let Some(spawn) = self.spawn.take() {
            let sink = self.sink(cx.waker());
            self.compositor = Some(spawn(sink));
        }
        for desc in std::mem::take(&mut self.cfg.windows) {
            // A window the compositor cannot take (no surface) is closed again rather than left blank.
            if let Err(e) = self.open_window(cx, desc) {
                eprintln!("presto: cannot open window: {e}");
            }
        }
    }

    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        let w = ev.window;
        if !self.windows.contains_key(&w) {
            return;
        }
        self.latency.begin(&ev);
        if !ev.event.is_discrete() {
            let seq: InputSeq = ev.seq;
            if let Pushed::Merged { old } = self.coalescer.push(ev) {
                self.latency.merged(old, seq);
            }
            self.request_frame(w);
            return;
        }
        self.flush_coalesced(w);
        let dirty = self.dispatch_one(&ev);
        if !dirty && !self.doc.needs_frame(w) {
            return;
        }
        let now = self.now();
        let Some(ws) = self.windows.get_mut(&w) else {
            return;
        };
        match ws.frame.on_urgent_input(now) {
            Urgency::RunNow { target } => {
                self.latency.counters_mut(w).urgent += 1;
                self.run_frame(cx, w, None, target, target.max(now));
            }
            Urgency::Wait => self.request_frame(w),
        }
    }

    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::Resized { size, scale } => self.resize(cx, window, size, scale),
            WindowEvent::ScaleChanged { scale } => {
                if let Some(size) = self.windows.get(&window).map(|s| s.size) {
                    self.resize(cx, window, size, scale);
                }
            }
            WindowEvent::Occluded(occluded) => {
                let Some(ws) = self.windows.get_mut(&window) else {
                    return;
                };
                ws.visible = !occluded;
                if occluded {
                    ws.frame_requested = false;
                }
                if let Some(c) = &self.compositor {
                    c.set_visible(window, !occluded);
                }
                if occluded {
                    self.coalescer.take(window);
                    self.latency.drop_window(window);
                } else {
                    // One frame when visible again (the compositor kept the damage).
                    self.request_frame(window);
                }
            }
            WindowEvent::CloseRequested => self.close_window(cx, window),
            WindowEvent::Focused(_) | WindowEvent::Moved { .. } | WindowEvent::ThemeChanged(_) => {
                self.doc.window_event(window, &ev);
                self.request_if_dirty(window);
            }
        }
    }

    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        self.pump(cx);
    }

    fn timer(&mut self, cx: &mut dyn PlatformCx, _now: Time) {
        self.pump(cx);
    }
}
