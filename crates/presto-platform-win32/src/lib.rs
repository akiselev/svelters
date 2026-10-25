//! presto's own Win32 platform backend (`plans/research/presto/platform.md` §5, package W-P3).
//!
//! It implements presto-platform's [`presto_platform::AppHandler`] / [`presto_platform::PlatformCx`]
//! contract without winit:
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the event loop, the handler and its re-entrancy queue, wakes, timers, the modal-loop policy |
//! | [`window`] | the window class and procedure, DPI, custom frames, the per-window state |
//! | [`pointer`] | `WM_POINTER` for mouse, pen and touch with history and QPC timestamps |
//! | [`keyboard`] | scancodes, layout characters, dead keys, AltGr, egui key names |
//! | [`tsf`] | the TSF text store (`ITextStoreACP`) over [`presto_base::input::TextInputClient`] |
//! | [`shadow`] | the IME's copy of the focused field, built from `ImeState`, editing through `ImeEvent`s |
//! | [`imm`] | `WM_IME_REQUEST` (reconversion, document feed, character positions) for IMM32 TIPs |
//! | [`dmanip`] | DirectManipulation: precision touchpad pan, pinch and inertia |
//! | [`dcomp`] | the DirectComposition device and per-window visual tree |
//! | [`frame`] | the frame source: the compositor clock, DXGI waitable, `DwmFlush` |
//! | [`hit_test`] | `WM_NCHITTEST` from the app's hit regions (snap layouts) |
//! | [`drag`] | OLE drag source and drop target |
//! | [`clipboard`], [`cursor`], [`monitor`] | the rest of the OS surface |
//!
//! The crate is empty on other OSes.

#![cfg(target_os = "windows")]

pub mod app;
pub mod clipboard;
pub mod cursor;
pub mod dcomp;
pub mod dmanip;
pub mod drag;
pub mod frame;
pub mod hit_test;
pub mod imm;
pub mod keyboard;
pub mod monitor;
pub mod pointer;
pub mod shadow;
pub mod tsf;
pub mod util;
pub mod window;

pub use app::run;

use presto_platform::{AppHandler, PlatformError};

/// The backend as the `presto` facade selects it: `Win32Platform::new().run(handler)`, in place of
/// presto-platform's winit `run`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Win32Platform;

impl Win32Platform {
    pub fn new() -> Self {
        Win32Platform
    }

    /// Runs the app on this thread until it exits (see [`app::run`]).
    pub fn run(self, handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
        app::run(handler)
    }
}
