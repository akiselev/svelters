//! The deterministic scheduler simulator (`round2/scheduler-latency.md` §6.2): a discrete-event loop
//! over a seeded priority queue that drives real [`FrameClock`]s and [`UiFrameState`]s against
//! modelled displays, a present engine, GPU, compositor-CPU and UI cost distributions, input
//! streams, timer error and visibility, with fault injection. It checks the scheduler's invariants
//! after every event and measures input latency in frames, so CI asserts frames under virtual time
//! and never wall-clock milliseconds.
//!
//! Invariants checked (§6.2, numbering kept):
//! 1. at most one present in flight per window and one draw per target vblank;
//! 2. the UI never starts a frame while a commit is unapplied;
//! 3. a quiescent window is unsubscribed with no timer armed (one tick of hysteresis);
//! 4. every input reaches exactly one terminal state (presented or dropped while hidden);
//! 5. targets strictly increase, no draw targets a vblank whose latch has passed, shown times
//!    strictly increase;
//! 7. deadline modes are consistent with the configuration and visibility;
//! 8. estimates stay within `[floor, 2 × interval]`.
//!
//! Invariant 6 (revisions of native-owned properties) belongs to presto-core's commit and is not
//! modelled here.
//!
//! Reproduce a failure with `PRESTO_SEED=<n>` (and `PRESTO_TRACE=1` for the timeline);
//! `PRESTO_ITERATIONS=<k>` widens [`seeds`].

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::ops::Range;

use presto_base::ids::{FrameId, InputSeq, Revision};
use presto_base::time::{Span, Time};

use crate::clock::{Action, ClockConfig, ClockStats, DeadlineMode, FrameClock, Signal};
use crate::estimate::FLOOR;
use crate::ui::{UiFrameState, UiPhase, Urgency};

/// The hand-off delay between threads. Every UI → compositor message takes it, so they stay in
/// order (the clock relies on a commit arriving before the dirt announced after it).
const HANDOFF: Span = Span::from_micros(50);

/// SplitMix64: small, seedable, and the same on every platform.
#[derive(Clone, Debug)]
pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { s: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.s = self.s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        p > 0.0 && self.unit() < p
    }

    /// Standard normal (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-300);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Exponential with the given mean.
    pub fn exp(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.unit()).max(1e-300).ln()
    }
}

/// A cost distribution: lognormal around `median` with shape `sigma`, plus rare spikes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub median: Span,
    pub sigma: f64,
    pub spike_p: f64,
    pub spike: Span,
}

impl Cost {
    pub const fn fixed(s: Span) -> Self {
        Cost { median: s, sigma: 0.0, spike_p: 0.0, spike: Span::ZERO }
    }

    pub const fn lognormal(median: Span, sigma: f64) -> Self {
        Cost { median, sigma, spike_p: 0.0, spike: Span::ZERO }
    }

