//! The compositor-side `FrameClock`, one per window (`round2/scheduler-latency.md` §2.3, architecture
//! §10): a pure state machine fed [`Signal`]s with the current time, answering [`Action`]s. It never
//! reads a clock or arms a timer itself; [`FrameClock::next_wakeup`] says when it wants a
//! [`Signal::Timer`].
//!
//! Rules it keeps:
//! - at most one present in flight and one draw per target vblank (Chromium `kMaxPendingSubmitFrames`);
//! - no `BeginUiFrame` while the UI's last commit is unapplied;
//! - two deadlines: the UI's `D_ui` is soft, the compositor's `D_comp` is final, so compositor-side
//!   updates (scroll, animations, caret) present on time when the UI is late;
//! - unsubscribe from vsync after one tick with nothing to do; the first frame after idle, after
//!   becoming visible and after an output change is drawn at once;
//! - under VRR, keep presenting for a second after high-rate input (GPUI `InputRateTracker`);
//! - the UI starts for the earliest target it can still make, which pipelines it (its start falls
//!   in the previous interval) when its cost does not fit one; the pipelined state is reported
//!   with hysteresis (10 frames that fit with 1.5× headroom, Swappy); a forced full redraw after
//!   3 failed draws (Chromium).

use skelter_base::ids::{FrameId, Revision};
use skelter_base::smallvec::SmallVec;
use skelter_base::time::{Span, Time};

use crate::estimate::{Estimator, Schedule};
use crate::rate::{InputRateTracker, RateConfig};

/// Draws that may fail in a row before the clock asks for a full redraw.
pub const MAX_FAILED_DRAWS: u32 = 3;
/// Frames that must fit with headroom before leaving pipelined mode.
pub const PIPELINE_EXIT_FRAMES: u32 = 10;
/// A present with no feedback after this many intervals past its target counts as presented one
/// interval late.
pub const FEEDBACK_TIMEOUT_INTERVALS: i64 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct ClockConfig {
    /// The refresh interval assumed until the first [`Signal::Vsync`].
    pub interval: Span,
    /// `L_latch`: how long before the target vblank the present must be issued. Seeds: 0 on macOS
    /// (`targetTimestamp` is the deadline), 1 ms for independent flip, 3 ms when composed.
    pub latch: Span,
    /// Variable refresh: the shortest interval the panel accepts. `None` for a fixed rate.
    pub vrr_min_interval: Option<Span>,
    /// Headless full-pipeline mode: never draw on a deadline, draw when there is something to draw.
    pub blocked: bool,
    pub rate: RateConfig,
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            interval: Span::from_nanos(16_666_667),
            latch: Span::from_millis(1),
            vrr_min_interval: None,
            blocked: false,
            rate: RateConfig::default(),
        }
    }
}

/// What happens to a window, converted to skelter time at the platform boundary. Signals from the
/// UI thread must arrive in the order the UI sent them (one channel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    /// The display's pacing callback: the next latchable vblank is `target`.
    Vsync { target: Time, interval: Span },
    /// A UI commit is in the mailbox; `frame` is the `BeginUiFrame` it answers (`None` when the UI
    /// started on its own, the urgent path).
    CommitArrived { revision: Revision, frame: Option<FrameId> },
    /// The compositor has damage of its own (fast commit, scroll, caret).
    CompositorDamage,
    /// Compositor animations need frames.
    AnimationsActive(bool),
    /// Feedback: `frame` reached the screen at `at`. `estimated` when the platform guessed the time.
    Presented { frame: FrameId, at: Time, estimated: bool },
    Visible(bool),
    /// A continuous input event (pointer move, touchpad scroll) arrived.
    HighRateInput,
    /// The wakeup from [`FrameClock::next_wakeup`] fired.
    Timer,
    /// The GPU finished `frame` (wgpu `on_submitted_work_done`).
    GpuDone { frame: FrameId, at: Time },
    /// The UI frame answering `frame` cost `cost`.
    UiCost { frame: FrameId, cost: Span },
    /// Like `UiCost`, for a UI frame that handled discrete input.
    UiInputCost { frame: FrameId, cost: Span },
    /// The UI's scene is dirty (a level: `false` once it has nothing more to commit).
    NeedsUiFrame(bool),
    /// Discrete input dirtied the scene and waits for the next UI frame, which then starts with
    /// the input-frame estimate.
    UrgentInput,
    /// The swapchain has `frame`'s buffer back (acquire succeeded, the waitable signalled): the
    /// frame is no longer in flight even if its feedback never comes.
    BufferAvailable { frame: FrameId },
    /// The UI answered `BeginUiFrame` without a commit (nothing changed).
    UiFrameSkipped { frame: FrameId },
    /// The compositor submitted and presented `frame` (end of its CPU work).
    Submitted { frame: FrameId, at: Time },
    /// The platform dropped `frame` without showing it.
    Discarded { frame: FrameId },
    /// The draw of `frame` failed (lost surface, acquire timeout).
    DrawFailed { frame: FrameId },
    /// The window moved to another output or the mode changed.
    OutputChanged { interval: Span, vrr_min_interval: Option<Span> },
    /// The GPU device was lost or the backend changed: all estimates restart from the seeds.
    DeviceLost,
}

