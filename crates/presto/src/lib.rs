//! presto's facade and app runner (`plans/cloud/presto-b2/LANE-app-test.md`).
//!
//! [`App`] runs on the UI thread as the platform's app handler: it owns the tree, dispatches input at
//! event time, answers the compositor's `BeginFrame`s with commits, and stamps every input's latency
//! from the OS timestamp to presentation ([`latency`]).
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the [`App`] runner, the resize handshake, the compositor's [`ToUiSink`] |
//! | [`coalesce`] | per-frame coalescing of pointer moves and wheel deltas |
//! | [`latency`] | stage records per `InputSeq`, histograms, Chrome JSON trace export |
//! | [`shim`] | stand-ins for wave-1 APIs not merged yet (replaced at consolidation) |

pub mod app;
pub mod coalesce;
pub mod latency;
pub mod shim;

pub use app::{App, AppConfig, ToUiSink};
pub use presto_base as base;
