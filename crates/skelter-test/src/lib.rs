//! skelter's test harness (`plans/research/skelter/architecture.md` §16; lanes APP-TEST, TEST-V2).
//!
//! | Module | What |
//! |---|---|
//! | [`harness`] | [`Harness`]: kittest's verbs (`run`, `step`, `get_by_label`, `click`, `key_press`, …) on a real `skelter::App` |
//! | [`core_islands`] | [`CoreIslands`]: a skelter-core document with islands mounted through `IslandHost` |
//! | [`island`] | [`EguiHarness`]: egui_kittest's `Harness` for egui closures in a skelter egui island |
//! | [`headless`] | the GPU-less compositor the harness runs on (with `skelter_platform::HeadlessPlatform`) |
//! | [`a11y`] | the AccessKit snapshot (grafted island subtrees included) and kittest's queries |
//! | [`convert`] | verb arguments in skelter's or egui's types |
//! | [`egui_event`] | the `egui::Event`-shaped injection adapter for tests moving off egui_kittest |
//! | [`render`] | pixels: the committed tree drawn by `skelter_render` on the environment's adapter (lavapipe in CI) |
//! | [`snapshot`] | the pixel tier: kittest's `snapshot` (dify's comparison, PNG goldens, `UPDATE_SNAPSHOTS`) |
//! | `automation` | (feature `automation`) `SkelterNativeDriver` (skelter-native UI) and `SkelterDriver` (svelters-egui in an island): a `svelters_automation::Session` (selectors, acts, waits, journal, MCP methods) on a harness |
//! | `scene_source` | (feature `automation`) `SkelterScene`: skelter-native UI as a `svelters_automation::SceneSource` (selectors, snapshot) |

pub mod a11y;
#[cfg(feature = "automation")]
pub mod automation;
pub mod convert;
pub mod core_islands;
pub mod egui_event;
pub mod harness;
pub mod headless;
pub mod island;
pub mod render;
#[cfg(feature = "automation")]
pub mod scene_source;
pub mod snapshot;

pub use a11y::{A11yTree, AkNode, By, NodeInfo, NodeKey, by};
pub use convert::TestEvent;
pub use core_islands::CoreIslands;
pub use harness::{Harness, HarnessBuilder, Node, NotSettled};
pub use headless::HeadlessCompositor;
pub use island::{EguiHarness, EguiHarnessBuilder, IslandDoc};
pub use skelter::platform::{HeadlessPlatform, PlatformCall, VirtualClock};
pub use skelter_base::accesskit::Role;
pub use render::{GpuRender, Image, RenderError};
#[cfg(feature = "automation")]
pub use scene_source::SkelterScene;
#[cfg(feature = "automation")]
pub use automation::{SkelterDriver, SkelterNativeDriver};
pub use snapshot::{SnapshotError, SnapshotOptions, image_snapshot, try_image_snapshot, try_image_snapshot_options};
