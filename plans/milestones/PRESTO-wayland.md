# PRESTO-wayland: presto-platform-wayland (presto batch 3, lane WAYLAND)

Date: 2026-10-25. Branch `ci/presto-wayland` from `ci/presto-base` efe20dd5. Session
`session_01JDnE12Ws2QHwSfPYzesDaR`. Brief: `plans/cloud/presto-b3/LANE-wayland.md`.

## What landed

`crates/presto-platform-wayland` (~5.9k lines, Linux/BSD only; other OSes build an empty crate). It is
presto's own Wayland backend on smithay-client-toolkit 0.20 and wayland-client 0.31, with no winit and no
calloop. It has its own `poll` loop on the Wayland socket and a wake pipe.

| Module | What |
|---|---|
| `app` | the connection, globals, `run`, the event loop, and `Pending` events delivered between dispatches |
| `cx` | `Cx`, the `PlatformCx` (and `Clipboard`) the app calls back into |
| `window` | xdg toplevels and popups (xdg_positioner, grabs), configure, min/max, fractional scale + viewporter, integer-scale fallback, decoration mode, focus (ACTIVATED), occlusion (SUSPENDED) |
| `seat` | pointer, keyboard, touch, tablet tools (tablet-v2), pinch (pointer-gestures), text-input-v3 dispatch, cursor-shape-v1 with an xcursor-theme fallback, client-side key repeat (and v10 compositor repeat) |
| `frames` | pure input frame assembly: pointer frames, axis sources, `axis_stop`, value120/discrete, history, touch frames, pen frames, pinch deltas |
| `keymap` | keysym → egui-named `Key` (physical fallback for non-Latin layouts, as egui-winit), evdev → USB HID usage (`page << 16 \| usage`), key locations |
| `xkb` | libxkbcommon via `xkbcommon-dl` (dlopen, no link dependency): keymap from the compositor fd (pread), modifiers incl. AltGr (Mod5), text, dead-key compose |
| `ime` | text-input-v3: `done`-batch ordering (preedit/delete/commit/preedit), content type and hints, 4000-byte surrounding clamp, UTF-16↔UTF-8, `apply_to_client` bridge to `TextInputClient` |
| `csd` | our CSD: hit regions → `move`/`resize`/`show_window_menu`/maximize/minimize/close, double click, resize cursors |
| `layers` | OS layers as `wl_subsurface`s with their own viewport, an empty input region, desync normally |
| `present` | `SurfaceLock` (buffer-size-keyed viewport/buffer-scale/window-geometry applied in `before_present`; children synced during resize, desynced after the parent's first present at the new size) and `WaylandFrameSource` (own event queue; frame callbacks as vsync, a bare-commit tick when idle, `wp_presentation` feedback, the 100 ms occlusion rule, `fifo-v1`/`commit-timing-v1` opt-in) |
| `data` | clipboard + primary selection (our own text served from memory, reads with a 500 ms timeout), file DnD (`text/uri-list` → FileHovered/Dropped/HoverCancelled), drag source, internal drags fed back as pointer events, `xdg-toplevel-drag-v1` tear-off |
| `portal` (feature `portal`, default) | ashpd 0.13 on async-io (no runtime): colour-scheme watcher → `ThemeChanged`, `accent_color`, blocking `open_files`/`save_file` |
| `shim` | the platform API, mirrored from lane PLATFORM's `api.rs`/`clipboard.rs` (base `ac7cc8c4`) plus marked additions |

Tests written, none run, as the brief says: key mapping (`keymap`), input frame assembly (`frames`), time
conversion, CSD, IME batches and the client bridge, buffer sizes, URI lists, cursor tables, and the shim's
UTF-16 conversion.

## Not run

Only `cargo check -p presto-platform-wayland --all-targets` and `--no-default-features` ran (clean, no
warnings). I ran no tests, no clippy, and nothing against a compositor. CI results are below.

## Stubbed or left out (honestly)

- `update_a11y`: ignored. AccessKit on Linux is AT-SPI (accesskit_unix) and doesn't depend on the
  display protocol, so the consolidation should share presto-platform's adapter.
- `set_visible`: ignored. xdg-shell cannot hide a mapped toplevel.
- `set_mouse_capture`: ignored, and `raw_mouse_motion` is never sent. pointer-constraints and
  relative-pointer are not bound yet; this is the next item for 3D-view capture.
- `os_layer_committer`: `None`. Subsurface state applies with the parent's commit.
- `WindowDesc.position` and `os_compositing` are ignored (Wayland has no client positioning). `visible:
  false` is ignored too.
- Popups are not repositioned when their size changes (`xdg_popup.reposition` is not used).
- File-dialog parenting: there is no xdg-foreign handle, so dialogs are modal to nothing.
- X11 is not covered; it stays on winit.
- Tablet pads (rings, strips, buttons) are bound but their events are dropped.

## Questions for the coordinator (with recommendations)

1. **Where the platform API lives.** presto-platform will select this backend with a cargo feature, so
   this crate cannot depend on presto-platform (cycle). *Recommend:* move `presto-platform/src/api.rs` +
   `clipboard.rs` (the trait, data types, `Clipboard`) into a small `presto-platform-api` crate (or a
   `platform` module of presto-base) that presto-platform and the three own backends depend on; then
   `shim.rs` becomes `pub use presto_platform_api::*`.
2. **Additions to PLATFORM's API** that this backend needs (marked "Addition" in `shim.rs`):
   - `HitKind::Resize*`: CSD draws the frame on Wayland.
   - `PlatformCx::frame_source` and the `FrameSource`/`FrameSignal` trait: PLATFORM's docs list a `frame`
     module but base has none yet; `FrameSignal` maps 1:1 onto presto-frame's `Signal`.
   - `client_side_decorations`, `set_window_state` (incl. `Activate` via xdg-activation), `start_drag`,
     `attach_window_to_drag` and `AppHandler::drag_ended` for tear-off.
   - `WindowDesc.popup`/`parent`, and `ImeHints` in `ImeState`.

   *Recommend:* add them to the shared API as written.
3. **Surrounding-text offsets disagree.** presto-base's `TextInputClient::surrounding_text` returns
   UTF-8 byte offsets, while PLATFORM's `ImeState.surrounding` is UTF-16. I follow PLATFORM (and convert
   in `ImeState::from_client`). *Recommend:* make both UTF-16, as the rest of `TextInputClient` is.
