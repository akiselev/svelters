//! presto's clock on macOS: `mach_absolute_time` in nanoseconds. `NSEvent.timestamp`,
//! `CACurrentMediaTime` and display-link timestamps count seconds on the same clock, so they convert
//! by scaling alone.

use presto_base::time::{Clock, Span, Time};
use std::sync::OnceLock;

#[repr(C)]
#[derive(Default)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

fn timebase() -> (u64, u64) {
    static TB: OnceLock<(u64, u64)> = OnceLock::new();
    *TB.get_or_init(|| {
        let mut info = MachTimebaseInfo::default();
        // SAFETY: `info` is a valid out-pointer for the call.
        let ok = unsafe { mach_timebase_info(&mut info) } == 0;
        if ok && info.denom != 0 {
            (info.numer as u64, info.denom as u64)
        } else {
            (1, 1)
        }
    })
}

/// The current time.
pub fn now() -> Time {
    // SAFETY: no preconditions.
    from_mach_ticks(unsafe { mach_absolute_time() })
}

/// `mach_absolute_time` ticks (Core Video host time) as presto time.
pub fn from_mach_ticks(ticks: u64) -> Time {
    let (numer, denom) = timebase();
    Time((ticks as u128 * numer as u128 / denom as u128) as u64)
}

/// An AppKit / Core Animation timestamp (seconds on the mach clock) as presto time.
pub fn from_secs(s: f64) -> Time {
    Time((s.max(0.0) * 1e9) as u64)
}

/// A span in seconds (display-link durations).
pub fn span_from_secs(s: f64) -> Span {
    Span::from_secs_f64(s)
}

/// The [`Clock`] the platform hands out.
#[derive(Debug, Default)]
pub struct MachClock;

impl Clock for MachClock {
    fn now(&self) -> Time {
        now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_monotonic_and_matches_seconds() {
        let a = now();
        let b = MachClock.now();
        assert!(b >= a);
        assert_eq!(from_secs(1.5), Time(1_500_000_000));
        assert_eq!(from_secs(-1.0), Time::ZERO);
    }
}
