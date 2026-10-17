//! Optional measurement of the time spent inside backend calls (the toolkit side of a host op).
//!
//! Disabled by default; `set_enabled(true)` costs two clock reads per backend call.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static NS: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Returns `(nanoseconds, calls)` spent in backend calls since the last take, and resets both.
pub fn take() -> (u64, u64) {
    (NS.swap(0, Ordering::Relaxed), CALLS.swap(0, Ordering::Relaxed))
}

pub(crate) struct Scope(Option<Instant>);

pub(crate) fn scope() -> Scope {
    Scope(ENABLED.load(Ordering::Relaxed).then(Instant::now))
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(t0) = self.0 {
            NS.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
    }
}
