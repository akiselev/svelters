//! The UI-thread frame state (`round2/scheduler-latency.md` §2.4–§2.5): when the UI may run its
//! phases, the urgent-input rule, and back-pressure (never start while the last commit is unapplied).

use presto_base::ids::{FrameId, Revision};
use presto_base::time::{Span, Time};

use crate::clock::FrameArgs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiPhase {
    Idle,
    Running { target: Time, frame: Option<FrameId>, started: Time, deadline: Time, input: bool },
    Committed { revision: Revision, target: Time },
}

/// The answer to discrete input that dirtied the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    /// Run the UI phases now for `target`.
    RunNow { target: Time },
    /// Leave it to the next `BeginUiFrame`; later events in this interval batch into it.
    Wait,
}

/// What the UI tells the compositor after committing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitReport {
    pub revision: Revision,
    /// The `BeginUiFrame` answered; `None` for an urgent frame.
    pub frame: Option<FrameId>,
    pub target: Time,
    pub cost: Span,
    /// The frame handled discrete input (its cost feeds `est_ui_input`).
    pub input: bool,
    /// Committed after `D_ui`: it lands a frame later.
    pub late: bool,
}

#[derive(Clone, Debug)]
pub struct UiFrameState {
    phase: UiPhase,
    args: Option<FrameArgs>,
    dirty: bool,
    requested: bool,
    revision: u64,
    pending_input: bool,
    pub late_commits: u64,
}

impl UiFrameState {
    pub fn new() -> Self {
        UiFrameState {
            phase: UiPhase::Idle,
            args: None,
            dirty: false,
            requested: false,
            revision: 0,
            pending_input: false,
            late_commits: 0,
        }
    }

    pub fn phase(&self) -> UiPhase {
        self.phase
    }

    /// The compositor's latest published snapshot.
    pub fn set_args(&mut self, args: FrameArgs) {
        self.args = Some(args);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The scene became dirty. `true` when the compositor must be told (`NeedsUiFrame(true)`): it
    /// is a level, sent once until a commit consumes it.
    pub fn mark_dirty(&mut self) -> bool {
        self.dirty = true;
        if self.requested {
            return false;
        }
        self.requested = true;
        true
    }

    /// Discrete input dirtied the scene at `now`: run now when the frame for the next target is
    /// still reachable (`now + est_ui_input ≤ D_ui`), or when the compositor is idle and nothing
    /// was presented for two intervals (Mutter `should_update_now`).
    pub fn on_urgent_input(&mut self, now: Time) -> Urgency {
        self.pending_input = true;
        self.mark_dirty();
        if self.phase != UiPhase::Idle {
            return Urgency::Wait;
        }
        let Some(args) = self.args else {
            return Urgency::RunNow { target: now };
        };
        if !args.visible {
            return Urgency::Wait;
        }
        let quiet = args.last_present.is_none_or(|p| now - p >= Span(args.interval.0 * 2));
        if args.idle && quiet {
            return Urgency::RunNow { target: args.next_target.max(now) };
        }
        if now + args.est_ui_input <= args.d_ui {
            return Urgency::RunNow { target: args.next_target };
        }
        Urgency::Wait
    }

    /// Starts the urgent frame [`Self::on_urgent_input`] allowed.
    pub fn start_urgent(&mut self, now: Time, target: Time) -> bool {
        if self.phase != UiPhase::Idle {
            return false;
        }
        let deadline = self.args.map_or(target, |a| if a.next_target == target { a.d_ui } else { target });
        self.phase = UiPhase::Running { target, frame: None, started: now, deadline, input: true };
        self.pending_input = false;
        true
    }

    /// `BeginUiFrame` arrived. `true` when the UI runs its phases now; `false` while a frame runs or
    /// the last commit is unapplied (the compositor never sends one then; this is the guard).
    pub fn on_begin_frame(&mut self, now: Time, frame: FrameId, target: Time, deadline: Time) -> bool {
        if self.phase != UiPhase::Idle {
            return false;
        }
        let input = std::mem::take(&mut self.pending_input);
        self.phase = UiPhase::Running { target, frame: Some(frame), started: now, deadline, input };
        true
    }

    /// The running frame committed at `now`.
    pub fn commit_sent(&mut self, now: Time) -> Option<CommitReport> {
        let UiPhase::Running { target, frame, started, deadline, input } = self.phase else {
            return None;
        };
        self.revision += 1;
        let revision = Revision(self.revision);
        let late = now > deadline;
        if late {
            self.late_commits += 1;
        }
        self.phase = UiPhase::Committed { revision, target };
        self.dirty = false;
        self.requested = false;
        Some(CommitReport { revision, frame, target, cost: now - started, input, late })
    }

    /// The running frame had nothing to commit (`UiFrameSkipped`).
    pub fn frame_skipped(&mut self) -> Option<FrameId> {
        let UiPhase::Running { frame, .. } = self.phase else { return None };
        self.phase = UiPhase::Idle;
        self.dirty = false;
        self.requested = false;
        frame
    }

    /// The compositor applied `revision`. If the scene got dirty meanwhile, the caller sends
    /// `NeedsUiFrame(true)` again (the return value).
    pub fn commit_applied(&mut self, revision: Revision) -> bool {
        if matches!(self.phase, UiPhase::Committed { revision: r, .. } if r == revision) {
            self.phase = UiPhase::Idle;
            if self.dirty && !self.requested {
                self.requested = true;
                return true;
            }
        }
        false
    }
}

impl Default for UiFrameState {
    fn default() -> Self {
        UiFrameState::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(next: u64, d_ui: u64, idle: bool) -> FrameArgs {
        FrameArgs {
            next_target: Time(next),
            d_ui: Time(d_ui),
            interval: Span::from_millis(16),
            est_ui_input: Span::from_millis(2),
            idle,
            last_present: Some(Time(0)),
            visible: true,
        }
    }

    #[test]
    fn urgent_when_reachable() {
        let mut u = UiFrameState::new();
        u.set_args(args(20_000_000, 10_000_000, false));
        assert_eq!(u.on_urgent_input(Time(8_000_000)), Urgency::RunNow { target: Time(20_000_000) });
        let mut u = UiFrameState::new();
        u.set_args(args(20_000_000, 10_000_000, false));
        assert_eq!(u.on_urgent_input(Time(8_500_000)), Urgency::Wait);
    }

    #[test]
    fn never_starts_while_commit_unapplied() {
        let mut u = UiFrameState::new();
        u.set_args(args(20_000_000, 10_000_000, false));
        assert!(u.on_begin_frame(Time(1), FrameId(1), Time(20_000_000), Time(10_000_000)));
        let r = u.commit_sent(Time(2)).unwrap();
        assert!(!r.late);
        assert!(!u.on_begin_frame(Time(3), FrameId(2), Time(36_000_000), Time(26_000_000)));
        assert_eq!(u.on_urgent_input(Time(3)), Urgency::Wait);
        assert!(u.commit_applied(r.revision), "input during the commit asks for a frame again");
        assert!(u.on_begin_frame(Time(4), FrameId(2), Time(36_000_000), Time(26_000_000)));
    }

    #[test]
    fn dirty_is_a_level() {
        let mut u = UiFrameState::new();
        assert!(u.mark_dirty());
        assert!(!u.mark_dirty());
    }
}
