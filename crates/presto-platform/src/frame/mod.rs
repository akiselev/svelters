//! Frame sources: what the compositor thread paces frames by (architecture §7, platform.md §4).
//! A source blocks until the next frame opportunity and reports when frames reached the screen
//! where the OS tells. Sources are `Send + Sync` so one thread can wait on vsync while the
//! compositor arms presentation feedback before each present.
//!
//! | OS | Source |
//! |---|---|
//! | Windows 11 | the compositor clock (`DCompositionWaitForCompositorClock`) |
//! | Windows 10 | `DwmFlush`, with GPUI's rule: a return under 1 ms sleeps one interval instead |
//! | macOS | `CVDisplayLink` (`CAMetalDisplayLink` is the planned replacement) |
//! | Wayland | frame callbacks plus `wp_presentation` feedback, occlusion after 100 ms without a callback |
//! | X11, fallback | [`IntervalSource`], estimated |

use presto_base::ids::FrameId;
use presto_base::time::{Clock, Span, Time};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;

/// What a wait ended with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameWait {
    /// A frame opportunity: present for `target`.
    Vsync { target: Time, interval: Span, estimated: bool },
    TimedOut,
    /// The surface stopped (or resumed) getting frames: hidden, minimised or covered.
    Occluded(bool),
}

/// A frame that reached the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presented {
    pub frame: FrameId,
    pub at: Time,
    /// The OS did not say; `at` is a guess.
    pub estimated: bool,
}

pub trait FrameSource: Send + Sync {
    fn name(&self) -> &'static str;
    /// Blocks until the next frame opportunity, or `timeout`.
    fn wait(&self, timeout: Span) -> FrameWait;
    /// Called right before presenting `frame`: arms the next frame callback and presentation
    /// feedback where the OS works that way.
    fn before_present(&self, _frame: FrameId) {}
    /// Presentation feedback that arrived since the last call.
    fn take_presented(&self) -> Vec<Presented> {
        Vec::new()
    }
    /// The refresh interval, when known.
    fn interval(&self) -> Option<Span>;
}

/// Ticks at a fixed interval from a phase origin, sleeping in between; every tick is an estimate.
/// For X11, headless runs and as the fallback when the OS source fails.
pub struct IntervalSource {
    interval: Span,
    clock: Arc<dyn Clock>,
    origin: Time,
    last: Mutex<Option<Time>>,
}

impl IntervalSource {
    pub fn new(interval: Span, clock: Arc<dyn Clock>) -> Self {
        let origin = clock.now();
        IntervalSource { interval: Span(interval.0.max(1_000_000)), clock, origin, last: Mutex::new(None) }
    }

    /// From a refresh rate in millihertz (60 Hz when unknown).
    pub fn from_refresh(millihertz: Option<u32>, clock: Arc<dyn Clock>) -> Self {
        let mhz = millihertz.filter(|m| *m >= 1_000).unwrap_or(60_000) as i64;
        Self::new(Span(1_000_000_000_000 / mhz), clock)
    }

    /// The first tick strictly after both `now` and the previous tick.
    pub fn next_tick(&self, now: Time, last: Option<Time>) -> Time {
        let i = self.interval.0;
        let since = (now - self.origin).0.max(0);
        let mut t = self.origin + Span((since / i + 1) * i);
        if let Some(l) = last {
            while t <= l {
                t = t + self.interval;
            }
        }
        t
    }
}

impl FrameSource for IntervalSource {
    fn name(&self) -> &'static str {
        "interval"
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let now = self.clock.now();
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let t = self.next_tick(now, *last);
        if t - now > timeout {
            sleep(timeout);
            return FrameWait::TimedOut;
        }
        sleep(t - self.clock.now());
        *last = Some(t);
        FrameWait::Vsync { target: t + self.interval, interval: self.interval, estimated: true }
    }

    fn interval(&self) -> Option<Span> {
        Some(self.interval)
    }
}

pub(crate) fn sleep(s: Span) {
    if s.0 > 0 {
        std::thread::sleep(std::time::Duration::from_nanos(s.0 as u64));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::VirtualClock;

    #[test]
    fn interval_ticks_are_aligned_and_strictly_increasing() {
        let clock = VirtualClock::new(Time(1_000));
        let s = IntervalSource::new(Span::from_millis(10), clock.clone());
        assert_eq!(s.next_tick(Time(1_000), None), Time(1_000 + 10_000_000));
        assert_eq!(s.next_tick(Time(1_000 + 15_000_000), None), Time(1_000 + 20_000_000));
        // A tick already handed out is never handed out again.
        assert_eq!(s.next_tick(Time(1_000 + 5_000_000), Some(Time(1_000 + 10_000_000))), Time(1_000 + 20_000_000));
    }

    #[test]
    fn refresh_rate_sets_the_interval() {
        let s = IntervalSource::from_refresh(Some(120_000), VirtualClock::new(Time::ZERO));
        assert_eq!(s.interval(), Some(Span(8_333_333)));
        let s = IntervalSource::from_refresh(None, VirtualClock::new(Time::ZERO));
        assert_eq!(s.interval(), Some(Span(16_666_666)));
    }

    #[test]
    fn a_short_timeout_times_out() {
        let s = IntervalSource::new(Span::from_millis(1_000), Arc::new(crate::clock::OsClock));
        assert_eq!(s.wait(Span::from_millis(1)), FrameWait::TimedOut);
    }
}
