//! SCHED1 (ZED-LESSONS §6.1, §9 #1): where off-thread work runs and what time it is, as two small
//! traits, and a deterministic [`TestScheduler`] that implements both for seeded randomized tests.
//!
//! - [`Spawner`] runs a [`Job`] "somewhere else" (a worker pool, a blocking pool). The shape is
//!   quire's planned `Spawner` (ARCH-CRATES §3.6) and close to cadatomic's `JobPool::execute`, so
//!   both can implement it, and a closure `Fn(Job)` already does.
//! - [`Clock`] says what time it is and wakes a [`Waker`] at a deadline. A host given a clock runs its
//!   timers, `performance.now()` and its backend wakeups on it instead of the wall clock.
//! - [`TestScheduler`] implements both on one thread: spawned jobs, deadline wakes and the event
//!   loops a test registers ("foregrounds": a host's checkpoint, an egui frame) wait in one ready set,
//!   and a seeded RNG picks which runs next. Time only moves when the test says so
//!   ([`TestScheduler::advance_clock`], [`TestScheduler::run_for`], [`TestScheduler::run_until_idle`]).
//!   Each foreground is one sequential loop: the scheduler never reorders the work inside a turn
//!   (one JS microtask queue drains inside a checkpoint, as in production).
//! - [`run_seeds`] runs a test body once per seed: `ITERATIONS=n` seeds from 0 (or the test's own
//!   default), `SEED=n` exactly one schedule; a failure prints its seed.
//!
//! ```
//! use svelters_sched::{Spawner, TestScheduler, run_seeds};
//! use std::sync::{Arc, Mutex};
//!
//! run_seeds("doc_example", 4, |s: &TestScheduler| {
//!     let order = Arc::new(Mutex::new(Vec::new()));
//!     for i in 0..3 {
//!         let order = order.clone();
//!         s.spawner().spawn(Box::new(move || order.lock().unwrap().push(i)));
//!     }
//!     s.run_until_parked();
//!     let mut got = order.lock().unwrap().clone();
//!     got.sort();
//!     assert_eq!(got, [0, 1, 2]); // every job ran, in an order the seed chose
//! });
//! ```

mod rng;
mod seeds;
mod test_scheduler;

pub use rng::Rng;
pub use seeds::{Seeds, run_seeds};
pub use test_scheduler::{Delay, Foreground, SchedHandle, TestScheduler, Timer, current, simulate_random_delay};

use std::sync::atomic::{AtomicU64, Ordering};
use std::task::Waker;
use std::time::Duration;

/// A unit of off-thread work.
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// What kind of pool a job asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum JobKind {
    /// CPU work (svelters' `#[js(worker)]`: rayon by default).
    #[default]
    Worker,
    /// Work that blocks a thread (file or database I/O, sleeps: `#[js(blocking)]`).
    Blocking,
    /// Anything else.
    Other,
}

/// What a spawner may want to know about a job: its kind and a label for traces and profiles.
/// Room for a priority and a principal later (ZED-LESSONS §1.9, §1.10; ARCH-SECURITY §5).
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct JobMeta {
    pub kind: JobKind,
    /// `app:<module>.<member>` for an export call; `None` when nobody named it.
    pub label: Option<String>,
}

impl JobMeta {
    pub fn new(kind: JobKind) -> JobMeta {
        JobMeta { kind, label: None }
    }

    pub fn label(mut self, label: impl Into<String>) -> JobMeta {
        self.label = Some(label.into());
        self
    }
}

/// Runs jobs somewhere else. Implementations decide threads, queues and priorities; the caller only
/// knows that `job` runs once, eventually, possibly on another thread.
pub trait Spawner: Send + Sync {
    fn spawn(&self, job: Job);

    /// [`Spawner::spawn`] with what the caller knows about the job. The default ignores it.
    fn spawn_with(&self, meta: JobMeta, job: Job) {
        let _ = meta;
        self.spawn(job);
    }
}

impl<F: Fn(Job) + Send + Sync> Spawner for F {
    fn spawn(&self, job: Job) {
        self(job)
    }
}

/// A clock and its wakeups. Time is a [`Duration`] since the clock's own epoch.
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;

    /// Wakes `waker` once [`Clock::now`] reaches `at`. A later call with the same `key` replaces the
    /// earlier request; `None` cancels it. Keys come from [`wake_key`].
    fn wake_at(&self, key: u64, at: Option<Duration>, waker: &Waker);
}

/// A process-wide unique key for [`Clock::wake_at`].
pub fn wake_key() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
