//! Latency instrumentation (architecture §16.3, `notes/round2/scheduler-latency.md` §5): one
//! [`LatencyRecord`] per [`InputSeq`], stamped at each [`Stage`] from the OS timestamp to presentation,
//! ending in exactly one [`Termination`]; log-linear histograms per (segment, input class, window); and
//! a Chrome JSON trace with one flow per input.
//!
//! The recorder lives on the UI thread. Compositor-side stages arrive through [`LatencyRecorder::frame_stages`]
//! and [`LatencyRecorder::presented`].

use skelter_base::event::{InputEnvelope, InputEvent, PointerPhase};
use skelter_base::ids::{FrameId, InputSeq, Revision, WindowId};
use skelter_base::time::{Span, Time};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Write as _;

/// The stages of one input, in pipeline order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Os = 0,
    Recv,
    DispatchEnd,
    JsEnd,
    UiFrameStart,
    Commit,
    CommitApplied,
    Draw,
    Submit,
    GpuDone,
    Present,
    Presented,
}

impl Stage {
    pub const COUNT: usize = 12;
    pub const ALL: [Stage; Stage::COUNT] = [
        Stage::Os,
        Stage::Recv,
        Stage::DispatchEnd,
        Stage::JsEnd,
        Stage::UiFrameStart,
        Stage::Commit,
        Stage::CommitApplied,
        Stage::Draw,
        Stage::Submit,
        Stage::GpuDone,
        Stage::Present,
        Stage::Presented,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Stage::Os => "os",
            Stage::Recv => "recv",
            Stage::DispatchEnd => "dispatch_end",
            Stage::JsEnd => "js_end",
            Stage::UiFrameStart => "ui_frame_start",
            Stage::Commit => "commit",
            Stage::CommitApplied => "commit_applied",
            Stage::Draw => "draw",
            Stage::Submit => "submit",
            Stage::GpuDone => "gpu_done",
            Stage::Present => "present",
            Stage::Presented => "presented",
        }
    }
    /// The thread the stage is stamped on, for the trace's tracks.
    pub const fn thread(self) -> Track {
        match self {
            Stage::Os
            | Stage::Recv
            | Stage::DispatchEnd
            | Stage::JsEnd
            | Stage::UiFrameStart
            | Stage::Commit => Track::Ui,
            Stage::CommitApplied
            | Stage::Draw
            | Stage::Submit
            | Stage::Present
            | Stage::Presented => Track::Compositor,
            Stage::GpuDone => Track::Gpu,
        }
    }
}

/// Input classes the histograms are keyed by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputClass {
    Key,
    Click,
    Wheel,
    PointerMove,
    Ime,
    Other,
}

