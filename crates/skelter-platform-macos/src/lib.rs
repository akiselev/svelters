//! skelter's own macOS platform backend (`plans/research/skelter/platform.md` §5, W-P5): AppKit windows
//! with our own `NSWindow` and `NSView` subclasses, the full `NSTextInputClient` over skelter's
//! [`TextInputClient`], keys named as egui names them (`UCKeyTranslate`, the Option-as-Alt policy),
//! mouse and tablet pen input with coalescing control, precise scrolling with gesture and momentum
//! phases, magnify and rotate, one `CAMetalLayer` per window plus OS-layer sublayers, display-link
//! frame sources, live resize with `presentsWithTransaction`, the app delegate (Cmd-Q veto, reopen),
//! an AccessKit host view, the menu bar through muda, cursors and the clipboard.
//!
//! It implements `skelter_platform`'s [`AppHandler`] / [`PlatformCx`] and its `FrameSource` (see
//! [`display_link`]). macOS extras are [`MacAppHandler`] callbacks and the
//! main-thread functions below.
//!
//! | Module | What |
//! |---|---|
//! | [`keymap`] | key codes, HID usages, modifiers, the Option policy, layout key naming (no AppKit) |
//! | [`text_input`] | `NSTextInputClient` semantics, key routing, the client-less IME (no AppKit) |
//! | `app` | `NSApplication`, the delegate, state, the event queue, [`PlatformCx`] |
//! | `window`, `view` | the window and view classes, input, IME, accessibility, drops |
//! | [`display_link`] | frame sources |
//! | `layer`, `cursor`, `clipboard`, `monitor`, `menu`, `a11y`, `keyboard`, `events`, `time` | the rest |
#![cfg(target_os = "macos")]

mod a11y;
mod app;
mod clipboard;
mod cursor;
pub mod display_link;
mod events;
mod keyboard;
pub mod keymap;
mod layer;
mod menu;
mod monitor;
pub mod text_input;
mod time;
mod view;
mod window;

pub use a11y::a11y_update;
pub use app::{MacAppHandler, MacConfig, run, run_mac};
pub use display_link::{
    FrameSourceConfig, FrameSourceKind, MacFrameSource, MetalDrawable, VsyncTick,
};
pub use keymap::OptionAsAlt;
pub use menu::set_menu_bar;
pub use muda;
pub use skelter_platform::{AppHandler, PlatformCx, PlatformError};
pub use time::MachClock;

use objc2::MainThreadMarker;
use skelter_base::ids::WindowId;
use skelter_base::input::TextInputClient;

/// The backend as the `skelter` facade picks it: `MacPlatform::new().run(handler)` is the macOS
/// counterpart of `skelter_platform::run`.
#[derive(Clone, Debug, Default)]
pub struct MacPlatform {
    pub config: MacConfig,
}

impl MacPlatform {
    pub fn new() -> Self {
        MacPlatform::default()
    }

    pub fn with_config(config: MacConfig) -> Self {
        MacPlatform { config }
    }

    /// Runs `handler` until it exits. Main thread only.
    pub fn run(self, handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
        app::run_with(handler, self.config)
    }

    /// [`MacPlatform::run`] with the macOS callbacks.
    pub fn run_mac(self, handler: Box<dyn MacAppHandler>) -> Result<(), PlatformError> {
        run_mac(handler, self.config)
    }
}

/// Gives `window` a full text-input client (architecture §11.3): the IME then reads and edits the
/// document through it (reconversion, accents, dictation, the Character Viewer, per-character
/// candidate placement), instead of reporting [`ImeEvent`](skelter_base::event::ImeEvent)s as
/// [`PlatformCx::set_ime`] does. Key presses the IME passes through still arrive as key events with
/// their text. The client is called on the main thread outside handler calls; it must not assume the
/// handler is idle. Main thread.
pub fn set_text_input_client(window: WindowId, client: Option<Box<dyn TextInputClient>>) {
    let Some(view) = app::with_window(window, |w| {
        w.has_client = client.is_some();
        w.client = client;
        w.view.clone()
    }) else {
        return;
    };
    window::invalidate_character_coordinates(&view);
}

/// Tells the input method that the client's text or selection changed outside it (the app edited the
/// document, the caret moved): it drops its composition state. Main thread.
pub fn text_input_changed(window: WindowId) {
    if let Some(view) = app::with_window(window, |w| w.view.clone()) {
        window::discard_marked_text(&view);
        window::invalidate_character_coordinates(&view);
    }
}

pub fn set_option_as_alt(policy: OptionAsAlt) {
    app::with_app(|st| st.option_as_alt = policy);
}

/// AppKit's mouse-move coalescing (off by default: every sample arrives).
pub fn set_mouse_coalescing(enabled: bool) {
    objc2_app_kit::NSEvent::setMouseCoalescingEnabled(enabled);
}

/// Puts `window`'s content under a transparent title bar (the window controls stay), for app-drawn
/// title bars marked with [`PlatformCx::set_hit_test_regions`]. Main thread.
pub fn set_custom_title_bar(window: WindowId, on: bool) {
    if let Some(w) = app::with_window(window, |w| w.ns_window.clone()) {
        window::set_custom_title_bar(&w, on);
    }
}

/// Presents through the Core Animation transaction (the main thread then presents; `platform.md` §4).
/// The platform turns it on for live resizes by itself; the compositor may also want it around
/// synchronous frames (key-window changes).
pub fn set_presents_with_transaction(window: WindowId, on: bool) {
    app::with_window(window, |w| w.layer.setPresentsWithTransaction(on));
}

pub fn in_live_resize(window: WindowId) -> bool {
    app::with_window(window, |w| w.live_resize).unwrap_or(false)
}

/// Creates `window`'s frame source (an `Arc` of it goes to `CompositorHandle::set_frame_source`); the
/// window's occlusion changes reach it as `FrameWait::Occluded`. Main thread.
pub fn frame_source(
    window: WindowId,
    config: FrameSourceConfig,
) -> Result<MacFrameSource, PlatformError> {
    let mtm =
        MainThreadMarker::new().ok_or_else(|| PlatformError::Os("not the main thread".into()))?;
    let (view, layer) = app::with_window(window, |w| (w.view.clone(), w.layer.clone()))
        .ok_or_else(|| PlatformError::Os(format!("unknown window {window:?}")))?;
    let source = MacFrameSource::new(mtm, &view, &layer, config)?;
    let handle = source.occlusion_handle();
    app::with_window(window, |w| w.frame_sources.push(handle));
    Ok(source)
}
