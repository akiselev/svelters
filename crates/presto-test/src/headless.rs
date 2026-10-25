//! The headless platform context and compositor the [`crate::Harness`] drives on a virtual clock.
//!
//! [`HeadlessCx`] stands in for `presto_platform::HeadlessPlatform` (lane PLATFORM) until it is merged:
//! no OS windows, an in-memory clipboard, and every platform request recorded for assertions.
//! [`HeadlessCompositor`] implements `presto::shim::CompositorApi` without a GPU: it applies commits at
//! each simulated vsync, reports `CommitApplied` and `Presented`, sends `BeginFrame` when the UI asked
//! for one or an animation runs, and answers resizes at once. Drawing goes through the real compositor
//! on lavapipe once `presto-compositor` is merged (`Harness::render`, not here yet).

use presto::ToUiSink;
use presto::shim::{
    Clipboard, CompositorApi, ImeState, PlatformCx, SurfaceTarget, Waker, WindowDesc,
};
use presto_base::accesskit::TreeUpdate;
use presto_base::anim::{
    AnimationCmd, AnimationEvent, AnimationSpec, Generator, SpringParams, StartTime,
};
use presto_base::commit::{Commit, FastCommit, ToUi};
use presto_base::cursor::CursorIcon;
use presto_base::geom::{DeviceSize, Scale};
use presto_base::ids::{AnimationId, FrameId, InputSeq, WindowId};
use presto_base::time::{Clock, Span, Time};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A clock that moves only when told.
#[derive(Clone, Default)]
pub struct VirtualClock(Arc<AtomicU64>);

impl VirtualClock {
    pub fn new(start: Time) -> Self {
        Self(Arc::new(AtomicU64::new(start.0)))
    }
    pub fn set(&self, t: Time) {
        self.0.store(t.0, Ordering::SeqCst);
    }
    pub fn advance(&self, s: Span) -> Time {
        let t = self.now() + s;
        self.set(t);
        t
    }
}

impl Clock for VirtualClock {
    fn now(&self) -> Time {
        Time(self.0.load(Ordering::SeqCst))
    }
}

#[derive(Default)]
struct MemClipboard(Option<String>);

impl Clipboard for MemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.clone()
    }
    fn set_text(&mut self, text: &str) {
        self.0 = Some(text.to_owned());
    }
}

/// The platform side of the harness.
pub struct HeadlessCx {
    clock: VirtualClock,
    next_window: u32,
    pub windows: BTreeMap<WindowId, WindowDesc>,
    pub cursors: HashMap<WindowId, CursorIcon>,
    pub ime: HashMap<WindowId, Option<ImeState>>,
    pub timer: Option<Time>,
    pub exited: bool,
    clipboard: MemClipboard,
    woken: Arc<AtomicBool>,
}

impl HeadlessCx {
    pub fn new(clock: VirtualClock) -> Self {
        Self {
            clock,
            next_window: 1,
            windows: BTreeMap::new(),
            cursors: HashMap::new(),
            ime: HashMap::new(),
            timer: None,
            exited: false,
            clipboard: MemClipboard::default(),
            woken: Arc::new(AtomicBool::new(false)),
        }
    }
    /// Whether something woke the UI thread since the last call.
    pub fn take_wake(&self) -> bool {
        self.woken.swap(false, Ordering::SeqCst)
    }
}

impl PlatformCx for HeadlessCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId {
        let id = WindowId(self.next_window);
        self.next_window += 1;
        self.windows.insert(id, desc);
        id
    }
    fn close_window(&mut self, id: WindowId) {
        self.windows.remove(&id);
    }
    fn surface_target(&self, id: WindowId) -> SurfaceTarget {
        SurfaceTarget(Box::new(id))
    }
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        self.cursors.insert(id, c);
    }
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        self.ime.insert(id, state);
    }
    fn clipboard(&mut self) -> &mut dyn Clipboard {
        &mut self.clipboard
    }
    fn set_timer(&mut self, at: Option<Time>) {
        self.timer = at;
    }
    fn waker(&self) -> Waker {
        let woken = self.woken.clone();
        Waker(Arc::new(move || woken.store(true, Ordering::SeqCst)))
    }
    fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(self.clock.clone())
    }
    fn exit(&mut self) {
        self.exited = true;
    }
}

struct CompWindow {
    size: DeviceSize,
    scale: Scale,
    visible: bool,
    frame_requested: bool,
    next_frame: u64,
}

