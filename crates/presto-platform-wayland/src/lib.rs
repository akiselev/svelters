//! presto's own Wayland platform backend (`plans/research/presto/platform.md` §5, package W-P4), on
//! smithay-client-toolkit and wayland-client. It implements the presto-platform API ([`shim`] until lane
//! PLATFORM's trait lands) directly on the compositor's protocols, without winit:
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the connection, globals, the event loop ([`run`]), and [`app::Cx`], the `PlatformCx` |
//! | [`window`] | xdg toplevels and popups, configure, fractional scale with `wp_viewporter`, decorations |
//! | [`seat`] | pointer, keyboard (xkb), touch, tablet, pinch, cursor shapes, key repeat |
//! | [`frames`] | input frame assembly (pointer frames, axis sources and `axis_stop`, touch, pen) |
//! | [`keymap`] | keysyms → egui-named keys, evdev → USB HID usages |
//! | [`xkb`] | libxkbcommon (loaded at runtime): keymap, modifiers, text, compose |
//! | [`ime`] | `zwp_text_input_v3`: surrounding text, purpose, hints, the `done` batches |
//! | [`csd`] | our client-side decorations: hit-test regions → move, resize, window menu |
//! | [`layers`] | OS layers as subsurfaces, locked with the toplevel's state |
//! | [`present`] | the frame source: frame callbacks, `wp_presentation`, `fifo-v1`, `commit-timing-v1`, 100 ms rule |
//! | [`data`] | clipboard, primary selection, drag and drop, `xdg-toplevel-drag-v1` tear-off |
//! | [`portal`] | the XDG desktop portal (ashpd): colour scheme, accent, file dialogs |
//! | [`time`] | presto time on `CLOCK_MONOTONIC` and Wayland timestamps |
//!
//! Every module compiles only on the Unix desktops Wayland runs on; elsewhere the crate is empty.

#![cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]

pub mod app;
pub mod csd;
pub mod cx;
pub mod data;
pub mod frames;
pub mod ime;
pub mod keymap;
pub mod layers;
#[cfg(feature = "portal")]
pub mod portal;
pub mod present;
pub mod seat;
pub mod shim;
pub mod time;
pub mod window;
pub mod xkb;

pub use app::run;