    pub fn sample(&self, rng: &mut Rng) -> Span {
        let base = self.median.0 as f64 * (self.sigma * rng.normal()).exp();
        let spike = if rng.chance(self.spike_p) { self.spike.0 as f64 } else { 0.0 };
        Span((base + spike).max(0.0) as i64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DisplayMode {
    Fixed,
    Variable { min_interval: Span },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayCfg {
    pub interval: Span,
    /// The first vblank.
    pub phase: Span,
    /// σ of how late the pacing callback runs after its vblank.
    pub tick_jitter: Span,
    /// The GPU must finish this long before a vblank to make it.
    pub latch: Span,
    /// Extra vblanks a composed (non-zero-copy) path adds.
    pub composed_frames: u32,
    pub feedback_delay: Span,
    pub mode: DisplayMode,
}

impl DisplayCfg {
    pub fn hz(hz: f64) -> Self {
        DisplayCfg {
            interval: Span::from_secs_f64(1.0 / hz),
            phase: Span::from_micros(1_234),
            tick_jitter: Span::from_micros(100),
            latch: Span::from_micros(500),
            composed_frames: 0,
            feedback_delay: Span::from_micros(500),
            mode: DisplayMode::Fixed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputCfg {
    /// Poisson rate of discrete input (keys, clicks), per second.
    pub discrete_hz: f64,
    /// Scripted discrete events.
    pub script: Vec<Time>,
    /// Continuous streams (pointer moves): `(from, to, rate in Hz)`.
    pub streams: Vec<(Time, Time, f64)>,
    /// OS → UI thread delivery delay.
    pub recv_delay: Span,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerCfg {
    /// Wake error is uniform in `[0, slack]` (Linux's default timer slack is 50 µs).
    pub slack: Span,
    pub late_p: f64,
    pub late: Span,
}

impl Default for TimerCfg {
    fn default() -> Self {
        TimerCfg { slack: Span::from_micros(50), late_p: 0.0, late: Span::ZERO }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    /// The first UI frame starting at or after `at` costs `cost` (a slow JS handler).
    SlowUiFrame { window: usize, at: Time, cost: Span },
    /// UI frames starting in `[from, to)` follow `cost` (sustained overrun).
    SlowUi { from: Time, to: Time, cost: Cost },
    /// The first GPU job starting at or after `at` costs `cost`.
    GpuSpike { at: Time, cost: Span },
    /// Feedback for a present is lost with probability `p`.
    DropFeedback { p: f64 },
    /// A present is discarded with probability `p` (not shown; `Discarded` is reported).
    Discard { p: f64 },
    /// The platform never reports buffer availability (only feedback releases a frame).
    NoBufferRelease,
    /// The window moves to an output with another refresh interval.
    OutputChange { window: usize, at: Time, interval: Span },
    Hide { window: usize, at: Time, until: Time },
    /// The device is lost: estimates reset and the next `failed_draws` draws fail.
    DeviceLost { window: usize, at: Time, failed_draws: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowCfg {
    pub display: DisplayCfg,
    pub clock: ClockConfig,
    /// Intervals in which compositor animations run.
    pub comp_anim: Vec<(Time, Time)>,
    /// Intervals in which UI-side animations (rAF, paint/layout tracks) dirty every frame.
    pub ui_anim: Vec<(Time, Time)>,
    pub input: InputCfg,
}

impl WindowCfg {
    pub fn hz(hz: f64) -> Self {
        let display = DisplayCfg::hz(hz);
        let clock = ClockConfig { interval: display.interval, latch: display.latch, ..ClockConfig::default() };
        WindowCfg { display, clock, comp_anim: Vec::new(), ui_anim: Vec::new(), input: InputCfg { recv_delay: Span::from_micros(300), ..InputCfg::default() } }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    pub duration: Span,
    pub windows: Vec<WindowCfg>,
    /// UI frames without discrete input.
    pub ui: Cost,
    /// UI frames that handle discrete input.
    pub ui_input: Cost,
    pub comp_cpu: Cost,
    pub gpu: Cost,
    pub timer: TimerCfg,
    pub faults: Vec<Fault>,
    /// Keep a text timeline of every signal and action.
    pub trace: bool,
}

impl SimConfig {
    /// One 60 Hz window with costs well inside budget.
    pub fn single(seed: u64) -> Self {
        SimConfig {
            seed,
            duration: Span::from_millis(3_000),
            windows: vec![WindowCfg::hz(60.0)],
            ui: Cost::lognormal(Span::from_micros(1_500), 0.3),
            ui_input: Cost::lognormal(Span::from_micros(2_000), 0.3),
            comp_cpu: Cost::lognormal(Span::from_micros(500), 0.2),
            gpu: Cost::lognormal(Span::from_micros(1_000), 0.3),
            timer: TimerCfg::default(),
            faults: Vec::new(),
            trace: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputRecord {
    pub seq: InputSeq,
    pub window: usize,
    pub at: Time,
    pub presented: Option<Time>,
    /// Vblanks between the first vblank after the input and the one that showed it (0 = the next).
    pub frames: Option<u32>,
    pub dropped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    pub at: Time,
    pub window: usize,
    pub invariant: u8,
    pub what: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowReport {
    pub stats: ClockStats,
    pub shown: u64,
    pub discarded: u64,
    pub ticks_delivered: u64,
    pub timer_wakeups: u64,
    /// Timer wakeups while the window had nothing to do.
    pub idle_wakeups: u64,
    pub failed_draws_injected: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimReport {
    pub seed: u64,
    pub inputs: Vec<InputRecord>,
    pub violations: Vec<Violation>,
    pub windows: Vec<WindowReport>,
    pub timeline: Vec<String>,
}

impl SimReport {
    /// The `p`-quantile (nearest rank) of input latency in frames over presented inputs.
    pub fn frames_percentile(&self, p: f64) -> Option<u32> {
        let mut v: Vec<u32> = self.inputs.iter().filter_map(|i| i.frames).collect();
        if v.is_empty() {
            return None;
        }
        v.sort_unstable();
        let rank = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len());
        Some(v[rank - 1])
    }

    /// Panics with the seed, the violations and the timeline's tail.
    pub fn assert_ok(&self) {
        if self.violations.is_empty() {
            return;
        }
        let tail: Vec<&String> = self.timeline.iter().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
        panic!(
            "Failing Seed: {} (rerun with PRESTO_SEED={})\n{:#?}\ntimeline tail:\n{}",
            self.seed,
            self.seed,
            &self.violations[..self.violations.len().min(10)],
            tail.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
    }
}

/// The seeds a property test runs: `PRESTO_SEED` alone, or `0..PRESTO_ITERATIONS` (default
/// `0..default`).
pub fn seeds(default: u64) -> Range<u64> {
    if let Some(s) = std::env::var("PRESTO_SEED").ok().and_then(|s| s.parse().ok()) {
        return s..s + 1;
    }
    let n = std::env::var("PRESTO_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(default);
    0..n
}

#[derive(Clone, Debug)]
enum Ev {
    VBlank { w: usize, n: u64, generation: u32 },
    Tick { w: usize, target: Time, interval: Span, generation: u32 },
    Timer { w: usize, armed: Time },
    Comp { w: usize, sig: Signal },
    UiBegin { w: usize, frame: FrameId, target: Time, deadline: Time },
    UiInput { w: usize, input: usize },
    UiCommit { w: usize },
    UiApplied { w: usize, revision: Revision },
    Shown { w: usize, frame: FrameId },
    CompAnim { w: usize, on: bool },
    UiAnim { w: usize, on: bool },
    HighRate { w: usize },
    Visible { w: usize, visible: bool },
    Output { w: usize, interval: Span },
    DeviceLost { w: usize, failed_draws: u32 },
}

struct Drawn {
    target: Time,
    inputs: Vec<usize>,
}

struct Win {
    cfg: WindowCfg,
    clock: FrameClock,
    ui: UiFrameState,
    subscribed: bool,
    armed: Option<Time>,
    visible: bool,
    interval: Span,
    epoch: Time,
    generation: u32,
    pending_inputs: Vec<usize>,
    running_inputs: Vec<usize>,
    running_cost: Span,
    commits: HashMap<Revision, Vec<usize>>,
    unapplied: Option<Revision>,
    staged: Vec<usize>,
    carry: Vec<usize>,
    drawn: HashMap<FrameId, Drawn>,
    last_target: Option<Time>,
    last_shown: Option<Time>,
    quiet_ticks: u32,
    ui_anim_on: bool,
    fail_draws: u32,
    slow_frame: Option<(Time, Span)>,
    report: WindowReport,
    max_interval: Span,
}

struct Sim {
    cfg: SimConfig,
    rng: Rng,
    now: Time,
    seq: u64,
    queue: BinaryHeap<Reverse<(Time, u64, usize)>>,
    events: Vec<Option<Ev>>,
    windows: Vec<Win>,
    inputs: Vec<InputRecord>,
    violations: Vec<Violation>,
    timeline: Vec<String>,
    comp_busy: Time,
    ui_busy: Time,
    gpu_free: Time,
    gpu_spike: Option<(Time, Span)>,
    end: Time,
}

/// Runs one simulation.
pub fn run(cfg: &SimConfig) -> SimReport {
    let mut cfg = cfg.clone();
    cfg.trace |= std::env::var_os("PRESTO_TRACE").is_some();
    let mut sim = Sim::new(cfg);
    sim.schedule_initial();
    while let Some(Reverse((t, _, idx))) = sim.queue.pop() {
        if t > sim.end {
            break;
        }
        sim.now = t;
        if let Some(ev) = sim.events[idx].take() {
            sim.handle(ev);
        }
    }
    sim.finish()
}

impl Sim {
    fn new(cfg: SimConfig) -> Self {
        let start = Time(1_000_000_000);
        let windows = cfg
            .windows
            .iter()
            .map(|w| Win {
                clock: FrameClock::new(w.clock.clone()),
                ui: UiFrameState::new(),
                subscribed: false,
                armed: None,
                visible: true,
                interval: w.display.interval,
                epoch: start + w.display.phase,
                generation: 0,
                pending_inputs: Vec::new(),
                running_inputs: Vec::new(),
                running_cost: Span::ZERO,
                commits: HashMap::new(),
                unapplied: None,
                staged: Vec::new(),
                carry: Vec::new(),
                drawn: HashMap::new(),
                last_target: None,
                last_shown: None,
                quiet_ticks: 0,
                ui_anim_on: false,
                fail_draws: 0,
                slow_frame: None,
                report: WindowReport::default(),
                max_interval: w.display.interval,
                cfg: w.clone(),
            })
            .collect();
        Sim {
            rng: Rng::new(cfg.seed),
            now: start,
            seq: 0,
            queue: BinaryHeap::new(),
            events: Vec::new(),
            windows,
            inputs: Vec::new(),
            violations: Vec::new(),
            timeline: Vec::new(),
            comp_busy: start,
            ui_busy: start,
            gpu_free: start,
            gpu_spike: None,
            end: start + cfg.duration,
            cfg,
        }
    }

    fn at(&mut self, t: Time, ev: Ev) {
        self.seq += 1;
        self.events.push(Some(ev));
        self.queue.push(Reverse((t, self.seq, self.events.len() - 1)));
    }

    fn violate(&mut self, w: usize, invariant: u8, what: String) {
        if self.cfg.trace {
            self.timeline.push(format!("!! inv{invariant} w{w}: {what}"));
        }
        self.violations.push(Violation { at: self.now, window: w, invariant, what });
    }

    fn schedule_initial(&mut self) {
        let start = self.now;
        for w in 0..self.windows.len() {
            let epoch = self.windows[w].epoch;
            self.at(epoch, Ev::VBlank { w, n: 0, generation: 0 });
            let cfg = self.windows[w].cfg.clone();
            for (a, b) in &cfg.comp_anim {
                self.at(*a, Ev::CompAnim { w, on: true });
                self.at(*b, Ev::CompAnim { w, on: false });
            }
            for (a, b) in &cfg.ui_anim {
                self.at(*a, Ev::UiAnim { w, on: true });
                self.at(*b, Ev::UiAnim { w, on: false });
            }
            let mut inputs: Vec<Time> = cfg.input.script.clone();
            if cfg.input.discrete_hz > 0.0 {
                let mut t = start;
                loop {
                    t = t + Span::from_secs_f64(self.rng.exp(1.0 / cfg.input.discrete_hz));
                    if t >= self.end {
                        break;
                    }
                    inputs.push(t);
                }
            }
            inputs.sort();
            for t in inputs {
                let idx = self.inputs.len();
                self.inputs.push(InputRecord { seq: InputSeq(idx as u64 + 1), window: w, at: t, presented: None, frames: None, dropped: false });
                self.at(t + cfg.input.recv_delay, Ev::UiInput { w, input: idx });
            }
            for (a, b, hz) in &cfg.input.streams {
                let step = Span::from_secs_f64(1.0 / hz.max(1.0));
                let mut t = *a;
                while t < *b {
                    self.at(t, Ev::HighRate { w });
                    t = t + step;
                }
            }
        }
        for f in self.cfg.faults.clone() {
            match f {
                Fault::OutputChange { window, at, interval } => self.at(at, Ev::Output { w: window, interval }),
                Fault::Hide { window, at, until } => {
                    self.at(at, Ev::Visible { w: window, visible: false });
                    self.at(until, Ev::Visible { w: window, visible: true });
                }
                Fault::DeviceLost { window, at, failed_draws } => self.at(at, Ev::DeviceLost { w: window, failed_draws }),
                Fault::SlowUiFrame { window, at, cost } => self.windows[window].slow_frame = Some((at, cost)),
                Fault::GpuSpike { at, cost } => self.gpu_spike = Some((at, cost)),
                Fault::SlowUi { .. } | Fault::DropFeedback { .. } | Fault::Discard { .. } | Fault::NoBufferRelease => {}
            }
        }
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::VBlank { w, n, generation } => {
                let win = &self.windows[w];
                if generation != win.generation {
                    return;
                }
                let interval = win.interval;
                let vblank = win.epoch + Span(interval.0 * n as i64);
                let jitter = Span((self.rng.normal().abs() * win.cfg.display.tick_jitter.0 as f64) as i64);
                let target = vblank + interval;
                self.at(vblank + jitter, Ev::Tick { w, target, interval, generation });
                self.at(vblank + interval, Ev::VBlank { w, n: n + 1, generation });
            }
            Ev::Tick { w, target, interval, generation } => {
                let win = &self.windows[w];
                if generation != win.generation || !win.visible || !win.subscribed {
                    return;
                }
                self.deliver(w, Signal::Vsync { target, interval });
            }
            Ev::Timer { w, armed } => {
                if self.windows[w].armed == Some(armed) {
                    self.windows[w].report.timer_wakeups += 1;
                    if self.windows[w].clock.is_quiescent() {
                        self.windows[w].report.idle_wakeups += 1;
                    }
                    self.deliver(w, Signal::Timer);
                }
            }
            Ev::Comp { w, sig } => self.deliver(w, sig),
            Ev::UiBegin { w, frame, target, deadline } => {
                if self.now < self.ui_busy {
                    let t = self.ui_busy;
                    self.at(t, Ev::UiBegin { w, frame, target, deadline });
                    return;
                }
                // A BeginUiFrame can cross an urgent commit in flight; the UI must refuse it then.
                let now = self.now;
                if self.windows[w].ui.on_begin_frame(now, frame, target, deadline) {
                    if self.windows[w].unapplied.is_some() {
                        self.violate(w, 2, format!("UI started {frame:?} with commit {:?} unapplied", self.windows[w].unapplied));
                    }
                    self.start_ui_frame(w, false);
                } else if self.windows[w].ui.phase() == UiPhase::Idle {
                    self.at(now + HANDOFF, Ev::Comp { w, sig: Signal::UiFrameSkipped { frame } });
                }
            }
            Ev::UiInput { w, input } => {
                if self.now < self.ui_busy {
                    let t = self.ui_busy;
                    self.at(t, Ev::UiInput { w, input });
                    return;
                }
                let now = self.now;
                if !self.windows[w].visible {
                    self.inputs[input].dropped = true;
                    return;
                }
                self.windows[w].pending_inputs.push(input);
                let notify = self.windows[w].ui.mark_dirty();
                match self.windows[w].ui.on_urgent_input(now) {
                    Urgency::RunNow { target } => {
                        if self.windows[w].unapplied.is_some() {
                            self.violate(w, 2, "urgent UI frame with a commit unapplied".into());
                        }
                        if self.windows[w].ui.start_urgent(now, target) {
                            self.start_ui_frame(w, true);
                        }
                    }
                    Urgency::Wait => {
                        if notify {
                            self.at(now + HANDOFF, Ev::Comp { w, sig: Signal::NeedsUiFrame(true) });
                        }
                        self.at(now + HANDOFF, Ev::Comp { w, sig: Signal::UrgentInput });
                    }
                }
            }
            Ev::UiCommit { w } => {
                let now = self.now;
                let win = &mut self.windows[w];
                let Some(report) = win.ui.commit_sent(now) else { return };
                let inputs = std::mem::take(&mut win.running_inputs);
                win.commits.insert(report.revision, inputs);
                win.unapplied = Some(report.revision);
                let frame = report.frame.unwrap_or(FrameId(0));
                let cost = if report.input {
                    Signal::UiInputCost { frame, cost: report.cost }
                } else {
                    Signal::UiCost { frame, cost: report.cost }
                };
                self.at(now + HANDOFF, Ev::Comp { w, sig: cost });
                self.at(now + HANDOFF, Ev::Comp { w, sig: Signal::CommitArrived { revision: report.revision, frame: report.frame } });
            }
            Ev::UiApplied { w, revision } => {
                let now = self.now;
                let win = &mut self.windows[w];
                let mut notify = win.ui.commit_applied(revision);
                if win.ui_anim_on {
                    notify |= win.ui.mark_dirty();
                }
                if notify {
                    self.at(now + HANDOFF, Ev::Comp { w, sig: Signal::NeedsUiFrame(true) });
                }
            }
            Ev::Shown { w, frame } => self.on_shown(w, frame),
            Ev::CompAnim { w, on } => self.deliver(w, Signal::AnimationsActive(on)),
            Ev::UiAnim { w, on } => {
                self.windows[w].ui_anim_on = on;
                if on && self.windows[w].ui.mark_dirty() {
                    let t = self.now + HANDOFF;
                    self.at(t, Ev::Comp { w, sig: Signal::NeedsUiFrame(true) });
                }
            }
            Ev::HighRate { w } => self.deliver(w, Signal::HighRateInput),
            Ev::Visible { w, visible } => {
                self.windows[w].visible = visible;
                self.deliver(w, Signal::Visible(visible));
            }
            Ev::Output { w, interval } => {
                let now = self.now;
                let win = &mut self.windows[w];
                win.generation += 1;
                win.interval = interval;
                win.max_interval = win.max_interval.max(interval);
                win.epoch = now + interval;
                let generation = win.generation;
                let epoch = win.epoch;
                self.at(epoch, Ev::VBlank { w, n: 0, generation });
                self.deliver(w, Signal::OutputChanged { interval, vrr_min_interval: None });
            }
            Ev::DeviceLost { w, failed_draws } => {
                self.windows[w].fail_draws = failed_draws;
                self.deliver(w, Signal::DeviceLost);
            }
        }
    }

    fn start_ui_frame(&mut self, w: usize, urgent: bool) {
        let now = self.now;
        let input = urgent || !self.windows[w].pending_inputs.is_empty();
        let mut cost = if input { self.cfg.ui_input.sample(&mut self.rng) } else { self.cfg.ui.sample(&mut self.rng) };
        for f in &self.cfg.faults {
            if let Fault::SlowUi { from, to, cost: c } = f
                && now >= *from
                && now < *to
            {
                cost = c.sample(&mut self.rng);
            }
        }
        if let Some((at, c)) = self.windows[w].slow_frame
            && now >= at
        {
            cost = c;
            self.windows[w].slow_frame = None;
        }
        let win = &mut self.windows[w];
        win.running_inputs = std::mem::take(&mut win.pending_inputs);
        win.running_cost = cost;
        self.ui_busy = now + cost;
        self.at(now + cost, Ev::UiCommit { w });
    }

    /// Delivers a signal to the compositor thread (later if it is busy drawing).
    fn deliver(&mut self, w: usize, sig: Signal) {
        if self.now < self.comp_busy {
            let t = self.comp_busy;
            self.at(t, Ev::Comp { w, sig });
            return;
        }
        let now = self.now;
        let prev = self.windows[w].clock.in_flight();
        let timeouts = self.windows[w].clock.stats().feedback_timeouts;
        let actions = self.windows[w].clock.on_signal(now, sig);
        let timed_out = self.windows[w].clock.stats().feedback_timeouts > timeouts;
        let args = self.windows[w].clock.frame_args();
        self.windows[w].ui.set_args(args);
        if self.cfg.trace {
            self.timeline.push(format!("{:>10.3}ms w{w} {sig:?} -> {actions:?} mode={:?}", (now.0 as f64) * 1e-6, self.windows[w].clock.deadline_mode()));
        }
        let draws = actions.iter().filter(|a| matches!(a, Action::Draw { .. })).count();
        if draws > 1 {
            self.violate(w, 1, format!("{draws} draws for one signal"));
        }
        if draws == 1 && prev.is_some() {
            let released = match sig {
                Signal::Presented { frame, .. }
                | Signal::Discarded { frame }
                | Signal::DrawFailed { frame }
                | Signal::BufferAvailable { frame } => Some(frame) == prev || timed_out,
                _ => timed_out,
            };
            if !released {
                self.violate(w, 1, format!("draw while {prev:?} is in flight ({sig:?})"));
            }
        }
        for a in actions {
            self.apply(w, a);
        }
        self.check_state(w);
        if let Signal::Vsync { .. } = sig {
            self.windows[w].report.ticks_delivered += 1;
            self.check_quiet(w);
        }
    }

    fn apply(&mut self, w: usize, a: Action) {
        let now = self.now;
        match a {
            Action::SubscribeVsync(on) => self.windows[w].subscribed = on,
            Action::BeginUiFrame { frame, target, deadline, .. } => {
                self.at(now, Ev::UiBegin { w, frame, target, deadline });
            }
            Action::ApplyCommit { revision } => {
                let win = &mut self.windows[w];
                if let Some(inputs) = win.commits.remove(&revision) {
                    win.staged.extend(inputs);
                }
                if win.unapplied == Some(revision) {
                    win.unapplied = None;
                }
                self.at(now + HANDOFF, Ev::UiApplied { w, revision });
            }
            Action::ForceFullRedraw | Action::Represent { .. } => {}
            Action::Draw { frame, target } => self.draw(w, frame, target),
            Action::SetTimer(t) => {
                self.windows[w].armed = Some(t);
                let mut err = Span((self.rng.unit() * self.cfg.timer.slack.0 as f64) as i64);
                if self.rng.chance(self.cfg.timer.late_p) {
                    err = err + self.cfg.timer.late;
                }
                self.at(t.max(now) + err, Ev::Timer { w, armed: t });
            }
            Action::ClearTimer => self.windows[w].armed = None,
        }
    }

    fn draw(&mut self, w: usize, frame: FrameId, target: Time) {
        let now = self.now;
        let blocked = self.windows[w].cfg.clock.blocked;
        if let Some(last) = self.windows[w].last_target
            && target <= last
        {
            self.violate(w, 5, format!("draw for {target:?} after {last:?}"));
        }
        if !blocked && target < now + self.windows[w].cfg.clock.latch {
            self.violate(w, 5, format!("draw at {now:?} targets {target:?}, inside its latch"));
        }
        self.windows[w].last_target = Some(target);
        let cpu = self.cfg.comp_cpu.sample(&mut self.rng);
        let submit = now + cpu;
        self.comp_busy = submit;
        let mut inputs = std::mem::take(&mut self.windows[w].staged);
        inputs.append(&mut self.windows[w].carry);
        if self.windows[w].fail_draws > 0 {
            self.windows[w].fail_draws -= 1;
            self.windows[w].report.failed_draws_injected += 1;
            self.windows[w].carry = inputs;
            self.at(submit, Ev::Comp { w, sig: Signal::DrawFailed { frame } });
            return;
        }
        self.at(submit, Ev::Comp { w, sig: Signal::Submitted { frame, at: submit } });
        let start = submit.max(self.gpu_free);
        let mut gpu = self.cfg.gpu.sample(&mut self.rng);
        if let Some((at, c)) = self.gpu_spike
            && start >= at
        {
            gpu = c;
            self.gpu_spike = None;
        }
        let done = start + gpu;
        self.gpu_free = done;
        self.at(done, Ev::Comp { w, sig: Signal::GpuDone { frame, at: done } });
        let discard_p = self.cfg.faults.iter().find_map(|f| if let Fault::Discard { p } = f { Some(*p) } else { None }).unwrap_or(0.0);
        let drop_p = self.cfg.faults.iter().find_map(|f| if let Fault::DropFeedback { p } = f { Some(*p) } else { None }).unwrap_or(0.0);
        if self.rng.chance(discard_p) {
            self.windows[w].carry = inputs;
            self.windows[w].report.discarded += 1;
            let at = done + self.windows[w].cfg.display.feedback_delay;
            self.at(at, Ev::Comp { w, sig: Signal::Discarded { frame } });
            return;
        }
        let win = &self.windows[w];
        let d = &win.cfg.display;
        let shown = match d.mode {
            DisplayMode::Fixed => {
                let ready = done + d.latch;
                let k = if ready <= win.epoch { 0 } else { ((ready - win.epoch).0 + win.interval.0 - 1) / win.interval.0 };
                let mut v = win.epoch + Span(k * win.interval.0) + Span(win.interval.0 * d.composed_frames as i64);
                if let Some(last) = win.last_shown
                    && v <= last
                {
                    v = last + win.interval;
                }
                v
            }
            DisplayMode::Variable { min_interval } => {
                let ready = done + d.latch;
                match win.last_shown {
                    Some(last) => ready.max(last + min_interval),
                    None => ready,
                }
            }
        };
        let feedback = shown + d.feedback_delay;
        self.windows[w].last_shown = Some(shown);
        self.windows[w].drawn.insert(frame, Drawn { target, inputs });
        self.at(shown, Ev::Shown { w, frame });
        // the swapchain gets the buffer back once the frame is latched, feedback or not
        if !self.cfg.faults.contains(&Fault::NoBufferRelease) {
            self.at(shown, Ev::Comp { w, sig: Signal::BufferAvailable { frame } });
        }
        if !self.rng.chance(drop_p) {
            self.at(feedback, Ev::Comp { w, sig: Signal::Presented { frame, at: shown, estimated: false } });
        }
    }

    fn on_shown(&mut self, w: usize, frame: FrameId) {
        let now = self.now;
        let Some(d) = self.windows[w].drawn.remove(&frame) else { return };
        // a frame shown before its target frees the vblanks up to it
        if d.target > now && self.windows[w].last_target == Some(d.target) {
            self.windows[w].last_target = Some(now);
        }
        self.windows[w].report.shown += 1;
        let (epoch, interval) = (self.windows[w].epoch, self.windows[w].interval);
        for i in d.inputs {
            let rec = &mut self.inputs[i];
            if rec.presented.is_some() || rec.dropped {
                let seq = rec.seq;
                self.violate(w, 4, format!("{seq:?} reached a second terminal state"));
                continue;
            }
            rec.presented = Some(now);
            // first vblank after the input on the current grid
            let first = if rec.at < epoch {
                epoch
            } else {
                epoch + Span(((rec.at - epoch).0 / interval.0 + 1) * interval.0)
            };
            let frames = if now <= first { 0 } else { (((now - first).0 as f64) / interval.0 as f64).round() as u32 };
            rec.frames = Some(frames);
        }
    }

    /// Invariants that hold after every signal.
    fn check_state(&mut self, w: usize) {
        let win = &self.windows[w];
        let mode = win.clock.deadline_mode();
        let blocked = win.cfg.clock.blocked;
        let mut bad: Vec<(u8, String)> = Vec::new();
        if mode == DeadlineMode::Blocked && !blocked {
            bad.push((7, "Blocked mode outside headless mode".into()));
        }
        if !win.visible && win.subscribed {
            bad.push((7, "hidden window subscribed to vsync".into()));
        }
        let cap = Span(win.max_interval.0 * 2);
        let est = win.clock.estimator();
        for (name, e) in [("ui", est.est_ui()), ("ui_input", est.est_ui_input()), ("comp_cpu", est.comp_cpu.estimate()), ("gpu", est.gpu.estimate())] {
            if e < FLOOR || e > cap.max(FLOOR) {
                bad.push((8, format!("estimate {name} = {e:?} outside [floor, 2 × interval]")));
            }
        }
        for (i, what) in bad {
            self.violate(w, i, what);
        }
    }

    /// Invariant 3, checked when a tick was delivered.
    fn check_quiet(&mut self, w: usize) {
        let now = self.now;
        let win = &mut self.windows[w];
        let quiet = win.clock.is_quiescent() && !win.ui_anim_on;
        if quiet && (win.subscribed || win.clock.next_wakeup().is_some()) {
            win.quiet_ticks += 1;
            if win.quiet_ticks >= 2 {
                let msg = format!("quiescent but subscribed={} wakeup={:?} at {now:?}", win.subscribed, win.clock.next_wakeup());
                if win.cfg.clock.vrr_min_interval.is_none() {
                    self.violate(w, 3, msg);
                }
            }
        } else {
            win.quiet_ticks = 0;
        }
    }

    fn finish(mut self) -> SimReport {
        let grace = Span::from_millis(250);
        for i in 0..self.inputs.len() {
            let rec = &self.inputs[i];
            if rec.presented.is_some() || rec.dropped {
                continue;
            }
            let w = rec.window;
            if !self.windows[w].visible {
                self.inputs[i].dropped = true;
            } else if rec.at + grace < self.end {
                let seq = rec.seq;
                let at = rec.at;
                self.now = self.end;
                self.violate(w, 4, format!("{seq:?} at {at:?} never presented"));
            }
        }
        SimReport {
            seed: self.cfg.seed,
            inputs: self.inputs,
            violations: self.violations,
            windows: self
                .windows
                .into_iter()
                .map(|w| WindowReport { stats: *w.clock.stats(), ..w.report })
                .collect(),
            timeline: self.timeline,
        }
    }
}