impl InputClass {
    pub fn of(ev: &InputEvent) -> Self {
        match ev {
            InputEvent::Key(_) => InputClass::Key,
            InputEvent::Pointer(p) => match p.phase {
                PointerPhase::Down | PointerPhase::Up => InputClass::Click,
                PointerPhase::Move => InputClass::PointerMove,
                _ => InputClass::Other,
            },
            InputEvent::Wheel(_) => InputClass::Wheel,
            InputEvent::Ime(_) => InputClass::Ime,
            _ => InputClass::Other,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            InputClass::Key => "key",
            InputClass::Click => "click",
            InputClass::Wheel => "wheel",
            InputClass::PointerMove => "pointer_move",
            InputClass::Ime => "ime",
            InputClass::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropReason {
    /// The window was hidden or closed before the input's frame reached the screen.
    WindowGone,
    /// No terminal stage arrived within [`LatencyConfig::stale_after`] (lost feedback).
    Timeout,
}

/// How a record ended. Every [`InputSeq`] gets exactly one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Termination {
    Presented(FrameId),
    /// A continuous event merged into a later one (its samples live on in that event's history).
    MergedInto(InputSeq),
    /// The handlers ran and changed nothing visible.
    NoVisualChange,
    Dropped(DropReason),
}

/// Derived segments, named as Chrome's EventLatency names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Segment {
    OsToRecv,
    RecvToDispatchEnd,
    DispatchToJsEnd,
    /// Queueing between the handlers and the UI frame that carries the change.
    HandledToFrameStart,
    FrameStartToCommit,
    CommitToDraw,
    DrawToSubmit,
    SubmitToGpuDone,
    GpuDoneToPresent,
    PresentToPresented,
    InputToPresented,
}

impl Segment {
    pub const ALL: [Segment; 11] = [
        Segment::OsToRecv,
        Segment::RecvToDispatchEnd,
        Segment::DispatchToJsEnd,
        Segment::HandledToFrameStart,
        Segment::FrameStartToCommit,
        Segment::CommitToDraw,
        Segment::DrawToSubmit,
        Segment::SubmitToGpuDone,
        Segment::GpuDoneToPresent,
        Segment::PresentToPresented,
        Segment::InputToPresented,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Segment::OsToRecv => "OsToRecv",
            Segment::RecvToDispatchEnd => "RecvToDispatchEnd",
            Segment::DispatchToJsEnd => "DispatchToJsEnd",
            Segment::HandledToFrameStart => "HandledToFrameStart",
            Segment::FrameStartToCommit => "FrameStartToCommit",
            Segment::CommitToDraw => "CommitToDraw",
            Segment::DrawToSubmit => "DrawToSubmit",
            Segment::SubmitToGpuDone => "SubmitToGpuDone",
            Segment::GpuDoneToPresent => "GpuDoneToPresent",
            Segment::PresentToPresented => "PresentToPresented",
            Segment::InputToPresented => "InputToPresented",
        }
    }
    /// The span of this segment in `r`, when both ends were stamped. `HandledToFrameStart` starts at
    /// `js_end` when JS ran and at `dispatch_end` otherwise.
    pub fn span(self, r: &LatencyRecord) -> Option<Span> {
        let (a, b) = match self {
            Segment::OsToRecv => (r.at(Stage::Os)?, r.at(Stage::Recv)?),
            Segment::RecvToDispatchEnd => (r.at(Stage::Recv)?, r.at(Stage::DispatchEnd)?),
            Segment::DispatchToJsEnd => (r.at(Stage::DispatchEnd)?, r.at(Stage::JsEnd)?),
            Segment::HandledToFrameStart => (
                r.at(Stage::JsEnd).or(r.at(Stage::DispatchEnd))?,
                r.at(Stage::UiFrameStart)?,
            ),
            Segment::FrameStartToCommit => (r.at(Stage::UiFrameStart)?, r.at(Stage::Commit)?),
            Segment::CommitToDraw => (r.at(Stage::Commit)?, r.at(Stage::Draw)?),
            Segment::DrawToSubmit => (r.at(Stage::Draw)?, r.at(Stage::Submit)?),
            Segment::SubmitToGpuDone => (r.at(Stage::Submit)?, r.at(Stage::GpuDone)?),
            Segment::GpuDoneToPresent => (r.at(Stage::GpuDone)?, r.at(Stage::Present)?),
            Segment::PresentToPresented => (r.at(Stage::Present)?, r.at(Stage::Presented)?),
            Segment::InputToPresented => (r.at(Stage::Os)?, r.at(Stage::Presented)?),
        };
        Some(b - a)
    }
}

/// One input's journey.
#[derive(Clone, Debug, PartialEq)]
pub struct LatencyRecord {
    pub seq: InputSeq,
    pub window: WindowId,
    pub class: InputClass,
    stamps: [Option<Time>; Stage::COUNT],
    /// The commit that carried this input's change.
    pub revision: Option<Revision>,
    pub frame: Option<FrameId>,
    pub end: Option<Termination>,
    /// The presentation time was an estimate (no OS feedback).
    pub estimated: bool,
}

impl LatencyRecord {
    fn new(seq: InputSeq, window: WindowId, class: InputClass) -> Self {
        Self {
            seq,
            window,
            class,
            stamps: [None; Stage::COUNT],
            revision: None,
            frame: None,
            end: None,
            estimated: false,
        }
    }
    pub fn at(&self, s: Stage) -> Option<Time> {
        self.stamps[s as usize]
    }
    fn stamp(&mut self, s: Stage, t: Time) {
        // The first stamp of a stage wins: an input carried by two frames is measured by the first.
        self.stamps[s as usize].get_or_insert(t);
    }
    /// The latest stamped time (where the record's trace ends).
    pub fn last_stamp(&self) -> Option<Time> {
        self.stamps.iter().flatten().copied().max()
    }
}