/// What the compositor thread must do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    SubscribeVsync(bool),
    /// Tell the UI thread to build the frame for `target`; it must commit by `deadline`.
    BeginUiFrame { frame: FrameId, target: Time, start_at: Time, deadline: Time },
    /// Apply the commit in the mailbox before the next draw (then tell the UI it was applied).
    ApplyCommit { revision: Revision },
    /// Redraw everything (after repeated failed draws).
    ForceFullRedraw,
    /// Draw and present `frame`, sampling animations at `target`.
    Draw { frame: FrameId, target: Time },
    /// Present the last frame again without drawing (VRR keep-alive).
    Represent { target: Time },
    /// Deliver [`Signal::Timer`] at this time.
    SetTimer(Time),
    /// The previously set timer is no longer wanted.
    ClearTimer,
}

/// Chromium's deadline modes, as skelter uses them (§2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DeadlineMode {
    #[default]
    None,
    /// Draw now: a commit is ready, the frame is urgent, or nothing else is expected.
    Immediate,
    /// Draw at `D_comp`, with or without the UI's commit.
    Regular,
    /// Only waiting for the UI: no compositor deadline this tick.
    Late,
    /// Headless full pipeline.
    Blocked,
}

/// Counters for tests and the in-app histograms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockStats {
    pub draws: u64,
    pub commits_applied: u64,
    pub ui_frames: u64,
    pub ui_missed: u64,
    pub comp_missed: u64,
    pub throttled: u64,
    pub no_damage: u64,
    pub late_ticks: u64,
    pub feedback_timeouts: u64,
    pub stale_feedback: u64,
    pub failed_draws: u64,
    pub forced_redraws: u64,
    pub represents: u64,
    pub subscribes: u64,
    pub unsubscribes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tick {
    sched: Schedule,
    drew: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiSide {
    Idle,
    /// A `BeginUiFrame` is planned for `start_at`.
    Pending { frame: FrameId, sched: Schedule, start_at: Time },
    Sent { frame: FrameId, target: Time },
    Ready { revision: Revision, target: Option<Time> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InFlight {
    frame: FrameId,
    target: Time,
    draw_at: Time,
    submitted: Option<Time>,
    latch_used: Span,
    timeout_at: Time,
}

/// The values the UI thread reads without a round trip (published by the compositor after every
/// signal; an atomic snapshot in the real app).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameArgs {
    pub next_target: Time,
    /// `D_ui(next_target)`.
    pub d_ui: Time,
    pub interval: Span,
    pub est_ui_input: Span,
    /// The clock is unsubscribed (nothing drawn recently).
    pub idle: bool,
    pub last_present: Option<Time>,
    pub visible: bool,
}

pub struct FrameClock {
    cfg: ClockConfig,
    est: Estimator,
    rate: InputRateTracker,
    interval: Span,
    vrr_min: Option<Span>,
    subscribed: bool,
    visible: bool,
    last_vsync: Option<Time>,
    tick: Option<Tick>,
    ui: UiSide,
    ui_wanted: bool,
    ui_input: bool,
    needs_draw: bool,
    animating: bool,
    in_flight: Option<InFlight>,
    last_released: Option<InFlight>,
    /// Frames released by buffer availability whose feedback has not arrived yet (newest last).
    awaiting_feedback: SmallVec<[InFlight; 2]>,
    next_frame: u64,
    last_drawn_target: Option<Time>,
    last_present: Option<Time>,
    deadline_at: Option<Time>,
    armed: Option<Time>,
    urgent_next: bool,
    pipelined: bool,
    fit_streak: u32,
    failed_draws: u32,
    mode: DeadlineMode,
    stats: ClockStats,
}

type Actions = SmallVec<[Action; 4]>;

impl FrameClock {
    pub fn new(cfg: ClockConfig) -> Self {
        FrameClock {
            rate: InputRateTracker::new(cfg.rate),
            interval: cfg.interval,
            vrr_min: cfg.vrr_min_interval,
            est: Estimator::new(),
            subscribed: false,
            visible: true,
            last_vsync: None,
            tick: None,
            ui: UiSide::Idle,
            ui_wanted: false,
            ui_input: false,
            needs_draw: false,
            animating: false,
            in_flight: None,
            last_released: None,
            awaiting_feedback: SmallVec::new(),
            next_frame: 1,
            last_drawn_target: None,
            last_present: None,
            deadline_at: None,
            armed: None,
            urgent_next: true,
            pipelined: false,
            fit_streak: 0,
            failed_draws: 0,
            mode: DeadlineMode::None,
            stats: ClockStats::default(),
            cfg,
        }
    }

    pub fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]> {
        let mut out = Actions::new();
        self.fire_due(now, &mut out);
        match sig {
            Signal::Vsync { target, interval } => self.on_vsync(now, target, interval, &mut out),
            Signal::CommitArrived { revision, frame } => self.on_commit(now, revision, frame, &mut out),
            Signal::CompositorDamage => {
                self.needs_draw = true;
                self.on_compositor_work(now, &mut out);
            }
            Signal::AnimationsActive(on) => {
                self.animating = on;
                if on {
                    self.ensure_subscribed(&mut out);
                }
            }
            Signal::Presented { frame, at, estimated } => self.on_presented(now, frame, Some(at), estimated, &mut out),
            Signal::Discarded { frame } => self.on_presented(now, frame, None, true, &mut out),
            Signal::Visible(v) => self.on_visible(now, v, &mut out),
            Signal::HighRateInput => {
                self.rate.record(now);
                if self.vrr_min.is_some() && self.rate.is_high_rate(now) {
                    self.ensure_subscribed(&mut out);
                }
            }
            Signal::Timer => {
                if let Some(armed) = self.armed
                    && armed <= now
                {
                    self.est.wake_error.record(now - armed, self.interval);
                }
            }
            Signal::GpuDone { frame, at } => {
                if let Some(f) = self.find_frame(frame) {
                    let from = f.submitted.unwrap_or(f.draw_at);
                    self.est.gpu.record(at - from, self.interval);
                }
            }
            Signal::Submitted { frame, at } => {
                if let Some(f) = self.in_flight.as_mut().filter(|f| f.frame == frame) {
                    f.submitted = Some(at);
                    let cost = at - f.draw_at;
                    self.est.comp_cpu.record(cost, self.interval);
                }
            }
            Signal::UiCost { cost, .. } => self.est.ui_other.record(cost, self.interval),
            Signal::UiInputCost { cost, .. } => self.est.ui_input.record(cost, self.interval),
            Signal::NeedsUiFrame(on) => {
                self.ui_wanted = on;
                if on {
                    self.on_ui_wanted(now, &mut out);
                }
            }
            Signal::UrgentInput => self.ui_input = true,
            Signal::BufferAvailable { frame } => {
                if let Some(f) = self.in_flight.filter(|f| f.frame == frame) {
                    if self.awaiting_feedback.len() == 4 {
                        self.awaiting_feedback.remove(0);
                    }
                    self.awaiting_feedback.push(f);
                    self.release(now, frame, &mut out);
                }
            }
            Signal::UiFrameSkipped { frame } => {
                if matches!(self.ui, UiSide::Sent { frame: f, .. } if f == frame) {
                    self.ui = UiSide::Idle;
                }
            }
            Signal::DrawFailed { frame } => {
                if self.in_flight.is_some_and(|f| f.frame == frame) {
                    self.in_flight = None;
                    self.needs_draw = true;
                    self.failed_draws += 1;
                    self.stats.failed_draws += 1;
                }
            }
            Signal::OutputChanged { interval, vrr_min_interval } => {
                self.interval = interval;
                self.vrr_min = vrr_min_interval;
                self.last_vsync = None;
                self.tick = None;
                self.deadline_at = None;
                self.urgent_next = true;
                self.needs_draw = true;
                self.ensure_subscribed(&mut out);
            }
            Signal::DeviceLost => {
                self.est.reset();
                self.needs_draw = true;
            }
        }
        self.sync_timer(&mut out);
        out
    }

    /// The earliest time this clock wants a [`Signal::Timer`].
    pub fn next_wakeup(&self) -> Option<Time> {
        let begin = match self.ui {
            UiSide::Pending { start_at, .. } => Some(start_at),
            _ => None,
        };
        [begin, self.deadline_at, self.in_flight.map(|f| f.timeout_at)].into_iter().flatten().min()
    }

    pub fn frame_args(&self) -> FrameArgs {
        let sched = match self.tick {
            Some(t) if !t.drew => t.sched,
            _ => {
                // the first vblank after the last one drawn (or announced)
                let base = self.last_vsync.unwrap_or(Time::ZERO);
                let next = match self.last_drawn_target {
                    Some(d) if d >= base => d + self.interval,
                    _ => base,
                };
                self.est.schedule(next, self.cfg.latch)
            }
        };
        FrameArgs {
            next_target: sched.target,
            d_ui: sched.d_ui,
            interval: self.interval,
            est_ui_input: self.est.est_ui_input(),
            idle: !self.subscribed,
            last_present: self.last_present,
            visible: self.visible,
        }
    }

    pub fn deadline_mode(&self) -> DeadlineMode {
        self.mode
    }

    pub fn stats(&self) -> &ClockStats {
        &self.stats
    }

    pub fn estimator(&self) -> &Estimator {
        &self.est
    }

    pub fn is_subscribed(&self) -> bool {
        self.subscribed
    }

    pub fn in_flight(&self) -> Option<FrameId> {
        self.in_flight.map(|f| f.frame)
    }

    pub fn is_pipelined(&self) -> bool {
        self.pipelined
    }

    /// A commit is waiting to be applied.
    pub fn has_unapplied_commit(&self) -> bool {
        matches!(self.ui, UiSide::Ready { .. })
    }

    /// A `BeginUiFrame` was sent and not answered yet.
    pub fn ui_frame_outstanding(&self) -> bool {
        matches!(self.ui, UiSide::Sent { .. })
    }

    /// Nothing to draw, nothing coming: the state in which the clock must be unsubscribed with no timer.
    pub fn is_quiescent(&self) -> bool {
        !self.needs_draw
            && !self.animating
            && !self.ui_wanted
            && self.ui == UiSide::Idle
            && self.in_flight.is_none()
    }

    fn alloc_frame(&mut self) -> FrameId {
        let f = FrameId(self.next_frame);
        self.next_frame += 1;
        f
    }

    fn find_frame(&self, frame: FrameId) -> Option<InFlight> {
        self.in_flight.filter(|f| f.frame == frame).or(self.last_released.filter(|f| f.frame == frame))
    }

    fn ensure_subscribed(&mut self, out: &mut Actions) {
        if self.visible && !self.subscribed {
            self.subscribed = true;
            self.stats.subscribes += 1;
            out.push(Action::SubscribeVsync(true));
        }
    }

    fn unsubscribe(&mut self, out: &mut Actions) {
        if self.subscribed {
            self.subscribed = false;
            self.stats.unsubscribes += 1;
            out.push(Action::SubscribeVsync(false));
        }
        self.tick = None;
        self.deadline_at = None;
        self.mode = DeadlineMode::None;
    }

    fn sync_timer(&mut self, out: &mut Actions) {
        let want = self.next_wakeup();
        if want != self.armed {
            match want {
                Some(t) => out.push(Action::SetTimer(t)),
                None => out.push(Action::ClearTimer),
            }
            self.armed = want;
        }
    }

    /// The earliest target after `after` (on the vsync grid when known) whose compositor deadline is
    /// still ahead of `now`-ish callers; under VRR the earliest the panel accepts.
    fn predict_target(&self, now: Time) -> Time {
        if let Some(min) = self.vrr_min {
            let ready = now + self.cfg.latch + self.est.margin() + self.est.est_comp();
            return match self.last_present {
                Some(p) => ready.max(p + min),
                None => ready,
            };
        }
        let lead = self.cfg.latch + self.est.margin() + self.est.est_comp();
        match self.last_vsync {
            Some(v) => {
                let earliest = now + lead;
                if earliest <= v {
                    return v;
                }
                let n = ((earliest - v).0 + self.interval.0 - 1) / self.interval.0.max(1);
                v + Span(n * self.interval.0)
            }
            None => now + lead,
        }
    }

    fn fire_due(&mut self, now: Time, out: &mut Actions) {
        if let UiSide::Pending { frame, sched, start_at } = self.ui
            && start_at <= now
        {
            self.send_begin(frame, sched, now, out);
        }
        if self.deadline_at.is_some_and(|d| d <= now) {
            self.deadline_at = None;
            self.attempt_draw(now, out);
        }
        if let Some(f) = self.in_flight
            && f.timeout_at <= now
        {
            self.stats.feedback_timeouts += 1;
            self.on_presented(now, f.frame, Some(f.target + self.interval), true, out);
        }
    }

    fn send_begin(&mut self, frame: FrameId, sched: Schedule, now: Time, out: &mut Actions) {
        self.ui = UiSide::Sent { frame, target: sched.target };
        self.stats.ui_frames += 1;
        out.push(Action::BeginUiFrame { frame, target: sched.target, start_at: now, deadline: sched.d_ui });
    }

    fn on_vsync(&mut self, now: Time, target: Time, interval: Span, out: &mut Actions) {
        self.interval = interval;
        self.last_vsync = Some(target);
        if !self.visible || !self.subscribed {
            return;
        }
        if self.cfg.blocked {
            self.mode = DeadlineMode::Blocked;
            self.tick = Some(Tick { sched: self.est.schedule(target, self.cfg.latch), drew: false });
            self.plan_ui(now, out);
            if self.has_work() || self.ready_for(target) {
                self.attempt_draw(now, out);
            } else if self.ui == UiSide::Idle && !self.ui_wanted {
                self.unsubscribe(out);
            }
            return;
        }
        // A tick that arrives after its own compositor deadline serves the next vblank it can make
        // (Chromium drops it; retargeting keeps a slow compositor from never drawing).
        let mut t = target;
        let mut sched = self.est.schedule(t, self.cfg.latch);
        let mut k = 0;
        while sched.d_comp < now && k < 2 {
            t = t + interval;
            sched = self.est.schedule(t, self.cfg.latch);
            k += 1;
        }
        if k > 0 {
            self.stats.late_ticks += 1;
        }
        if let Some(c) = self.tick
            && c.sched.target == t
        {
            // A tick the urgent or predicted path opened for this target: plan it if it is open.
            if !c.drew && self.deadline_at.is_none() {
                self.plan_ui(now, out);
                self.plan_deadline(now, out);
            }
            return;
        }
        if self.last_drawn_target.is_some_and(|d| d >= t) {
            return;
        }
        self.tick = Some(Tick { sched, drew: false });
        self.deadline_at = None;
        self.update_pipelining(interval);
        self.plan_ui(now, out);
        self.plan_deadline(now, out);
    }

    fn update_pipelining(&mut self, interval: Span) {
        let budget = interval - self.cfg.latch;
        let total = self.est.est_total(self.cfg.latch, interval);
        if total > budget {
            self.pipelined = true;
            self.fit_streak = 0;
        } else if self.pipelined {
            if total.0.saturating_mul(3) / 2 < budget.0 {
                self.fit_streak += 1;
            } else {
                self.fit_streak = 0;
            }
            if self.fit_streak >= PIPELINE_EXIT_FRAMES {
                self.pipelined = false;
                self.fit_streak = 0;
            }
        }
    }

    /// Plans the `BeginUiFrame` for the current tick if the UI wants a frame and may start one.
    fn plan_ui(&mut self, now: Time, out: &mut Actions) {
        if !self.ui_wanted || self.ui != UiSide::Idle || !self.visible {
            return;
        }
        let Some(tick) = self.tick else { return };
        // The earliest target the UI can still make; when its cost does not fit one interval this
        // is a later vblank whose start falls in this interval (pipelined).
        let mut target = tick.sched.target;
        let est_ui = self.next_ui_cost();
        let mut sched = self.est.schedule_with(target, self.cfg.latch, est_ui);
        let mut k = 0;
        while sched.d_ui < now + est_ui && k < 2 {
            target = target + self.interval;
            sched = self.est.schedule_with(target, self.cfg.latch, est_ui);
            k += 1;
        }
        let frame = self.alloc_frame();
        if sched.s_ui <= now {
            self.send_begin(frame, sched, now, out);
        } else {
            self.ui = UiSide::Pending { frame, sched, start_at: sched.s_ui };
        }
    }

    /// The estimate for the next UI frame: the input-frame one when it carries discrete input.
    fn next_ui_cost(&self) -> Span {
        if self.ui_input { self.est.est_ui().max(self.est.est_ui_input()) } else { self.est.est_ui() }
    }

    fn has_work(&self) -> bool {
        self.needs_draw || self.animating
    }

    /// A ready commit may be drawn for `target` (it was built for this target or earlier).
    fn ready_for(&self, target: Time) -> bool {
        match self.ui {
            UiSide::Ready { target: t, .. } => t.is_none_or(|t| t <= target),
            _ => false,
        }
    }

    /// The UI is building (or will build) a frame for `target` or an earlier one.
    fn ui_expected_for(&self, target: Time) -> bool {
        match self.ui {
            UiSide::Pending { sched, .. } => sched.target <= target,
            UiSide::Sent { target: t, .. } => t <= target,
            _ => false,
        }
    }

    fn plan_deadline(&mut self, now: Time, out: &mut Actions) {
        let Some(tick) = self.tick else { return };
        let target = tick.sched.target;
        let high_rate = self.vrr_min.is_some() && self.rate.is_high_rate(now);
        if self.ready_for(target) || (self.urgent_next && self.has_work()) {
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
        } else if self.has_work() && self.ui_expected_for(target) {
            self.mode = DeadlineMode::Regular;
            self.deadline_at = Some(tick.sched.d_comp.max(now));
        } else if self.has_work() {
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
        } else if self.ui != UiSide::Idle {
            self.mode = DeadlineMode::Late;
        } else if high_rate {
            self.mode = DeadlineMode::None;
            self.stats.represents += 1;
            out.push(Action::Represent { target });
        } else if !self.ui_wanted {
            self.mode = DeadlineMode::None;
            self.unsubscribe(out);
        } else {
            self.mode = DeadlineMode::None;
        }
    }

    fn attempt_draw(&mut self, now: Time, out: &mut Actions) {
        let Some(mut tick) = self.tick else { return };
        if tick.drew {
            return;
        }
        // Past this target's deadline by more than the margin (a late commit, a throttled draw
        // released late): draw for the next vblank it can still make, so animations sample the
        // time it shows. Lateness within the margin (timer slack) keeps the target.
        if now > tick.sched.d_comp + self.est.margin() && !self.cfg.blocked {
            let t = self.predict_target(now);
            tick.sched = self.est.schedule(t, self.cfg.latch);
            self.stats.late_ticks += 1;
        }
        let target = tick.sched.target;
        if self.last_drawn_target.is_some_and(|d| d >= target) {
            return;
        }
        let ready = self.ready_for(target);
        if !ready && !self.has_work() {
            self.stats.no_damage += 1;
            return;
        }
        if self.in_flight.is_some() {
            self.stats.throttled += 1;
            return;
        }
        if self.failed_draws >= MAX_FAILED_DRAWS {
            self.failed_draws = 0;
            self.stats.forced_redraws += 1;
            out.push(Action::ForceFullRedraw);
        }
        if let UiSide::Ready { revision, .. } = self.ui
            && ready
        {
            out.push(Action::ApplyCommit { revision });
            self.ui = UiSide::Idle;
            self.stats.commits_applied += 1;
        } else if self.ui_expected_for(target) {
            self.stats.ui_missed += 1;
        }
        let frame = self.alloc_frame();
        out.push(Action::Draw { frame, target });
        self.stats.draws += 1;
        let interval = self.interval;
        self.in_flight = Some(InFlight {
            frame,
            target,
            draw_at: now,
            submitted: None,
            latch_used: self.cfg.latch + self.est.margin(),
            timeout_at: target + Span(interval.0 * FEEDBACK_TIMEOUT_INTERVALS),
        });
        tick.drew = true;
        self.tick = Some(tick);
        self.last_drawn_target = Some(target);
        self.needs_draw = false;
        self.urgent_next = false;
        self.deadline_at = None;
        // A UI frame that wants to run after this commit was applied starts now if it can still
        // make a later target.
        if self.ui == UiSide::Idle && self.ui_wanted {
            self.plan_ui_after_draw(now, out);
        }
    }

    fn plan_ui_after_draw(&mut self, now: Time, out: &mut Actions) {
        let target = self.predict_target(now.max(self.last_drawn_target.unwrap_or(now) + Span(1)));
        let saved = self.tick;
        self.tick = Some(Tick { sched: self.est.schedule(target, self.cfg.latch), drew: false });
        self.plan_ui(now, out);
        self.tick = saved;
    }

    /// No tick in progress and nothing presented for two intervals: the urgent path (Mutter
    /// `should_update_now`): draw at once for the next latchable target.
    /// Also true with no tick open at all (unsubscribed, after an output change): the next
    /// latchable target is predicted from the vsync grid.
    fn idle_for_long(&self, now: Time) -> bool {
        if self.tick.is_none() {
            return true;
        }
        self.tick.is_none_or(|t| t.drew)
            && self.in_flight.is_none()
            && self.last_present.is_none_or(|p| now - p >= Span(self.interval.0 * 2))
    }

    fn start_predicted_tick(&mut self, now: Time) {
        let target = self.predict_target(now);
        if self.last_drawn_target.is_some_and(|d| d >= target) {
            return;
        }
        self.tick = Some(Tick { sched: self.est.schedule(target, self.cfg.latch), drew: false });
    }

    fn on_commit(&mut self, now: Time, revision: Revision, frame: Option<FrameId>, out: &mut Actions) {
        let target = match (self.ui, frame) {
            (UiSide::Sent { frame: f, target }, Some(fr)) if f == fr => Some(target),
            _ => None,
        };
        self.ui = UiSide::Ready { revision, target };
        self.ui_wanted = false;
        self.ui_input = false;
        if !self.visible {
            return;
        }
        self.ensure_subscribed(out);
        if self.cfg.blocked {
            if self.tick.is_none_or(|t| t.drew) {
                self.start_predicted_tick(now);
            }
            self.mode = DeadlineMode::Blocked;
            self.attempt_draw(now, out);
            return;
        }
        if self.vrr_min.is_some() || self.idle_for_long(now) {
            self.start_predicted_tick(now);
        }
        if let Some(t) = self.tick
            && !t.drew
            && self.ready_for(t.sched.target)
        {
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
        }
    }

    fn on_compositor_work(&mut self, now: Time, out: &mut Actions) {
        if !self.visible {
            return;
        }
        self.ensure_subscribed(out);
        if self.cfg.blocked {
            if self.tick.is_none_or(|t| t.drew) {
                self.start_predicted_tick(now);
            }
            self.attempt_draw(now, out);
            return;
        }
        if self.idle_for_long(now) {
            self.start_predicted_tick(now);
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
            return;
        }
        let Some(tick) = self.tick else { return };
        if tick.drew {
            return;
        }
        if self.ui_expected_for(tick.sched.target) {
            if self.deadline_at.is_none() {
                self.mode = DeadlineMode::Regular;
                self.deadline_at = Some(tick.sched.d_comp.max(now));
            }
        } else {
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
        }
    }

    fn on_ui_wanted(&mut self, now: Time, out: &mut Actions) {
        if !self.visible {
            return;
        }
        self.ensure_subscribed(out);
        if self.ui != UiSide::Idle {
            return;
        }
        if self.idle_for_long(now) && !self.cfg.blocked {
            // First frame after idle: start the UI at once for the next latchable target.
            let est_ui = self.next_ui_cost();
            let target = self.predict_target(now + est_ui);
            let sched = self.est.schedule_with(target, self.cfg.latch, est_ui);
            let frame = self.alloc_frame();
            self.send_begin(frame, sched, now, out);
            return;
        }
        match self.tick {
            Some(t) if !t.drew => self.plan_ui(now, out),
            Some(_) => self.plan_ui_after_draw(now, out),
            None => {}
        }
    }

    /// The frame is no longer in flight (feedback, buffer availability, timeout).
    fn release(&mut self, now: Time, frame: FrameId, out: &mut Actions) {
        let Some(f) = self.in_flight.filter(|f| f.frame == frame) else { return };
        self.in_flight = None;
        self.last_released = Some(f);
        self.failed_draws = 0;
        // A draw throttled earlier in this tick, or a tick reopened by an early present, may still
        // make its vblank.
        if let Some(t) = self.tick
            && !t.drew
            && self.deadline_at.is_none()
            && (self.has_work() || self.ready_for(t.sched.target))
        {
            self.plan_deadline(now, out);
        }
    }

    fn on_presented(&mut self, now: Time, frame: FrameId, at: Option<Time>, estimated: bool, out: &mut Actions) {
        // Feedback for the frame in flight, or late feedback for one already released by buffer
        // availability (still worth learning from).
        let f = match self.in_flight {
            Some(f) if f.frame == frame => f,
            _ => match self.awaiting_feedback.iter().position(|f| f.frame == frame) {
                Some(i) => self.awaiting_feedback.remove(i),
                None => {
                    self.stats.stale_feedback += 1;
                    return;
                }
            },
        };
        if let Some(at) = at {
            let missed = at > f.target + Span(self.interval.0 / 2);
            if missed {
                self.stats.comp_missed += 1;
            }
            if !estimated {
                let call = f.submitted.unwrap_or(f.draw_at);
                let slack = (f.target - f.latch_used) - call;
                self.est.margin.update(slack, missed, self.interval);
            }
            self.last_present = Some(self.last_present.map_or(at, |p| p.max(at)));
            // Shown before its target (a conservative prediction): the vblanks up to the target
            // are free again.
            if at < f.target && self.last_drawn_target == Some(f.target) {
                self.last_drawn_target = Some(at);
                if let Some(t) = self.tick.as_mut()
                    && t.drew
                    && t.sched.target == f.target
                {
                    t.drew = false;
                }
            }
        }
        if self.in_flight.is_some_and(|f| f.frame == frame) {
            self.release(now, frame, out);
        } else if let Some(t) = self.tick
            && !t.drew
            && self.deadline_at.is_none()
            && (self.has_work() || self.ready_for(t.sched.target))
        {
            self.plan_deadline(now, out);
        }
    }

    fn on_visible(&mut self, now: Time, visible: bool, out: &mut Actions) {
        if !visible {
            if self.visible {
                self.unsubscribe(out);
                if let UiSide::Pending { .. } = self.ui {
                    self.ui = UiSide::Idle;
                }
                self.visible = false;
            }
            return;
        }
        if self.visible {
            return;
        }
        self.visible = true;
        self.needs_draw = true;
        self.urgent_next = true;
        self.ensure_subscribed(out);
        if self.ui_wanted && self.ui == UiSide::Idle {
            self.on_ui_wanted(now, out);
        }
        if self.ready_for(Time(u64::MAX)) || self.ui == UiSide::Idle {
            self.start_predicted_tick(now);
            self.mode = DeadlineMode::Immediate;
            self.attempt_draw(now, out);
        }
    }
}
