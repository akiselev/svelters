//! The slice of Web Animations' `Animation` that Svelte's transition code uses (`element.animate`,
//! `onfinish`, `cancel`, `currentTime`, `playState`, `effect = null`, `getAnimations`), as skelter
//! plays it: one [`Animation`] per `OP_ANIMATE` id, timed on skelter's clock, with the finish event
//! raised once ([`Animations::poll`]) for the host to deliver as the node's `animationend` signal.
//!
//! Times are milliseconds as in Web Animations (`currentTime` includes the start delay; the
//! animation finishes at `delay + duration + endDelay`). Only the playback rate 1 is used.

use skelter_base::time::Time;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Idle,
    Running,
    Paused,
    Finished,
}

impl PlayState {
    /// The `playState` string.
    pub fn as_str(self) -> &'static str {
        match self {
            PlayState::Idle => "idle",
            PlayState::Running => "running",
            PlayState::Paused => "paused",
            PlayState::Finished => "finished",
        }
    }
}

fn ms(t: Time) -> f64 {
    t.0 as f64 * 1e-6
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub id: u64,
    pub delay: f64,
    pub duration: f64,
    pub end_delay: f64,
    /// Fill forwards: the effect keeps applying after the end (Svelte always asks for it).
    pub fill_forwards: bool,
    /// `start time` on the clock, in ms; `None` while idle or paused.
    start: Option<f64>,
    /// `hold time`: the current time while paused or after `finish()`/a seek without a start.
    hold: Option<f64>,
    /// `effect = null`: the animation no longer applies anything (Svelte sets it after `cancel`).
    has_effect: bool,
    /// Whether the finish event was raised for the current run.
    finish_raised: bool,
    cancelled: bool,
}

impl Animation {
    /// `element.animate(keyframes, { duration, delay, fill })`, playing from `now`.
    pub fn new(id: u64, delay: f64, duration: f64, fill_forwards: bool, now: Time) -> Animation {
        Animation {
            id,
            delay,
            duration: duration.max(0.0),
            end_delay: 0.0,
            fill_forwards,
            start: Some(ms(now)),
            hold: None,
            has_effect: true,
            finish_raised: false,
            cancelled: false,
        }
    }

    /// The end of the effect (`delay + duration + endDelay`).
    pub fn end_time(&self) -> f64 {
        (self.delay + self.duration + self.end_delay).max(0.0)
    }

    /// `currentTime`: ms since the start (delay included), `None` when idle; held at the end once
    /// finished.
    pub fn current_time(&self, now: Time) -> Option<f64> {
        if self.cancelled {
            return None;
        }
        if let Some(h) = self.hold {
            return Some(h);
        }
        let t = ms(now) - self.start?;
        Some(t.min(self.end_time()))
    }

    /// `currentTime = t`: seek (a running animation keeps running from there).
    pub fn set_current_time(&mut self, t: f64, now: Time) {
        self.cancelled = false;
        if self.start.is_some() {
            self.start = Some(ms(now) - t);
            self.hold = None;
        } else {
            self.hold = Some(t);
        }
        if t < self.end_time() {
            self.finish_raised = false;
        }
    }

    pub fn play_state(&self, now: Time) -> PlayState {
        if self.cancelled {
            return PlayState::Idle;
        }
        match (self.start, self.current_time(now)) {
            (None, Some(t)) if t < self.end_time() => PlayState::Paused,
            (_, Some(t)) if t >= self.end_time() => PlayState::Finished,
            (Some(_), _) => PlayState::Running,
            _ => PlayState::Idle,
        }
    }

    pub fn pause(&mut self, now: Time) {
        if let Some(t) = self.current_time(now) {
            self.hold = Some(t);
            self.start = None;
        }
    }

    pub fn play(&mut self, now: Time) {
        let t = match self.current_time(now) {
            Some(t) if t < self.end_time() => t,
            _ => 0.0,
        };
        self.cancelled = false;
        self.start = Some(ms(now) - t);
        self.hold = None;
        if t < self.end_time() {
            self.finish_raised = false;
        }
    }

