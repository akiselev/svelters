//! presto's real clock: nanoseconds since a process-wide epoch taken from `Instant` on first use,
//! so every thread and crate reads the same [`Time`]. OS timestamps on other clocks convert through
//! [`time_from_os`], which samples both clocks now.

use presto_base::time::{Clock, Span, Time};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Now on presto's clock.
pub fn now() -> Time {
    from_instant(Instant::now())
}

pub fn from_instant(i: Instant) -> Time {
    Time(i.saturating_duration_since(epoch()).as_nanos() as u64)
}

pub fn to_instant(t: Time) -> Instant {
    epoch() + Duration::from_nanos(t.0)
}

/// A timestamp `os_ns` on another monotonic clock whose current reading is `os_now_ns`, on presto's
/// clock (both in nanoseconds; `os_now_ns` sampled as close to now as possible).
pub fn time_from_os(os_ns: u64, os_now_ns: u64) -> Time {
    now() - Span(os_now_ns as i64 - os_ns as i64)
}

/// The [`Clock`] that reads [`now`].
#[derive(Clone, Copy, Debug, Default)]
pub struct OsClock;

impl Clock for OsClock {
    fn now(&self) -> Time {
        now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_round_trip() {
        let t = now();
        let back = from_instant(to_instant(t));
        assert_eq!(t, back);
    }

    #[test]
    fn os_time_is_in_the_past() {
        // Run late enough after the epoch that "3 ms ago" does not clamp to it.
        let before = now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t = time_from_os(1_000, 3_000_000);
        let after = now();
        assert!(t > before && t <= after, "{before:?} < {t:?} <= {after:?}");
        assert!(after - t >= Span::from_micros(2_999));
    }
}