4. **Physical key codes** are `page << 16 | usage` HID usages (keyboard page 0x07, consumer page 0x0C;
   `0xFFFF_0000 | evdev` when unknown). *Recommend:* all backends use this encoding; please confirm with
   PLATFORM/WIN32/MACOS.
5. **fifo-v1 / commit-timing-v1 and Mesa.** Mesa's Vulkan WSI creates its own `wp_fifo_v1` for FIFO
   present mode, and a second one on the surface is a protocol error that kills the connection. Both
   are therefore opt-in (`FrameSourceConfig`, default off). *Recommend:* the compositor enables `fifo`
   only with a Mailbox/Immediate swapchain, and `commit_timing` only after checking that Mesa does not
   already hold a timer (Mesa 25.1+ with present-timing).
6. **Bare-commit ticks** (frame callbacks while nothing presents) also apply a pending
   `xdg_surface.ack_configure` (sctk acks immediately) with the old buffer. This is suppressed while a
   resize is pending; it is harmless on KWin/Mutter as far as I know, but it needs the hardware checks in
   `platform.md` §8.
7. **Multi-window frame waiting.** All frame sources share one socket. `FrameSource::wait` on one source
   wakes for any event, and the compositor then `poll`s every source. *Recommend:* the compositor thread
   waits on one source per loop and polls all of them.

## Cross-lane edits

None. `Cargo.lock` gained the new dependencies: ashpd 0.13 (async-io, settings, file_chooser),
wayland-cursor, xkbcommon-dl, wayland-protocols staging/unstable, and wayland-backend `client_system` +
`dlopen`.