    /// `cancel()`: idle, no effect, no finish event.
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.start = None;
        self.hold = None;
        self.finish_raised = true;
    }

    /// `finish()`: jump to the end; the finish event follows on the next poll.
    pub fn finish(&mut self) {
        self.cancelled = false;
        self.hold = Some(self.end_time());
        self.start = None;
    }

    /// `effect = null`.
    pub fn clear_effect(&mut self) {
        self.has_effect = false;
    }

    /// Whether the animation applies its effect at `now` (running or paused, or finished with fill
    /// forwards).
    pub fn in_effect(&self, now: Time) -> bool {
        self.has_effect
            && match self.play_state(now) {
                PlayState::Idle => false,
                PlayState::Finished => self.fill_forwards,
                PlayState::Running | PlayState::Paused => true,
            }
    }

    /// The active-phase progress 0..=1 at `now`; `None` before the delay ends and after the end
    /// without fill.
    pub fn progress(&self, now: Time) -> Option<f64> {
        if !self.has_effect {
            return None;
        }
        let t = self.current_time(now)? - self.delay;
        if t < 0.0 {
            return None;
        }
        if t >= self.duration {
            return self.fill_forwards.then_some(1.0);
        }
        Some(if self.duration == 0.0 { 1.0 } else { t / self.duration })
    }

    /// When the animation will finish if left running (for the frame clock's wakeup).
    pub fn finishes_at(&self) -> Option<Time> {
        let end = self.start? + self.end_time();
        Some(Time((end.max(0.0) * 1e6) as u64))
    }
}

/// The animations of one document, by `OP_ANIMATE` id, with the node each runs on.
#[derive(Debug)]
pub struct Animations<N> {
    by_id: HashMap<u64, (N, Animation)>,
}

impl<N> Default for Animations<N> {
    fn default() -> Self {
        Animations { by_id: HashMap::new() }
    }
}

impl<N: Copy + PartialEq> Animations<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, node: N, animation: Animation) {
        self.by_id.insert(animation.id, (node, animation));
    }

    pub fn get(&self, id: u64) -> Option<&Animation> {
        self.by_id.get(&id).map(|(_, a)| a)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Animation> {
        self.by_id.get_mut(&id).map(|(_, a)| a)
    }

    /// `cancel()` and drop it (Svelte never reuses a cancelled animation).
    pub fn cancel(&mut self, id: u64) -> Option<N> {
        self.by_id.remove(&id).map(|(n, _)| n)
    }

    /// Drops every animation of a destroyed node.
    pub fn remove_node(&mut self, node: N) {
        self.by_id.retain(|_, (n, _)| *n != node);
    }

    /// `node.getAnimations()`: the ids in effect on `node`, oldest first.
    pub fn of_node(&self, node: N, now: Time) -> Vec<u64> {
        let mut ids: Vec<u64> = self.by_id.iter().filter(|(_, (n, a))| *n == node && a.in_effect(now)).map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids
    }

    /// The animations that finished since the last poll, as `(node, id)`, each once: the host sends
    /// them as `animationend` (JS runs `onfinish`).
    pub fn poll(&mut self, now: Time) -> Vec<(N, u64)> {
        let mut out = Vec::new();
        for (id, (node, a)) in self.by_id.iter_mut() {
            if !a.finish_raised && a.play_state(now) == PlayState::Finished {
                a.finish_raised = true;
                out.push((*node, *id));
            }
        }
        out.sort_unstable_by_key(|&(_, id)| id);
        out
    }

    /// The earliest finish of a running animation (the next wakeup).
    pub fn next_finish(&self) -> Option<Time> {
        self.by_id.values().filter(|(_, a)| !a.finish_raised).filter_map(|(_, a)| a.finishes_at()).min()
    }
}
