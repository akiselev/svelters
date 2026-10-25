//! [`IntervalPacer`]: a small frame clock the compositor runs until `presto_frame::FrameClock` replaces
//! it behind [`crate::seams::FramePacer`]. It keeps the rules that matter for correctness (one frame in
//! flight, at most one draw per target, the two-deadline rule, unsubscribing after a quiet tick, no
//! frames while hidden) and leaves estimation out: the UI frame starts at the tick and the compositor
//! deadline is a fixed budget before the target.

use crate::seams::{Action, FramePacer, Signal};
use presto_base::ids::FrameId;
use presto_base::time::{Span, Time};
use smallvec::SmallVec;

/// The compositor's own budget before the target present time (CPU encode, GPU, latch).
pub const COMPOSITOR_BUDGET: Span = Span::from_millis(4);

#[derive(Debug)]
pub struct IntervalPacer {
    interval: Span,
    subscribed: bool,
    hidden: bool,
    ui_wanted: bool,
    ui_sent: bool,
    commit_ready: bool,
    damage: bool,
    animating: bool,
    in_flight: Option<FrameId>,
    frame: u64,
    last_drawn: Option<u64>,
    target: Option<Time>,
    drawn_for: Option<Time>,
    timer: Option<Time>,
}

impl IntervalPacer {
    pub fn new(interval: Span) -> Self {
        IntervalPacer {
            interval,
            subscribed: false,
            hidden: false,
            ui_wanted: false,
            ui_sent: false,
            commit_ready: false,
            damage: false,
            animating: false,
            in_flight: None,
            frame: 0,
            last_drawn: None,
            target: None,
            drawn_for: None,
            timer: None,
        }
    }

    fn wants_draw(&self) -> bool {
        self.damage || self.animating || self.commit_ready
    }

    fn subscribe(&mut self, out: &mut SmallVec<[Action; 4]>) {
        if !self.subscribed && !self.hidden {
            self.subscribed = true;
            out.push(Action::SubscribeVsync(true));
        }
    }

    fn draw(&mut self, target: Time, out: &mut SmallVec<[Action; 4]>) {
        if self.hidden || self.in_flight.is_some() || self.drawn_for == Some(target) {
            return;
        }
        // A draw outside a tick (an urgent commit) gets its own id.
        if self.last_drawn == Some(self.frame) {
            self.frame += 1;
        }
        self.last_drawn = Some(self.frame);
        let frame = FrameId(self.frame);
        self.in_flight = Some(frame);
        self.drawn_for = Some(target);
        self.damage = false;
        self.commit_ready = false;
        self.timer = None;
        out.push(Action::Draw { frame, target });
    }
}

impl FramePacer for IntervalPacer {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]> {
        let mut out = SmallVec::new();
        match sig {
            Signal::Vsync { target, interval } => {
                self.interval = interval;
                if self.hidden {
                    return out;
                }
                self.frame += 1;
                self.target = Some(target);
                let deadline = target - COMPOSITOR_BUDGET;
                if self.ui_wanted && !self.ui_sent && !self.commit_ready {
                    self.ui_wanted = false;
                    self.ui_sent = true;
                    out.push(Action::BeginUiFrame { frame: FrameId(self.frame), target, start_at: now, deadline });
                }
                if self.wants_draw() && self.in_flight.is_none() {
                    if now >= deadline || (self.commit_ready && !self.ui_sent) {
                        self.draw(target, &mut out);
                    } else {
                        self.timer = Some(deadline);
                        out.push(Action::SetTimer(deadline));
                    }
                } else if !self.ui_sent && !self.ui_wanted && !self.wants_draw() && self.in_flight.is_none() {
                    // One quiet tick: stop the clock until something needs a frame.
                    self.subscribed = false;
                    out.push(Action::SubscribeVsync(false));
                }
            }
            Signal::NeedsUiFrame => {
                self.ui_wanted = true;
                self.subscribe(&mut out);
            }
            Signal::CommitArrived { .. } => {
                self.commit_ready = true;
                self.ui_sent = false;
                let target = self.target.filter(|t| *t > now).unwrap_or(now + self.interval);
                if self.drawn_for == Some(target) || self.in_flight.is_some() {
                    self.subscribe(&mut out);
                } else {
                    self.draw(target, &mut out);
                    self.subscribe(&mut out);
                }
            }
            Signal::CompositorDamage => {
                self.damage = true;
                self.subscribe(&mut out);
            }
            Signal::AnimationsActive(on) => {
                self.animating = on;
                if on {
                    self.subscribe(&mut out);
                }
            }
            Signal::Timer => {
                self.timer = None;
                if let Some(target) = self.target {
                    if self.wants_draw() {
                        self.draw(target, &mut out);
                    }
                }
            }
            Signal::GpuDone { frame, .. } | Signal::Presented { frame, .. } | Signal::DidNotDraw { frame } => {
                if self.in_flight == Some(frame) {
                    self.in_flight = None;
                }
                if self.wants_draw() {
                    self.subscribe(&mut out);
                }
            }
            Signal::Visible(v) => {
                self.hidden = !v;
                if v {
                    self.damage = true;
                    self.drawn_for = None;
                    self.subscribe(&mut out);
                } else {
                    self.timer = None;
                    if self.subscribed {
                        self.subscribed = false;
                        out.push(Action::SubscribeVsync(false));
                    }
                }
            }
            Signal::HighRateInput | Signal::UiCost { .. } => {}
        }
        out
    }

    fn next_wakeup(&self) -> Option<Time> {
        self.timer
    }
}