/// Log-linear histogram of spans: exact below 128 µs, then 64 sub-buckets per power of two (relative
/// error under 1 %, i.e. two significant digits), up to about 16.7 s. Values are clamped into range.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    counts: Vec<u32>,
    total: u64,
    sum_us: u64,
    min_us: u64,
    max_us: u64,
}

const EXACT: u64 = 128;
const SUB_BITS: u32 = 6;
const SUB: u64 = 1 << SUB_BITS;
/// Powers of two from 2^7 µs to 2^23 µs (8.4 s) and the octave above, to ~16.7 s.
const OCTAVES: u64 = 17;
const BUCKETS: usize = (EXACT + OCTAVES * SUB) as usize;
const MAX_US: u64 = (1 << 24) - 1;

impl Default for Histogram {
    fn default() -> Self {
        Self {
            counts: vec![0; BUCKETS],
            total: 0,
            sum_us: 0,
            min_us: u64::MAX,
            max_us: 0,
        }
    }
}

impl Histogram {
    pub fn new() -> Self {
        Self::default()
    }
    fn index(us: u64) -> usize {
        if us < EXACT {
            return us as usize;
        }
        let e = 63 - us.leading_zeros() as u64; // ≥ 7
        let sub = (us >> (e - SUB_BITS as u64)) & (SUB - 1);
        (EXACT + (e - 7) * SUB + sub) as usize
    }
    /// The midpoint of bucket `i`, in µs.
    fn value(i: usize) -> u64 {
        let i = i as u64;
        if i < EXACT {
            return i;
        }
        let e = (i - EXACT) / SUB + 7;
        let sub = (i - EXACT) % SUB;
        let width = 1u64 << (e - SUB_BITS as u64);
        ((SUB + sub) << (e - SUB_BITS as u64)) + width / 2
    }
    pub fn record(&mut self, s: Span) {
        let us = (s.0.max(0) as u64 / 1_000).min(MAX_US);
        self.counts[Self::index(us)] += 1;
        self.total += 1;
        self.sum_us += us;
        self.min_us = self.min_us.min(us);
        self.max_us = self.max_us.max(us);
    }
    pub fn count(&self) -> u64 {
        self.total
    }
    pub fn max(&self) -> Option<Span> {
        (self.total > 0).then(|| Span::from_micros(self.max_us as i64))
    }
    pub fn min(&self) -> Option<Span> {
        (self.total > 0).then(|| Span::from_micros(self.min_us as i64))
    }
    pub fn mean(&self) -> Option<Span> {
        (self.total > 0).then(|| Span::from_micros((self.sum_us / self.total) as i64))
    }
    /// The value at quantile `q` (0..=1), within the bucket resolution, clamped to the recorded min/max.
    pub fn quantile(&self, q: f64) -> Option<Span> {
        if self.total == 0 {
            return None;
        }
        let rank = ((q.clamp(0.0, 1.0) * self.total as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (i, &c) in self.counts.iter().enumerate() {
            seen += c as u64;
            if seen >= rank {
                let v = Self::value(i).clamp(self.min_us, self.max_us);
                return Some(Span::from_micros(v as i64));
            }
        }
        self.max()
    }
    pub fn merge(&mut self, o: &Histogram) {
        for (a, b) in self.counts.iter_mut().zip(&o.counts) {
            *a += b;
        }
        self.total += o.total;
        self.sum_us += o.sum_us;
        self.min_us = self.min_us.min(o.min_us);
        self.max_us = self.max_us.max(o.max_us);
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A histogram over the recent past: two slots of `window` each; [`Rolling::snapshot`] merges the
/// current and the previous slot, so it covers between one and two windows.
#[derive(Clone, Debug)]
pub struct Rolling {
    window: Span,
    slot_start: Time,
    current: Histogram,
    previous: Histogram,
}

impl Rolling {
    pub fn new(window: Span) -> Self {
        Self {
            window,
            slot_start: Time::ZERO,
            current: Histogram::new(),
            previous: Histogram::new(),
        }
    }
    fn rotate(&mut self, now: Time) {
        if now - self.slot_start >= self.window + self.window {
            self.previous.clear();
            self.current.clear();
            self.slot_start = now;
        } else if now - self.slot_start >= self.window {
            self.previous = std::mem::take(&mut self.current);
            self.slot_start = self.slot_start + self.window;
        }
    }
    pub fn record(&mut self, now: Time, s: Span) {
        self.rotate(now);
        self.current.record(s);
    }
    pub fn snapshot(&mut self, now: Time) -> Histogram {
        self.rotate(now);
        let mut h = self.previous.clone();
        h.merge(&self.current);
        h
    }
}

/// Per-window frame counters (`scheduler-latency.md` §5.3; the compositor-side ones come with its feedback).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounters {
    pub presented: u64,
    /// Presentation times the compositor estimated (no OS feedback).
    pub estimated: u64,
    /// UI frames committed after their deadline.
    pub ui_missed: u64,
    /// UI frames run at event time (discrete input).
    pub urgent: u64,
    /// `BeginFrame`s that produced no commit.
    pub no_damage: u64,
    /// Resize handshakes that timed out waiting for `ResizeDone`.
    pub resize_timeouts: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Track {
    Ui = 1,
    Compositor = 2,
    Gpu = 3,
}

impl Track {
    const fn name(self) -> &'static str {
        match self {
            Track::Ui => "UI",
            Track::Compositor => "compositor",
            Track::Gpu => "GPU",
        }
    }
}

/// A slice on one thread's track (UI phases, compositor frames). Slices on one track must nest.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceSlice {
    pub track: Track,
    pub name: &'static str,
    pub start: Time,
    pub dur: Span,
    pub window: WindowId,
}

#[derive(Clone, Copy, Debug)]
pub struct LatencyConfig {
    /// Records not terminated after this long are dropped as [`DropReason::Timeout`].
    pub stale_after: Span,
    /// Terminated records kept for the trace and for queries.
    pub keep_records: usize,
    /// Slices kept for the trace.
    pub keep_slices: usize,
    /// The "recent" histogram window.
    pub recent: Span,
}

impl Default for LatencyConfig {
    fn default() -> Self {
        Self {
            stale_after: Span::from_millis(2_000),
            keep_records: 4_096,
            keep_slices: 16_384,
            recent: Span::from_millis(10_000),
        }
    }
}

/// A key of the histogram table.
pub type HistKey = (Segment, InputClass, WindowId);

/// A broken invariant: a seq terminated twice, or stamped after it terminated, or unknown.
#[derive(Clone, Debug, PartialEq)]
pub enum Violation {
    TerminatedTwice {
        seq: InputSeq,
        first: Termination,
        second: Termination,
    },
    StampAfterEnd {
        seq: InputSeq,
        stage: Stage,
    },
    Unknown {
        seq: InputSeq,
    },
    DuplicateBegin {
        seq: InputSeq,
    },
}

/// The UI thread's latency recorder.
pub struct LatencyRecorder {
    cfg: LatencyConfig,
    live: BTreeMap<InputSeq, LatencyRecord>,
    done: VecDeque<LatencyRecord>,
    /// Every seq that terminated (kept as ranges would be leaner; a set is exact and bounded by the ring
    /// trimming below).
    ended: HashMap<InputSeq, Termination>,
    session: HashMap<HistKey, Histogram>,
    recent: HashMap<HistKey, Rolling>,
    counters: HashMap<WindowId, FrameCounters>,
    slices: VecDeque<TraceSlice>,
    violations: Vec<Violation>,
    terminated: u64,
}

impl Default for LatencyRecorder {
    fn default() -> Self {
        Self::new(LatencyConfig::default())
    }
}

impl LatencyRecorder {
    pub fn new(cfg: LatencyConfig) -> Self {
        Self {
            cfg,
            live: BTreeMap::new(),
            done: VecDeque::new(),
            ended: HashMap::new(),
            session: HashMap::new(),
            recent: HashMap::new(),
            counters: HashMap::new(),
            slices: VecDeque::new(),
            violations: Vec::new(),
            terminated: 0,
        }
    }

    /// Open a record for a platform input (stamps `os` and `recv`).
    pub fn begin(&mut self, ev: &InputEnvelope) {
        if self.live.contains_key(&ev.seq) || self.ended.contains_key(&ev.seq) {
            self.violations
                .push(Violation::DuplicateBegin { seq: ev.seq });
            return;
        }
        let mut r = LatencyRecord::new(ev.seq, ev.window, InputClass::of(&ev.event));
        r.stamp(Stage::Os, ev.os_time);
        r.stamp(Stage::Recv, ev.recv_time);
        self.live.insert(ev.seq, r);
    }

    pub fn stamp(&mut self, seq: InputSeq, stage: Stage, t: Time) {
        match self.live.get_mut(&seq) {
            Some(r) => r.stamp(stage, t),
            None if self.ended.contains_key(&seq) => self
                .violations
                .push(Violation::StampAfterEnd { seq, stage }),
            None => self.violations.push(Violation::Unknown { seq }),
        }
    }

    pub fn stamp_all(&mut self, seqs: &[InputSeq], stage: Stage, t: Time) {
        for &s in seqs {
            self.stamp(s, stage, t);
        }
    }

    /// Stamp `stage` on every live record of `window` that has reached `after` but not `stage`
    /// (e.g. `UiFrameStart` on everything handled and waiting for a frame).
    pub fn stamp_pending(
        &mut self,
        window: WindowId,
        after: Stage,
        stage: Stage,
        t: Time,
    ) -> Vec<InputSeq> {
        let mut out = Vec::new();
        for r in self.live.values_mut() {
            if r.window == window && r.at(after).is_some() && r.at(stage).is_none() {
                r.stamp(stage, t);
                out.push(r.seq);
            }
        }
        out
    }

    /// A commit left the UI thread carrying every handled input of `window` that a frame started for.
    pub fn commit_sent(&mut self, window: WindowId, revision: Revision, t: Time) -> Vec<InputSeq> {
        let mut out = Vec::new();
        for r in self.live.values_mut() {
            if r.window == window && r.at(Stage::UiFrameStart).is_some() && r.revision.is_none() {
                r.revision = Some(revision);
                r.stamp(Stage::Commit, t);
                out.push(r.seq);
            }
        }
        out
    }

    /// A fast commit carried this input to the compositor without a UI frame.
    pub fn fast_sent(&mut self, seq: InputSeq, revision: Revision, t: Time) {
        if let Some(r) = self.live.get_mut(&seq) {
            r.revision.get_or_insert(revision);
            r.stamp(Stage::Commit, t);
            // The compositor applies fast commits on arrival.
            r.stamp(Stage::CommitApplied, t);
        } else {
            self.stamp(seq, Stage::Commit, t);
        }
    }

    /// Inputs of `window` that a UI frame ran for but that no commit carried: nothing visible changed.
    pub fn frame_without_commit(&mut self, window: WindowId) {
        let seqs: Vec<_> = self
            .live
            .values()
            .filter(|r| {
                r.window == window && r.at(Stage::UiFrameStart).is_some() && r.revision.is_none()
            })
            .map(|r| r.seq)
            .collect();
        for s in seqs {
            self.terminate(s, Termination::NoVisualChange);
        }
    }

    pub fn commit_applied(&mut self, window: WindowId, revision: Revision, t: Time) {
        for r in self.live.values_mut() {
            if r.window == window && r.revision.is_some_and(|rv| rv <= revision) {
                r.stamp(Stage::CommitApplied, t);
            }
        }
    }

    /// Compositor-side stage times of one frame (draw, submit, gpu_done, present), applied to every
    /// committed, unterminated input of the window whose commit the frame can contain.
    ///
    /// The compositor sends them after `Presented`, so records that frame already presented are
    /// amended too, and the segments the new stamps complete go into the histograms.
    pub fn frame_stages(&mut self, window: WindowId, frame: FrameId, stages: &[(Stage, Time)]) {
        for r in self.live.values_mut() {
            if r.window == window
                && r.at(Stage::CommitApplied).is_some()
                && r.frame.is_none_or(|f| f == frame)
            {
                r.frame = Some(frame);
                for &(s, t) in stages {
                    r.stamp(s, t);
                }
            }
        }
        let recent = self.cfg.recent;
        for r in self.done.iter_mut().rev() {
            if r.window != window
                || r.frame != Some(frame)
                || r.end != Some(Termination::Presented(frame))
            {
                continue;
            }
            let before: Vec<bool> = Segment::ALL
                .iter()
                .map(|seg| seg.span(r).is_some())
                .collect();
            for &(s, t) in stages {
                r.stamp(s, t);
            }
            let at = r.last_stamp().unwrap_or(Time::ZERO);
            for (seg, had) in Segment::ALL.iter().zip(before) {
                if had {
                    continue;
                }
                if let Some(s) = seg.span(r) {
                    let key = (*seg, r.class, r.window);
                    self.session.entry(key).or_default().record(s);
                    self.recent
                        .entry(key)
                        .or_insert_with(|| Rolling::new(recent))
                        .record(at, s);
                }
            }
        }
    }

    /// A frame reached the screen. `inputs` are the seqs the compositor names; every committed input of
    /// the window up to the newest of them is presented by this frame (commits fold newer over older).
    pub fn presented(
        &mut self,
        window: WindowId,
        frame: FrameId,
        at: Time,
        estimated: bool,
        inputs: &[InputSeq],
    ) {
        let c = self.counters.entry(window).or_default();
        c.presented += 1;
        if estimated {
            c.estimated += 1;
        }
        let Some(&newest) = inputs.iter().max() else {
            return;
        };
        let seqs: Vec<_> = self
            .live
            .range(..=newest)
            .filter(|(_, r)| r.window == window && r.revision.is_some())
            .map(|(s, _)| *s)
            .collect();
        for s in seqs {
            if let Some(r) = self.live.get_mut(&s) {
                r.stamp(Stage::Presented, at);
                r.frame = Some(frame);
                r.estimated = estimated;
            }
            self.terminate_at(s, Termination::Presented(frame), at);
        }
    }

    pub fn merged(&mut self, seq: InputSeq, into: InputSeq) {
        self.terminate(seq, Termination::MergedInto(into));
    }

    pub fn no_visual_change(&mut self, seq: InputSeq) {
        self.terminate(seq, Termination::NoVisualChange);
    }

    /// Drop every live record of `window` (hidden, closed).
    pub fn drop_window(&mut self, window: WindowId) {
        let seqs: Vec<_> = self
            .live
            .values()
            .filter(|r| r.window == window)
            .map(|r| r.seq)
            .collect();
        for s in seqs {
            self.terminate(s, Termination::Dropped(DropReason::WindowGone));
        }
    }

    /// Drop records older than [`LatencyConfig::stale_after`] (measured from `recv`).
    pub fn sweep(&mut self, now: Time) {
        let stale = self.cfg.stale_after;
        let seqs: Vec<_> = self
            .live
            .values()
            .filter(|r| r.at(Stage::Recv).is_some_and(|t| now - t > stale))
            .map(|r| r.seq)
            .collect();
        for s in seqs {
            self.terminate(s, Termination::Dropped(DropReason::Timeout));
        }
    }

    fn terminate(&mut self, seq: InputSeq, end: Termination) {
        let at = self
            .live
            .get(&seq)
            .and_then(LatencyRecord::last_stamp)
            .unwrap_or(Time::ZERO);
        self.terminate_at(seq, end, at);
    }

    fn terminate_at(&mut self, seq: InputSeq, end: Termination, at: Time) {
        if let Some(&first) = self.ended.get(&seq) {
            self.violations.push(Violation::TerminatedTwice {
                seq,
                first,
                second: end,
            });
            return;
        }
        let Some(mut r) = self.live.remove(&seq) else {
            self.violations.push(Violation::Unknown { seq });
            return;
        };
        r.end = Some(end);
        self.ended.insert(seq, end);
        self.terminated += 1;
        if matches!(end, Termination::Presented(_)) {
            for seg in Segment::ALL {
                if let Some(s) = seg.span(&r) {
                    let key = (seg, r.class, r.window);
                    self.session.entry(key).or_default().record(s);
                    let window = self.cfg.recent;
                    self.recent
                        .entry(key)
                        .or_insert_with(|| Rolling::new(window))
                        .record(at, s);
                }
            }
        }
        self.done.push_back(r);
        while self.done.len() > self.cfg.keep_records {
            if let Some(old) = self.done.pop_front() {
                // Keep `ended` bounded: a seq that old cannot be stamped again by a correct pipeline.
                self.ended.remove(&old.seq);
            }
        }
    }

    pub fn counters_mut(&mut self, window: WindowId) -> &mut FrameCounters {
        self.counters.entry(window).or_default()
    }
    pub fn counters(&self, window: WindowId) -> FrameCounters {
        self.counters.get(&window).copied().unwrap_or_default()
    }

    /// Record a phase slice for the trace.
    pub fn slice(
        &mut self,
        track: Track,
        name: &'static str,
        window: WindowId,
        start: Time,
        end: Time,
    ) {
        self.slices.push_back(TraceSlice {
            track,
            name,
            start,
            dur: end - start,
            window,
        });
        while self.slices.len() > self.cfg.keep_slices {
            self.slices.pop_front();
        }
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }
    pub fn terminated_count(&self) -> u64 {
        self.terminated
    }
    pub fn record(&self, seq: InputSeq) -> Option<&LatencyRecord> {
        self.live
            .get(&seq)
            .or_else(|| self.done.iter().rev().find(|r| r.seq == seq))
    }
    pub fn termination(&self, seq: InputSeq) -> Option<Termination> {
        self.ended.get(&seq).copied()
    }
    pub fn finished(&self) -> impl Iterator<Item = &LatencyRecord> {
        self.done.iter()
    }
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
    pub fn histogram(&self, key: HistKey) -> Option<&Histogram> {
        self.session.get(&key)
    }
    pub fn recent_histogram(&mut self, key: HistKey, now: Time) -> Option<Histogram> {
        self.recent.get_mut(&key).map(|r| r.snapshot(now))
    }

    /// p50/p95/p99/max per key over the session, as JSON (the `rpc`/MCP snapshot surface).
    pub fn snapshot_json(&self) -> String {
        let mut keys: Vec<_> = self.session.keys().copied().collect();
        keys.sort();
        let mut out = String::from("{\"histograms\":[");
        for (i, k) in keys.iter().enumerate() {
            let h = &self.session[k];
            let ms = |s: Option<Span>| s.map_or(0.0, Span::as_millis_f64);
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"segment\":\"{}\",\"class\":\"{}\",\"window\":{},\"count\":{},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"p99_ms\":{:.3},\"max_ms\":{:.3}}}",
                k.0.name(),
                k.1.name(),
                k.2.0,
                h.count(),
                ms(h.quantile(0.5)),
                ms(h.quantile(0.95)),
                ms(h.quantile(0.99)),
                ms(h.max()),
            );
        }
        out.push_str("],\"windows\":[");
        let mut windows: Vec<_> = self.counters.keys().copied().collect();
        windows.sort();
        for (i, w) in windows.iter().enumerate() {
            let c = self.counters[w];
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"window\":{},\"presented\":{},\"estimated\":{},\"ui_missed\":{},\"urgent\":{},\"no_damage\":{},\"resize_timeouts\":{}}}",
                w.0,
                c.presented,
                c.estimated,
                c.ui_missed,
                c.urgent,
                c.no_damage,
                c.resize_timeouts
            );
        }
        let _ = write!(
            out,
            "],\"live\":{},\"terminated\":{},\"violations\":{}}}",
            self.live.len(),
            self.terminated,
            self.violations.len()
        );
        out
    }