struct Running {
    window: WindowId,
    start: Option<Time>,
    /// `None` = no end the headless compositor can compute (see [`end_of`]).
    length: Option<Span>,
}

#[derive(Default)]
struct CompState {
    windows: BTreeMap<WindowId, CompWindow>,
    pending: BTreeMap<WindowId, Vec<Commit>>,
    fast: BTreeMap<WindowId, Vec<FastCommit>>,
    animations: BTreeMap<AnimationId, Running>,
    a11y: Vec<(WindowId, TreeUpdate)>,
    applied: Vec<Commit>,
    resizes: Vec<(WindowId, DeviceSize, Scale, u64)>,
    shut_down: bool,
}

/// The length of an animation from its spec, when it has one without sampling: keyframes (delay +
/// duration × iterations + end delay) and `{duration, bounce}` springs. Physical and svelte/motion
/// springs need `presto_anim::spring`'s settle time (not merged yet): they never end here.
fn end_of(spec: &AnimationSpec) -> Option<Span> {
    match &spec.generator {
        Generator::Keyframes { timing, .. } => {
            if !timing.iterations.is_finite() {
                return None;
            }
            let active = (timing.duration.0 as f64 * timing.iterations.max(0.0) as f64) as i64;
            Some(timing.delay + Span(active) + timing.end_delay)
        }
        Generator::Spring {
            params: SpringParams::DurationBounce { duration, .. },
            ..
        } => Some(*duration),
        Generator::Spring { .. } => None,
    }
}

/// The GPU-less compositor.
pub struct HeadlessCompositor {
    state: Mutex<CompState>,
    sink: ToUiSink,
}

