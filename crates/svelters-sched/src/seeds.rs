//! `SEED` and `ITERATIONS`: which schedules a randomized test runs.

use crate::TestScheduler;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// The seeds a test runs: `first..first + count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seeds {
    pub first: u64,
    pub count: u64,
}

impl Seeds {
    /// From the environment:
    /// - neither set: seeds `0..default_iterations` (the test's own count, kept modest for CI);
    /// - `ITERATIONS=n`: seeds `0..n`;
    /// - `SEED=s`: exactly seed `s` (the schedule a failure printed);
    /// - both: `s..s + n`.
    ///
    /// A value that does not parse is a panic, not a silent default.
    pub fn from_env(default_iterations: u64) -> Seeds {
        let var = |k: &str| {
            std::env::var(k).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().parse::<u64>().unwrap_or_else(|_| panic!("{k}={v:?} is not a number")))
        };
        Seeds::new(var("SEED"), var("ITERATIONS"), default_iterations)
    }

    /// [`Seeds::from_env`] with the variables given.
    pub fn new(seed: Option<u64>, iterations: Option<u64>, default_iterations: u64) -> Seeds {
        match (seed, iterations) {
            (Some(s), None) => Seeds { first: s, count: 1 },
            (Some(s), Some(n)) => Seeds { first: s, count: n.max(1) },
            (None, Some(n)) => Seeds { first: 0, count: n.max(1) },
            (None, None) => Seeds { first: 0, count: default_iterations.max(1) },
        }
    }

    pub fn iter(self) -> impl Iterator<Item = u64> {
        self.first..self.first.saturating_add(self.count)
    }
}

/// Runs `body` once per seed of [`Seeds::from_env`], each on a fresh [`TestScheduler`]. A panic stops
/// the run after printing the seed and how to replay exactly that schedule; returns how many seeds ran
/// (a test that checks coverage across schedules, "both outcomes were seen", should only do so when
/// it ran enough of them).
pub fn run_seeds(name: &str, default_iterations: u64, mut body: impl FnMut(&TestScheduler)) -> u64 {
    let seeds = Seeds::from_env(default_iterations);
    for seed in seeds.iter() {
        let s = TestScheduler::new(seed);
        let r = catch_unwind(AssertUnwindSafe(|| body(&s)));
        let steps = s.steps();
        drop(s);
        if let Err(p) = r {
            eprintln!(
                "\n{name}: FAILED with SEED={seed} after {steps} scheduler steps (seeds {}..{}).\n  Replay exactly this schedule: SEED={seed} cargo nextest run <package> {name}\n",
                seeds.first,
                seeds.first.saturating_add(seeds.count)
            );
            resume_unwind(p);
        }
    }
    seeds.count
}