    /// The Chrome JSON trace (Perfetto opens it): a track per thread with the recorded slices; per
    /// finished input a zero-length slice at each stage on its thread's track, joined by a flow (`s`,
    /// `t`, `f`); and an async track per input with its segments.
    pub fn chrome_trace(&self) -> String {
        let mut ev = Vec::new();
        for t in [Track::Ui, Track::Compositor, Track::Gpu] {
            ev.push(format!(
                "{{\"ph\":\"M\",\"pid\":1,\"tid\":{},\"name\":\"thread_name\",\"args\":{{\"name\":\"{}\"}}}}",
                t as u32,
                t.name()
            ));
        }
        for s in &self.slices {
            ev.push(format!(
                "{{\"ph\":\"X\",\"pid\":1,\"tid\":{},\"name\":\"{}\",\"cat\":\"phase\",\"ts\":{},\"dur\":{},\"args\":{{\"window\":{}}}}}",
                s.track as u32,
                s.name,
                us(s.start),
                span_us(s.dur),
                s.window.0
            ));
        }
        for r in &self.done {
            let stamped: Vec<(Stage, Time)> = Stage::ALL
                .iter()
                .filter_map(|&s| r.at(s).map(|t| (s, t)))
                .collect();
            let end = match r.end {
                Some(Termination::Presented(f)) => format!("presented frame {}", f.0),
                Some(Termination::MergedInto(s)) => format!("merged into {}", s.0),
                Some(Termination::NoVisualChange) => "no visual change".to_owned(),
                Some(Termination::Dropped(d)) => format!("dropped ({d:?})"),
                None => "open".to_owned(),
            };
            let name = format!("{} #{}", r.class.name(), r.seq.0);
            for (i, &(stage, t)) in stamped.iter().enumerate() {
                let tid = stage.thread() as u32;
                ev.push(format!(
                    "{{\"ph\":\"X\",\"pid\":1,\"tid\":{tid},\"name\":\"{}\",\"cat\":\"input\",\"ts\":{},\"dur\":0,\"args\":{{\"seq\":{},\"window\":{}}}}}",
                    stage.name(),
                    us(t),
                    r.seq.0,
                    r.window.0
                ));
                if stamped.len() > 1 {
                    let ph = if i == 0 {
                        "s"
                    } else if i + 1 == stamped.len() {
                        "f"
                    } else {
                        "t"
                    };
                    let bp = if ph == "s" { "" } else { ",\"bp\":\"e\"" };
                    ev.push(format!(
                        "{{\"ph\":\"{ph}\",\"pid\":1,\"tid\":{tid},\"name\":\"{}\",\"cat\":\"input\",\"id\":{},\"ts\":{}{bp}}}",
                        escape(&name),
                        r.seq.0,
                        us(t)
                    ));
                }
            }
            if let (Some(&(_, first)), Some(&(_, last))) = (stamped.first(), stamped.last()) {
                ev.push(format!(
                    "{{\"ph\":\"b\",\"pid\":1,\"name\":\"{}\",\"cat\":\"latency\",\"id\":{},\"ts\":{},\"args\":{{\"end\":\"{}\"}}}}",
                    escape(&name),
                    r.seq.0,
                    us(first),
                    escape(&end)
                ));
                // Segments between consecutive stamped stages nest inside the input's async slice.
                for w in stamped.windows(2) {
                    let (a, ta) = w[0];
                    let (b, tb) = w[1];
                    let seg = format!("{}→{}", a.name(), b.name());
                    ev.push(format!(
                        "{{\"ph\":\"b\",\"pid\":1,\"name\":\"{seg}\",\"cat\":\"latency\",\"id\":{},\"ts\":{}}}",
                        r.seq.0,
                        us(ta)
                    ));
                    ev.push(format!(
                        "{{\"ph\":\"e\",\"pid\":1,\"name\":\"{seg}\",\"cat\":\"latency\",\"id\":{},\"ts\":{}}}",
                        r.seq.0,
                        us(tb)
                    ));
                }
                ev.push(format!(
                    "{{\"ph\":\"e\",\"pid\":1,\"name\":\"{}\",\"cat\":\"latency\",\"id\":{},\"ts\":{}}}",
                    escape(&name),
                    r.seq.0,
                    us(last)
                ));
            }
        }
        let mut out = String::from("{\"displayTimeUnit\":\"ms\",\"traceEvents\":[\n");
        out.push_str(&ev.join(",\n"));
        out.push_str("\n]}\n");
        out
    }
}

/// Microseconds with three decimals (the trace format's `ts` unit).
fn us(t: Time) -> String {
    format!("{}.{:03}", t.0 / 1_000, t.0 % 1_000)
}

fn span_us(s: Span) -> String {
    let n = s.0.max(0) as u64;
    format!("{}.{:03}", n / 1_000, n % 1_000)
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}
