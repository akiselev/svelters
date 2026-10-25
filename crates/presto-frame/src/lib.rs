//! presto's frame scheduler (`plans/research/presto/architecture.md` §10; the design and constants are
//! in `plans/research/presto/notes/round2/scheduler-latency.md` §2–§3 and §6).
//!
//! Everything here is a pure function of the signals and times it is given, so the platform layer
//! only translates OS callbacks into [`Signal`]s and [`Action`]s into OS calls, and [`sim`] can run it
//! under a seeded virtual clock.
//!
//! | Module | What |
//! |---|---|
//! | [`clock`] | the compositor-side [`FrameClock`], one per window |
//! | [`ui`] | the UI-thread [`UiFrameState`]: urgent input, back-pressure |
//! | [`estimate`] | cost series (p90 of 64), the latch margin, `S_ui`/`D_ui`/`D_comp` |
//! | [`rate`] | high-rate input detection for VRR keep-alive |
//! | [`sim`] | the deterministic simulator and its invariant checks |

pub mod clock;
pub mod estimate;
pub mod rate;
pub mod sim;
pub mod ui;

pub use clock::{Action, ClockConfig, ClockStats, DeadlineMode, FrameArgs, FrameClock, Signal};
pub use estimate::{Estimator, Schedule};
pub use rate::{InputRateTracker, RateConfig};
pub use ui::{CommitReport, UiFrameState, UiPhase, Urgency};
