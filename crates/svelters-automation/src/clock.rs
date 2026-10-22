//! AU1b: a session's clock and seed (E2E-INSPECTOR §1.6, §2.3).
//!
//! - [`VirtualClock`]: a seeded [`TestScheduler`] (SCHED1) behind the host's hooks. Its clock runs the
//!   host's timers, `performance.now()` and timer wakeups; its spawner runs `#[js(worker)]` and
//!   `#[js(blocking)]` jobs on the UI thread in a seeded order (E1 ordering); its seed also seeds
//!   `Math.random` (`HostSched::seed`). Time moves only when frames run.
//! - [`Clocked`]: a [`Driver`] whose every frame first lets one frame of virtual time pass on its
//!   [`VirtualClock`] (`run_for(frame_ms)`: due timers wake the host, ready jobs complete), then runs
//!   the frame. A 30 s `wait.for` costs frames, not 30 s.
//! - [`RealTime`]: a [`Driver`] paced by the wall clock (`--clock real`): each frame starts no
//!   earlier than one frame time after the last. The host keeps the wall clock.
//!
//! ```ignore
//! let clock = VirtualClock::new(7);
//! let app = TestApp::from_source_with("fixtures/counter-egui", 0.016, clock.config(HostConfig::default()))?;
//! let mut s = Session::for_test(Clocked::new(app, clock));
//! s.advance_clock(1000.0)?; // one virtual second, frame by frame
//! ```

use crate::driver::{A11yNode, Driver};
use std::time::{Duration, Instant};
use svelters_egui::scene::SceneHandle;
use svelters_host::svelters_sched::TestScheduler;
use svelters_host::{HostConfig, HostSched};

/// Which clock a session's host runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockMode {
    /// [`VirtualClock`]: time moves with the frames.
    Virtual,
    /// The wall clock; frames are paced in wall time ([`RealTime`]).
    Real,
}

impl ClockMode {
    pub fn parse(s: &str) -> Result<ClockMode, String> {
        match s {
            "virtual" => Ok(ClockMode::Virtual),
            "real" => Ok(ClockMode::Real),
            c => Err(format!("unknown clock `{c}` (virtual | real)")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ClockMode::Virtual => "virtual",
            ClockMode::Real => "real",
        }
    }
}

/// A seeded virtual clock for one host (see the module docs). Lives on the UI thread.
pub struct VirtualClock {
    sched: TestScheduler,
    seed: u64,
}

impl VirtualClock {
    pub fn new(seed: u64) -> VirtualClock {
        let sched = TestScheduler::new(seed);
        // A long-running app (svelters-run) must not hit the livelock guard meant for tests.
        sched.set_max_steps(u64::MAX);
        VirtualClock { sched, seed }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Virtual milliseconds since the clock was made.
    pub fn now_ms(&self) -> f64 {
        self.sched.now().as_secs_f64() * 1000.0
    }

    /// The scheduler, for a test that wants its knobs (`set_job_delay`, `forbid_parking`).
    pub fn scheduler(&self) -> &TestScheduler {
        &self.sched
    }

    /// The host hooks: the scheduler's spawner and clock, and the seed for `Math.random`. Host wakes
    /// keep going to the backend (the frame loop), so no foreground is registered.
    pub fn host_sched(&self) -> HostSched {
        HostSched::default()
            .spawner(self.sched.spawner())
            .clock(self.sched.clock())
            .seed(self.seed)
    }

    /// `cfg` on this clock (`scripted` off: it has its own virtual clock).
    pub fn config(&self, cfg: HostConfig) -> HostConfig {
        HostConfig {
            scripted: false,
            sched: self.host_sched(),
            ..cfg
        }
    }

    /// Lets `ms` of virtual time pass: due timers wake the host, ready jobs run. No frame runs; the
    /// host's checkpoint runs in the next one.
    pub fn advance(&self, ms: f64) {
        self.sched
            .run_for(Duration::from_secs_f64(ms.max(0.0) / 1000.0));
    }
}

/// A driver on a [`VirtualClock`]: each frame advances the clock by the frame time first.
pub struct Clocked<D> {
    // Dropped first: the host goes before the scheduler its hooks point at.
    pub driver: D,
    clock: VirtualClock,
}

impl<D: Driver> Clocked<D> {
    /// `driver`'s host must have been made with `clock.config(..)` (or `clock.host_sched()`).
    pub fn new(driver: D, clock: VirtualClock) -> Clocked<D> {
        Clocked { driver, clock }
    }

    pub fn clock(&self) -> &VirtualClock {
        &self.clock
    }
}

/// A driver paced by the wall clock (see the module docs).
pub struct RealTime<D> {
    pub driver: D,
    last: Option<Instant>,
}

impl<D: Driver> RealTime<D> {
    pub fn new(driver: D) -> RealTime<D> {
        RealTime { driver, last: None }
    }
}

macro_rules! delegate {
    () => {
        fn scene(&self) -> SceneHandle {
            self.driver.scene()
        }
        fn busy(&self) -> Option<String> {
            self.driver.busy()
        }
        fn send(&mut self, event: egui::Event) {
            self.driver.send(event)
        }
        fn frame_ms(&self) -> f64 {
            self.driver.frame_ms()
        }
        fn a11y_nodes(&self) -> Vec<A11yNode> {
            self.driver.a11y_nodes()
        }
        fn screenshot(&mut self) -> Option<egui::ColorImage> {
            self.driver.screenshot()
        }
    };
}

impl<D: Driver> Driver for Clocked<D> {
    delegate!();
    fn step(&mut self) {
        let ms = self.driver.frame_ms();
        self.clock.advance(if ms > 0.0 { ms } else { 16.0 });
        self.driver.step();
    }
    fn clock_ms(&self) -> Option<f64> {
        Some(self.clock.now_ms())
    }
    fn seed(&self) -> Option<u64> {
        Some(self.clock.seed)
    }
}

impl<D: Driver> Driver for RealTime<D> {
    delegate!();
    fn step(&mut self) {
        let frame = Duration::from_secs_f64(self.driver.frame_ms().max(1.0) / 1000.0);
        if let Some(last) = self.last {
            let next = last + frame;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            }
        }
        self.last = Some(Instant::now());
        self.driver.step();
    }
}
