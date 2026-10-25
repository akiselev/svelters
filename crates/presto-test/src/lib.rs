//! presto's test harness (`plans/research/presto/architecture.md` §16; lanes APP-TEST, TEST-V2).
//!
//! | Module | What |
//! |---|---|
//! | [`harness`] | [`Harness`]: kittest's verbs (`run`, `step`, `get_by_label`, `click`, `key_press`, …) on a real `presto::App` |
//! | [`island`] | [`EguiHarness`]: egui_kittest's `Harness` for egui closures in a presto egui island |
//! | [`headless`] | the GPU-less compositor the harness runs on (with `presto_platform::HeadlessPlatform`) |
//! | [`a11y`] | the AccessKit snapshot (grafted island subtrees included) and kittest's queries |
//! | [`convert`] | verb arguments in presto's or egui's types |
//! | [`egui_event`] | the `egui::Event`-shaped injection adapter for tests moving off egui_kittest |
//! | [`render`] | pixels: the committed tree drawn by `presto_render` on the environment's adapter (lavapipe in CI) |
//! | [`snapshot`] | the pixel tier: kittest's `snapshot` (dify's comparison, PNG goldens, `UPDATE_SNAPSHOTS`) |
//! | `automation` | (feature `automation`) `svelters_automation::Driver` for a harness, so selector and MCP queries drive presto |

pub mod a11y;
#[cfg(feature = "automation")]
pub mod automation;
pub mod convert;
pub mod egui_event;
pub mod harness;
pub mod headless;
pub mod island;
pub mod render;
pub mod snapshot;

pub use a11y::{A11yTree, AkNode, By, NodeInfo, NodeKey, by};
pub use convert::TestEvent;
pub use harness::{Harness, HarnessBuilder, Node, NotSettled};
pub use headless::HeadlessCompositor;
pub use island::{EguiHarness, EguiHarnessBuilder, IslandDoc};
pub use presto::platform::{HeadlessPlatform, PlatformCall, VirtualClock};
pub use presto_base::accesskit::Role;
pub use render::{GpuRender, Image, RenderError};
pub use snapshot::{SnapshotError, SnapshotOptions, image_snapshot, try_image_snapshot, try_image_snapshot_options};
