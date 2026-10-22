//! TK1 (cadatomic SV34): one `.svelte` test case with inline expectations, run on every backend.
//!
//! A case is a component plus an `@test` comment ([`case`] has the format). `svelters-cases`
//! (`packages/testkit`) builds it once per toolkit, with `svelters:testkit` resolved to that
//! toolkit's kit (portable `Window`, `Column`, `Row`, `Button`, `TextInput`, `Check`). Then:
//!
//! | Backend | Runner | View | Actions |
//! |---|---|---|---|
//! | `log-gtk`, `log-qt` | [`log`] (log backend, the toolkit's prop schema) | op log replayed into a tree | `event`/`changed` steps |
//! | `egui` | [`egui`] (`TestApp`, kittest) | AccessKit tree | kittest input |
//! | `gtk`, `qt` | `examples/counter` (`tests/testkit.rs`) | native widget dump per checkpoint | native click and typing |
//!
//! The GTK and Qt runs perform the handle-based [`log::Plan`] the log run resolved, since all three
//! number handles in creation order. Results go through the per-backend [`ratchet`].
//!
//! ```ignore
//! let cases = svelters_testkit::case::load_dir(&dir)?;
//! for c in cases.iter().filter(|c| c.targets("egui")) {
//!     let (outcome, _) = svelters_testkit::egui::run(c, &c.bundle(&dir.join("dist"), "egui"), None);
//! }
//! ```

pub mod case;
#[cfg(feature = "egui")]
pub mod egui;
pub mod log;
pub mod oplog;
pub mod ratchet;
pub mod run;
/// SCHED1: an egui [`svelters_egui::harness::TestApp`] driven by the deterministic scheduler.
#[cfg(feature = "egui")]
pub mod sched;
pub mod view;

pub use case::{Case, Expect, Step};
pub use run::{Outcome, Target, run_case};
pub use view::ViewNode;

use std::path::{Path, PathBuf};

/// svelters' own cases: `tests/cases` in this repository.
pub fn repo_cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cases")
}

/// Where per-backend results go: `<workspace>/target/testkit`.
pub fn results_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
        .join("testkit")
}