impl HeadlessCompositor {
    pub fn new(sink: ToUiSink) -> Self {
        Self {
            state: Mutex::new(CompState::default()),
            sink,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CompState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// One vsync at `now`: apply the folded commits (`CommitApplied`, `Presented { at: now }`), advance
    /// animations, and send `BeginFrame` for `now + interval` where a frame is wanted.
    pub fn vsync(&self, now: Time, interval: Span) {
        let mut out = Vec::new();
        {
            let mut st = self.lock();
            let st = &mut *st;
            let ids: Vec<_> = st.windows.keys().copied().collect();
            for w in ids {
                let commits = st.pending.remove(&w).unwrap_or_default();
                let fast = st.fast.remove(&w).unwrap_or_default();
                let visible = st.windows.get(&w).is_some_and(|cw| cw.visible);
                if !visible {
                    // Hidden: keep the changes for the frame after it shows again.
                    if !commits.is_empty() {
                        st.pending.insert(w, commits);
                    }
                    if !fast.is_empty() {
                        st.fast.insert(w, fast);
                    }
                    continue;
                }
                let mut inputs: Vec<InputSeq> = Vec::new();
                let mut frame = None;
                let mut last_revision = None;
                for c in commits {
                    for cmd in &c.animations {
                        match cmd {
                            AnimationCmd::Start(spec) => {
                                let start = match spec.start {
                                    StartTime::At(t) => Some(t),
                                    StartTime::NextFrame | StartTime::Group(_) => None,
                                };
                                st.animations.insert(
                                    spec.id,
                                    Running {
                                        window: w,
                                        start,
                                        length: end_of(spec),
                                    },
                                );
                            }
                            AnimationCmd::Cancel(id) => {
                                st.animations.remove(id);
                            }
                            AnimationCmd::Retarget { .. }
                            | AnimationCmd::Pause { .. }
                            | AnimationCmd::Resume { .. } => {}
                        }
                    }
                    if let Some(u) = &c.a11y {
                        st.a11y.push((w, u.clone()));
                    }
                    inputs.push(c.input_seq);
                    frame = frame.or(c.frame);
                    last_revision = Some(c.revision);
                    st.applied.push(c);
                }
                for f in &fast {
                    if let FastCommit::ScrollOffset { input_seq, .. }
                    | FastCommit::Transform { input_seq, .. }
                    | FastCommit::CanvasView { input_seq, .. } = *f
                    {
                        inputs.push(input_seq);
                    }
                }
                if let Some(revision) = last_revision {
                    out.push(ToUi::CommitApplied {
                        window: w,
                        revision,
                    });
                }
                let Some(cw) = st.windows.get_mut(&w) else {
                    continue;
                };
                if last_revision.is_some() || !fast.is_empty() {
                    let frame = frame.unwrap_or_else(|| {
                        cw.next_frame += 1;
                        FrameId(cw.next_frame)
                    });
                    out.push(ToUi::Presented {
                        window: w,
                        frame,
                        at: now,
                        estimated: false,
                        inputs,
                    });
                }
                let mut animating = false;
                let mut done = Vec::new();
                for (id, a) in st.animations.iter_mut().filter(|(_, a)| a.window == w) {
                    let start = *a.start.get_or_insert_with(|| {
                        out.push(ToUi::Animation {
                            window: w,
                            event: AnimationEvent::Started { id: *id, at: now },
                        });
                        now
                    });
                    match a.length {
                        Some(len) if now >= start + len => done.push((*id, start + len)),
                        _ => animating = true,
                    }
                }
                for (id, at) in done {
                    st.animations.remove(&id);
                    out.push(ToUi::Animation {
                        window: w,
                        event: AnimationEvent::Finished { id, at },
                    });
                }
                if cw.frame_requested || animating {
                    cw.frame_requested = false;
                    cw.next_frame += 1;
                    let target = now + interval;
                    out.push(ToUi::BeginFrame {
                        window: w,
                        frame: FrameId(cw.next_frame),
                        target,
                        deadline: target - Span(interval.0 / 2),
                    });
                }
            }
        }
        for m in out {
            self.sink.send(m);
        }
    }

    /// Nothing for the compositor to do: no unapplied commit, no frame asked for, no running animation.
    pub fn is_idle(&self) -> bool {
        let st = self.lock();
        st.pending.values().all(Vec::is_empty)
            && st.fast.values().all(Vec::is_empty)
            && st.windows.values().all(|w| !w.frame_requested)
            && st.animations.is_empty()
    }

    /// Animations that never end here (no computable length).
    pub fn endless_animations(&self) -> Vec<AnimationId> {
        self.lock()
            .animations
            .iter()
            .filter(|(_, a)| a.length.is_none())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn take_a11y(&self) -> Vec<(WindowId, TreeUpdate)> {
        std::mem::take(&mut self.lock().a11y)
    }

    /// Every commit applied so far, oldest first.
    pub fn applied(&self) -> Vec<Commit> {
        self.lock().applied.clone()
    }
    pub fn applied_count(&self) -> usize {
        self.lock().applied.len()
    }
    pub fn resizes(&self) -> Vec<(WindowId, DeviceSize, Scale, u64)> {
        self.lock().resizes.clone()
    }
    pub fn window_size(&self, w: WindowId) -> Option<(DeviceSize, Scale)> {
        self.lock().windows.get(&w).map(|cw| (cw.size, cw.scale))
    }
    pub fn is_shut_down(&self) -> bool {
        self.lock().shut_down
    }
}

impl CompositorApi for HeadlessCompositor {
    fn add_window(
        &self,
        window: WindowId,
        _target: SurfaceTarget,
        size: DeviceSize,
        scale: Scale,
    ) -> Result<(), String> {
        self.lock().windows.insert(
            window,
            CompWindow {
                size,
                scale,
                visible: true,
                frame_requested: false,
                next_frame: 0,
            },
        );
        Ok(())
    }
    fn remove_window(&self, window: WindowId) {
        let mut st = self.lock();
        st.windows.remove(&window);
        st.pending.remove(&window);
        st.fast.remove(&window);
        st.animations.retain(|_, a| a.window != window);
    }
    fn commit(&self, c: Commit) {
        self.lock().pending.entry(c.window).or_default().push(c);
    }
    fn fast(&self, f: FastCommit) {
        let w = match f {
            FastCommit::ScrollOffset { window, .. }
            | FastCommit::Transform { window, .. }
            | FastCommit::Opacity { window, .. }
            | FastCommit::CanvasView { window, .. }
            | FastCommit::Caret { window, .. } => window,
        };
        self.lock().fast.entry(w).or_default().push(f);
    }
    fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64) {
        {
            let mut st = self.lock();
            if let Some(cw) = st.windows.get_mut(&window) {
                cw.size = size;
                cw.scale = scale;
            }
            st.resizes.push((window, size, scale, seq));
        }
        self.sink.send(ToUi::ResizeDone {
            window,
            seq,
            size,
            scale,
        });
    }
    fn set_visible(&self, window: WindowId, visible: bool) {
        if let Some(cw) = self.lock().windows.get_mut(&window) {
            cw.visible = visible;
        }
    }
    fn request_frame(&self, window: WindowId) {
        if let Some(cw) = self.lock().windows.get_mut(&window) {
            cw.frame_requested = true;
        }
    }
    fn shutdown(&self) {
        self.lock().shut_down = true;
    }
}
