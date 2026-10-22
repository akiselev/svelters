//! SCHED1: the host's job, clock and wake hooks ([`HostSched`], `HostConfig::sched`).
//!
//! By default a host runs offloaded export calls on rayon and its blocking pool, keeps time with the
//! wall clock, and tells only the backend when it needs a checkpoint (`wake`) or a timer (`schedule_wake`).
//! Each hook replaces or extends one of those:
//!
//! | Hook | Default | With the hook |
//! | --- | --- | --- |
//! | `spawner` | rayon / blocking pool | `#[js(worker)]` and `#[js(blocking)]` jobs and the `exports::worker()`/`blocking()` futures go to it (`JobMeta` carries the kind and `app:<module>.<member>`) |
//! | `clock` | `Instant::now()` | timers, `performance.now()` and the timer wakeup (`Clock::wake_at` instead of the backend's `schedule_wake`) |
//! | `wake` | backend `wake` only | also notified on every host wake: a pending job, a completion, a store change, a queued event |
//! | `seed` | QuickJS's own `Math.random` | AU1b: `Math.random` is a seeded generator (sfc32, 53-bit doubles), reseeded with the same seed by every engine the host makes (a full reload included), so a run replays |
//!
//! A production embedder may set only `spawner` (cadatomic's job pool, quire's `Spawner`). A test sets
//! all three from one [`svelters_sched::TestScheduler`] (`testing::SchedHost` does it on the log backend;
//! `svelters-testkit`'s `sched` module on egui), so a real host runs deterministically under a seed.
//!
//! Not covered yet (follow-ups in plans/milestones/SCHED1-deterministic-scheduler.md): `#[js(tokio)]`
//! tasks, E3 service threads and the file watcher still use their own threads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Wake, Waker};
use svelters_abi::WakeHandle;
use svelters_sched::{Clock, Spawner};

/// The scheduler hooks of a host. All optional; see the module docs.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct HostSched {
    pub spawner: Option<Arc<dyn Spawner>>,
    pub clock: Option<Arc<dyn Clock>>,
    pub wake: Option<Waker>,
    /// AU1b: seeds `Math.random` (see the module docs). `None` leaves QuickJS's generator alone and
    /// costs nothing.
    pub seed: Option<u64>,
}

impl HostSched {
    pub fn spawner(mut self, s: Arc<dyn Spawner>) -> HostSched {
        self.spawner = Some(s);
        self
    }

    pub fn clock(mut self, c: Arc<dyn Clock>) -> HostSched {
        self.clock = Some(c);
        self
    }

    pub fn wake(mut self, w: Waker) -> HostSched {
        self.wake = Some(w);
        self
    }

    pub fn seed(mut self, seed: u64) -> HostSched {
        self.seed = Some(seed);
        self
    }

    /// Every hook from one test scheduler: its spawner and clock, and `fg`'s waker.
    pub fn test(s: &svelters_sched::TestScheduler, fg: svelters_sched::Foreground) -> HostSched {
        HostSched::default().spawner(s.spawner()).clock(s.clock()).wake(s.waker(fg))
    }

    pub fn is_empty(&self) -> bool {
        self.spawner.is_none() && self.clock.is_none() && self.wake.is_none() && self.seed.is_none()
    }
}

impl std::fmt::Debug for HostSched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostSched")
            .field("spawner", &self.spawner.is_some())
            .field("clock", &self.clock.is_some())
            .field("wake", &self.wake.is_some())
            .field("seed", &self.seed)
            .finish()
    }
}

/// The host's waker: the backend's thread-safe `wake`, then the `wake` hook if any. Every host wake
/// goes through it (the core's own, the completion queue's and the store hub's). A scheduler may keep
/// a clone past the host (a due `wake_at` not yet run), so the host detaches it when it goes away and
/// later wakes reach nobody (the backend's vtable may be freed by then).
pub(crate) struct HostWake {
    backend: WakeHandle,
    extra: Option<Waker>,
    alive: AtomicBool,
}

impl HostWake {
    pub(crate) fn detach(&self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}

impl Wake for HostWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if !self.alive.load(Ordering::SeqCst) {
            return;
        }
        self.backend.wake();
        if let Some(w) = &self.extra {
            w.wake_by_ref();
        }
    }
}

pub(crate) fn host_waker(backend: WakeHandle, extra: Option<Waker>) -> (Waker, Arc<HostWake>) {
    let st = Arc::new(HostWake { backend, extra, alive: AtomicBool::new(true) });
    (Waker::from(st.clone()), st)
}
