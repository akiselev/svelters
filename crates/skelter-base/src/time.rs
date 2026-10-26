//! Time. One monotonic clock for every thread: nanoseconds since an arbitrary process epoch. The
//! platform converts OS timestamps (QPC, `mach_absolute_time`, `CLOCK_MONOTONIC`) into it; the
//! scheduler's simulator injects its own.

use std::ops::{Add, Sub};

/// A point on the monotonic clock, in nanoseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub u64);

/// A signed span of time in nanoseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span(pub i64);

impl Time {
    pub const ZERO: Time = Time(0);
    pub fn saturating_sub(self, o: Time) -> Span {
        Span(self.0 as i64 - o.0 as i64)
    }
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 * 1e-9
    }
}

impl Span {
    pub const ZERO: Span = Span(0);
    pub const fn from_nanos(n: i64) -> Self {
        Span(n)
    }
    pub const fn from_micros(us: i64) -> Self {
        Span(us * 1_000)
    }
    pub const fn from_millis(ms: i64) -> Self {
        Span(ms * 1_000_000)
    }
    pub fn from_secs_f64(s: f64) -> Self {
        Span((s * 1e9) as i64)
    }
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 * 1e-9
    }
    pub fn as_millis_f64(self) -> f64 {
        self.0 as f64 * 1e-6
    }
}

impl Add<Span> for Time {
    type Output = Time;
    fn add(self, s: Span) -> Time {
        Time((self.0 as i64 + s.0).max(0) as u64)
    }
}
impl Sub<Span> for Time {
    type Output = Time;
    fn sub(self, s: Span) -> Time {
        Time((self.0 as i64 - s.0).max(0) as u64)
    }
}
impl Sub for Time {
    type Output = Span;
    fn sub(self, o: Time) -> Span {
        Span(self.0 as i64 - o.0 as i64)
    }
}
impl Add for Span {
    type Output = Span;
    fn add(self, o: Span) -> Span {
        Span(self.0 + o.0)
    }
}
impl Sub for Span {
    type Output = Span;
    fn sub(self, o: Span) -> Span {
        Span(self.0 - o.0)
    }
}

/// A source of [`Time`]. The real one reads the OS clock; tests and the simulator inject theirs.
pub trait Clock: Send + Sync {
    fn now(&self) -> Time;
}
