//! svelters-winui: the WinUI 3 backend.
//!
//! Element kinds, props, events, slots and child attributes are generated from the Windows App
//! SDK metadata (`metadata/winui`, `metadata/overrides/winui`) by `tools/metadata/winui`. The
//! native calls go through windows-rs bindings generated from `Microsoft.UI.Xaml.winmd` with a
//! member-level filter (`bindings.filter` -> `src/bindings.rs`).
//!
//! Hosting: XAML is initialized on the calling thread in [`WinUiBackend::new`] (a dispatcher
//! queue, an `Application` with `XamlControlsResources`), and `run` is a Win32 message loop that
//! forwards keyboard input through `ContentPreTranslateMessage`. `window` is
//! `Microsoft.UI.Xaml.Window`. See `docs/notes/win1-winui.md`.
//!
//! Only the platform-neutral parts ([`table`], [`names`], [`parse`]) exist on other targets.

pub mod harness;
pub mod parse;
pub mod table;
pub mod types;

#[path = "generated/names.rs"]
pub mod names;

#[cfg(windows)]
#[allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code, unused_imports, clippy::all)]
#[rustfmt::skip]
mod bindings;

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod conv;
#[cfg(windows)]
mod custom;
#[cfg(windows)]
#[path = "generated/dispatch.rs"]
mod dispatch;
#[cfg(windows)]
mod sink;
#[cfg(windows)]
pub mod testing;

#[cfg(windows)]
pub use backend::WinUiBackend;
