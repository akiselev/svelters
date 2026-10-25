//! skelter [`Time`] on Linux: nanoseconds of `CLOCK_MONOTONIC`. Wayland input timestamps (32-bit
//! milliseconds, `CLOCK_MONOTONIC` on every compositor in practice) and presentation timestamps
//! (`wp_presentation.clock_id`, normally `CLOCK_MONOTONIC`) map onto it directly.

use skelter_base::time::{Clock, Span, Time};
use rustix::time::{ClockId, clock_gettime};

#[derive(Clone, Copy, Debug, Default)]
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    fn now(&self) -> Time {
        now()
    }
}

pub fn now() -> Time {
    let ts = clock_gettime(ClockId::Monotonic);
    Time(ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
}

/// The Linux `CLOCK_MONOTONIC` id, as `wp_presentation.clock_id` reports it.
pub const CLOCK_MONOTONIC_ID: u32 = 1;

/// Maps a 32-bit millisecond event timestamp onto skelter time, given the time it was received. The
/// timestamp wraps every ~49.7 days; an event is never from the future, so a timestamp ahead of `recv`
/// (clock skew between compositor and client, or a compositor that does not use `CLOCK_MONOTONIC`) and one
/// more than ten seconds old both fall back to `recv`.
pub fn from_event_ms(ms: u32, recv: Time) -> Time {
    let recv_ms = (recv.0 / 1_000_000) as u32;
    let age_ms = recv_ms.wrapping_sub(ms);
    if age_ms > 10_000 {
        return recv;
    }
    let sub_ms = recv.0 % 1_000_000;
    recv - Span::from_millis(age_ms as i64) - Span::from_nanos(sub_ms as i64)
}

/// A `wp_presentation_feedback.presented` timestamp as skelter time; `None` for a clock other than
/// `CLOCK_MONOTONIC` (the caller then uses its receive time and marks the sample estimated).
pub fn from_presentation(clock_id: u32, tv_sec_hi: u32, tv_sec_lo: u32, tv_nsec: u32) -> Option<Time> {
    if clock_id != CLOCK_MONOTONIC_ID {
        return None;
    }
    let secs = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
    Some(Time(secs * 1_000_000_000 + tv_nsec as u64))
}

/// `t` as the `(tv_sec_hi, tv_sec_lo, tv_nsec)` triple `wp_commit_timer_v1.set_timestamp` takes.
pub fn to_timespec_parts(t: Time) -> (u32, u32, u32) {
    let secs = t.0 / 1_000_000_000;
    ((secs >> 32) as u32, secs as u32, (t.0 % 1_000_000_000) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_ms_maps_back_from_receive_time() {
        let recv = Time(5_000_123_456_789); // 5000.123456789 s
        let t = from_event_ms(5_000_120, recv);
        assert_eq!(t, Time(5_000_120_000_000));
    }

    #[test]
    fn event_ms_handles_wraparound() {
        // recv just past a u32 millisecond wrap; the event just before it.
        let wrap_ms = 1u64 << 32;
        let recv = Time((wrap_ms + 3) * 1_000_000);
        let t = from_event_ms(u32::MAX - 1, recv);
        assert_eq!(t, Time((wrap_ms - 2) * 1_000_000));
    }

    #[test]
    fn future_or_stale_timestamps_fall_back() {
        let recv = Time(100_000 * 1_000_000);
        assert_eq!(from_event_ms(100_005, recv), recv);
        assert_eq!(from_event_ms(80_000, recv), recv);
    }

    #[test]
    fn presentation_and_commit_timer_round_trip() {
        let t = Time(((1u64 << 32) + 11) * 1_000_000_000 + 999);
        let (hi, lo, ns) = to_timespec_parts(t);
        assert_eq!(from_presentation(CLOCK_MONOTONIC_ID, hi, lo, ns), Some(t));
        assert_eq!(from_presentation(7, hi, lo, ns), None);
    }
}
