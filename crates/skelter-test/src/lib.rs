//! skelter's test harness (`plans/research/skelter/architecture.md` §16; lanes APP-TEST, TEST-V2).
//!
//! | Module | What |
//! |---|---|
//! | [`harness`] | [`Harness`]: kittest's verbs (`run`, `step`, `get_by_label`, `click`, `key_press`, …) on a real `skelter::App` |
//! | `core_islands` | (feature `egui`) `CoreIslands`: a skelter-core document with islands mounted through `IslandHost` |
//! | `island` | (feature `egui`) `EguiHarness`: egui_kittest's `Harness` for egui closures in a skelter egui island |
//! | [`headless`] | the GPU-less compositor the harness runs on (with `skelter_platform::HeadlessPlatform`) |
//! | [`a11y`] | the AccessKit snapshot (grafted island subtrees included) and kittest's queries |
//! | [`consumer`] | AccessKit's own consumers on the app's updates: an `accesskit_consumer::Tree` and, on Linux, an AT-SPI adapter recording interfaces and events |
//! | [`convert`] | verb arguments in skelter's types (and egui's, feature `egui`) |
//! | [`egui_event`] | the injection adapter's state; with feature `egui`, the `egui::Event`-shaped adapter for tests moving off egui_kittest |
//! | [`render`] | pixels: the committed tree drawn by `skelter_render` on the environment's adapter (lavapipe in CI) |
//! | [`snapshot`] | the pixel tier: kittest's `snapshot` (dify's comparison, PNG goldens, `UPDATE_SNAPSHOTS`) |
//! | `automation` | (feature `automation`) `SkelterNativeDriver` (skelter-native UI, no egui) and `SkelterDriver` (feature `automation-egui`: svelters-egui in an island): a `svelters_automation::Session` (selectors, acts, waits, journal, MCP methods) on a harness |
//! | `scene_source` | (feature `automation`) `SkelterScene`: skelter-native UI as a `svelters_automation::SceneSource` (selectors, snapshot) |

pub mod a11y;
#[cfg(feature = "automation")]
pub mod automation;
pub mod consumer;
pub mod convert;
#[cfg(feature = "egui")]
pub mod core_islands;
pub mod egui_event;
pub mod harness;
pub mod headless;
#[cfg(feature = "egui")]
pub mod island;
pub mod render;
#[cfg(feature = "automation")]
pub mod scene_source;
pub mod snapshot;

pub use a11y::{A11yTree, AkNode, By, NodeInfo, NodeKey, by};
pub use consumer::{Consumer, FocusMove, Located};
pub use convert::TestEvent;
#[cfg(feature = "egui")]
pub use core_islands::CoreIslands;
pub use harness::{Harness, HarnessBuilder, Node, NotSettled};
pub use headless::HeadlessCompositor;
#[cfg(feature = "egui")]
pub use island::{EguiHarness, EguiHarnessBuilder, IslandDoc, Registrations, register_textures};
pub use skelter::platform::{HeadlessPlatform, PlatformCall, VirtualClock};
pub use skelter_base::accesskit::Role;
pub use render::{GpuRender, Image, RenderError};
#[cfg(feature = "automation")]
pub use scene_source::SkelterScene;
#[cfg(feature = "automation")]
pub use automation::SkelterNativeDriver;
#[cfg(feature = "automation-egui")]
pub use automation::SkelterDriver;
pub use snapshot::{SnapshotError, SnapshotOptions, image_snapshot, try_image_snapshot, try_image_snapshot_options};
