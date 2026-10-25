//! The GPU-less compositor the [`crate::Harness`] drives on the headless platform's virtual clock.
//!
//! [`HeadlessCompositor`] implements `presto::seams::CompositorApi` without a GPU: it applies commits at
//! each simulated vsync, reports `CommitApplied` and `Presented`, sends `BeginFrame` when the UI asked
//! for one or an animation runs, and answers resizes at once. It also folds each commit into a real
//! `presto_compositor::tree::WindowTree`, which [`HeadlessCompositor::render`] draws.

use crate::render::{GpuRender, Image, RenderError};
use presto::ToUiSink;
use presto::compositor_crate::tree::{SurfaceKey, WindowTree};
use presto::platform::SurfaceTarget;
use presto::seams::CompositorApi;
use presto_base::accesskit::TreeUpdate;
use presto_base::anim::{
    AnimationCmd, AnimationEvent, AnimationSpec, Generator, SpringParams, StartTime,
};
use presto_base::color::Color;
use presto_base::commit::{Commit, FastCommit, ToUi};
use presto_base::geom::{DeviceSize, Scale};
use presto_base::ids::{AnimationId, FrameId, InputSeq, WindowId};
use presto_base::scene::{DrawItem, FrameScene};
use presto_base::time::{Span, Time};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
    /// Each window's committed scene, folded as the real compositor folds it (what `render` draws).
    trees: BTreeMap<WindowId, WindowTree>,
    pending: BTreeMap<WindowId, Vec<Commit>>,
    fast: BTreeMap<WindowId, Vec<FastCommit>>,
    animations: BTreeMap<AnimationId, Running>,
    a11y: Vec<(WindowId, TreeUpdate)>,
    applied: Vec<Commit>,
    resizes: Vec<(WindowId, DeviceSize, Scale, u64)>,
    ui_costs: Vec<(WindowId, FrameId, Span)>,
    high_rate: u64,
    skipped: u64,
    caret_blinks: Vec<(WindowId, Option<presto::compositor_crate::CaretBlink>)>,
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

/// The GPU-less compositor. Clones share one state (the harness keeps one after the app drops its own).
#[derive(Clone)]
pub struct HeadlessCompositor {
    state: Arc<Mutex<CompState>>,
    sink: ToUiSink,
}

impl HeadlessCompositor {
    pub fn new(sink: ToUiSink) -> Self {
        Self {
            state: Arc::new(Mutex::new(CompState::default())),
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
                    st.trees.entry(w).or_default().apply_commit(c.clone());
                    st.applied.push(c);
                }
                for f in &fast {
                    // A stale or unknown-node fast commit is dropped, as the compositor drops it.
                    let _ = st.trees.entry(w).or_default().apply_fast(f);
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
    /// UI costs reported for answered `BeginFrame`s.
    pub fn ui_costs(&self) -> Vec<(WindowId, FrameId, Span)> {
        self.lock().ui_costs.clone()
    }
    /// How many continuous inputs were reported.
    pub fn high_rate_inputs(&self) -> u64 {
        self.lock().high_rate
    }
    /// Every caret blink the app set, per window, in order.
    pub fn caret_blinks(&self) -> Vec<(WindowId, Option<presto::compositor_crate::CaretBlink>)> {
        self.lock().caret_blinks.clone()
    }
    /// `BeginFrame`s the UI answered with nothing.
    pub fn skipped_frames(&self) -> u64 {
        self.lock().skipped
    }
    /// Draws the window's committed scene (its main surface, all of it) on `gpu`, cleared to `clear`.
    pub fn render(
        &self,
        window: WindowId,
        gpu: &mut GpuRender,
        clear: Color,
    ) -> Result<Image, RenderError> {
        let mut st = self.lock();
        let st = &mut *st;
        let (size, scale) = st
            .windows
            .get(&window)
            .map(|w| (w.size, w.scale))
            .ok_or_else(|| RenderError(format!("no window {window:?}")))?;
        let tree = st.trees.entry(window).or_default();
        let flat = tree.flatten();
        let items: Vec<DrawItem<'_>> = flat
            .get(&SurfaceKey::Main)
            .map(|v| &v[..])
            .unwrap_or(&[])
            .iter()
            .filter(|it| it.opacity > 0.0 && !it.bounds.is_empty())
            .map(|it| DrawItem {
                transform: it.transform,
                clip: it.clip,
                opacity: it.opacity,
                list: &it.list,
            })
            .collect();
        let scene = FrameScene {
            target: size,
            scale,
            clear: Some(clear),
            damage: &[],
            items: &items,
            resources: &tree.resources,
        };
        gpu.draw(&scene)
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
        st.trees.remove(&window);
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
    fn request_ui_frame(&self, window: WindowId) {
        if let Some(cw) = self.lock().windows.get_mut(&window) {
            cw.frame_requested = true;
        }
    }
    fn high_rate_input(&self, _window: WindowId) {
        self.lock().high_rate += 1;
    }
    fn ui_cost(&self, window: WindowId, frame: FrameId, cost: Span, _input: bool) {
        self.lock().ui_costs.push((window, frame, cost));
    }
    fn set_caret_blink(
        &self,
        window: WindowId,
        blink: Option<presto::compositor_crate::CaretBlink>,
    ) {
        self.lock().caret_blinks.push((window, blink));
    }
    fn ui_frame_skipped(&self, _window: WindowId, _frame: FrameId) {
        self.lock().skipped += 1;
    }
    fn shutdown(&self) {
        self.lock().shut_down = true;
    }
}
