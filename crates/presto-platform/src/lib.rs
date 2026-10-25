//! presto's platform layer (`plans/research/presto/platform.md`): windows, input, IME, clipboard,
//! accessibility, OS layers and frame sources behind one interface ([`AppHandler`] / [`PlatformCx`]).
//!
//! Phase 1 is winit 0.30.13 ([`run`]) plus per-OS shims ([`os`]) for what winit lacks; own backends
//! per OS replace it later behind the same interface. [`HeadlessPlatform`] runs an app without an OS
//! for tests: scripted input, a virtual clock, no windows.
//!
//! | Module | What |
//! |---|---|
//! | [`api`] | the app ↔ platform interface and its data (window and layer descriptions, IME state, hit regions, monitors) |
//! | [`backend`] | the winit event loop: translation, timers, wakes, windows |
//! | [`translate`], [`keys`] | winit events and keys → presto events and egui-named keys |
//! | [`os`] | Windows, macOS and Wayland shims |
//! | [`clipboard`] | clipboard and primary selection |
//! | [`a11y`] | AccessKit platform adapters |
//! | [`frame`] | vsync and presentation sources for the compositor thread |
//! | [`headless`] | the test platform |
//! | [`clock`] | presto's real clock and OS timestamp conversion |

pub mod a11y;
pub mod api;
pub mod backend;
pub mod clipboard;
pub mod clock;
pub mod headless;
pub mod keys;
pub mod os;
pub mod translate;

pub use api::*;
pub use backend::run;
pub use clipboard::{Clipboard, ClipboardKind, MemoryClipboard, SystemClipboard};
pub use headless::{HeadlessPlatform, PlatformCall, Scripted, VirtualClock};
