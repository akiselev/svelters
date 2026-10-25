//! presto's test harness (`plans/research/presto/architecture.md` §16; lane APP-TEST).
//!
//! | Module | What |
//! |---|---|
//! | [`harness`] | [`Harness`]: kittest's verbs (`run`, `step`, `get_by_label`, `click`, `key_press`, …) on a real `presto::App` |
//! | [`headless`] | the GPU-less compositor the harness runs on (with `presto_platform::HeadlessPlatform`) |
//! | [`a11y`] | the AccessKit snapshot and its queries (by label, role, value, author id) |
//! | [`egui_event`] | the `egui::Event`-shaped injection adapter for tests moving off egui_kittest |
//!
//! Not here yet: `render` (pixels through the compositor on lavapipe) waits for `presto-compositor`.

pub mod a11y;
pub mod egui_event;
pub mod harness;
pub mod headless;

pub use a11y::{A11yTree, By, NodeInfo};
pub use harness::{Harness, HarnessBuilder, Node, NotSettled};
pub use headless::HeadlessCompositor;
pub use presto::platform::{HeadlessPlatform, PlatformCall, VirtualClock};
pub use presto_base::accesskit::Role;
