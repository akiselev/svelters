//! skelter's facade and app runner (`plans/cloud/presto-b2/LANE-app-test.md`).
//!
//! [`App`] runs on the UI thread as the platform's app handler: it owns the tree, dispatches input at
//! event time, answers the compositor's `BeginFrame`s with commits, and stamps every input's latency
//! from the OS timestamp to presentation ([`latency`]).
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the [`App`] runner, the resize handshake, the compositor's [`ToUiSink`] |
//! | [`seams`] | what the runner needs from the tree, the compositor and the script host |
//! | [`backends`] | those seams for `skelter_core::Document`, `skelter_compositor::CompositorHandle`, `skelter_frame::UiFrameState` |
//! | [`coalesce`] | per-frame coalescing of pointer moves and wheel deltas |
//! | [`run`] | the platform backend chosen at start (Win32, AppKit, Wayland; winit otherwise or as the fallback) |
//! | [`latency`] | stage records per `InputSeq`, histograms, Chrome JSON trace export |

pub mod app;
pub mod backends;
pub mod coalesce;
pub mod latency;
pub mod run;
pub mod seams;

pub use app::{App, AppConfig, SpawnCompositor, ToUiSink};
pub use backends::{CoreDocument, UiFrame, caret_blink_of, compositor};
pub use skelter_base as base;
pub use skelter_compositor as compositor_crate;
pub use skelter_core as core;
pub use skelter_platform as platform;
pub use run::{Backend, run};
