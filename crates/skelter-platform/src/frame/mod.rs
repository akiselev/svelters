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
//!
//! Behaviour every caller can rely on (recorded from the Wayland lane's questions, 2026-10-26):
//! - **One source per surface, independent waits.** `wait` returns only for its own surface (a frame,
//!   an occlusion change or the timeout). Sources of several windows may share one OS connection
//!   (Wayland's socket): a wait on one reads the others' events into their own queues, and they pick
//!   them up on their next `wait` or `take_presented` with no extra delay. A thread waiting on several
//!   windows therefore waits on each source in turn (or one thread per source).
//! - **A source may commit by itself.** With nothing presented since the last frame callback, the
//!   Wayland source asks for the next callback with a bare `wl_surface.commit` (no buffer), so `wait`
//!   keeps ticking while the app draws nothing. Such a commit also applies a pending `ack_configure`
//!   with the old buffer; it is skipped while a resize's surface state is pending, so a resize's state
//!   always lands with a buffer of the new size ([`FrameSource::before_present_sized`]).
//! - **FIFO pacing is opt-in.** A source never adds `wp_fifo_v1` on its own: Mesa's Vulkan WSI makes
//!   its own for a FIFO swapchain and a second one is a protocol error. Enable it only with a Mailbox
//!   or Immediate swapchain (skelter-platform-wayland's `FrameSourceConfig::fifo`).

use skelter_base::ids::FrameId;
use skelter_base::time::{Clock, Span, Time};
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
    /// Blocks until this source's next frame opportunity or occlusion change, or `timeout` (see the
    /// module docs for sources that share a connection).
    fn wait(&self, timeout: Span) -> FrameWait;
    /// Called right before presenting `frame`: arms the next frame callback and presentation
    /// feedback where the OS works that way.
    fn before_present(&self, _frame: FrameId) {}
    /// [`FrameSource::before_present`] knowing the buffer size `frame` presents at (device pixels).
    /// Sources that apply resize state with a present (Wayland's viewport and buffer scale) apply it
    /// only with a buffer of the new size, so an old-size frame after a configure stays valid. The
    /// compositor calls this one; the default forwards to `before_present`.
    fn before_present_sized(&self, frame: FrameId, _size: skelter_base::geom::DeviceSize) {
        self.before_present(frame)
    }
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
