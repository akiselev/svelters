# PRESTO-macos: presto's own macOS platform backend (presto batch 3, lane MACOS)

Branch `ci/presto-macos` (from `ci/presto-base` at efe20dd5; merged `ci/presto-base` through d58a0fa0: presto-platform's API and frame-source trait). Session `session_01TjQo3Ks6gxG7GdDLdgUZ86`.
Brief: `plans/cloud/presto-b3/LANE-macos.md`. Crate: `crates/presto-platform-macos` (empty on other OSes:
`#![cfg(target_os = "macos")]` at the crate root).

## What landed

| Module | What |
|---|---|
| (trait) | Implements `presto_platform::{AppHandler, PlatformCx}` as merged into `ci/presto-base` (all methods, including `set_os_layer_bounds`, `remove_os_layer`, `os_layer_committer` → `None` (CA transactions need no commit), `end_ime_composition`, `set_mouse_capture` (CGAssociateMouseAndMouseCursorPosition + hidden cursor, moves become `raw_mouse_motion`), `set_title`, `set_visible`, `drag_window`, `update_a11y`, `window_scale`; `quit_requested`/`exiting`/`a11y_event` delivered). `PlatformCx::frame_source` returns a `MacFrameSource`. |
| `app` | `PrestoApplication` (NSApplication subclass: forwards Cmd+keyUp, which AppKit swallows), `PrestoAppDelegate` (Cmd-Q / Quit always vetoed → `MacAppHandler::quit_requested`, default sends `CloseRequested` to every window; reopen → `MacAppHandler::reopen`), main-thread state, the re-entrancy-safe event queue (every callback posts; the handler is never re-entered), `PlatformCx` (windows, OS layers, cursor, IME state, clipboard, `NSTimer` timers in common modes, GCD-based coalescing `Waker`, clock, monitors, hit regions, exit via `NSApp.stop`). `run_mac(handler, MacConfig)` adds the macOS callbacks; `run` wraps a plain `AppHandler`. |
| `window` | `PrestoWindow` (NSWindow subclass: borderless windows can be key; forwards `accessibilityFocusedUIElement` to the view), `PrestoWindowDelegate` (close → `CloseRequested`, move, focus + AccessKit focus, occlusion), `WindowDesc` → style mask, custom title bar (transparent, full-size content), transparency, min/max size, position in global top-left points. |
| `view` | `PrestoView`: layer-hosting `CAMetalLayer`; mouse (all buttons, enter/leave via a tracking area), tablet pen (`tabletPoint:`/`tabletProximity:`, subtype TabletPoint: pressure, tilt, pointer id per device), precise scrolling with `phase`/`momentumPhase`, magnify, rotate, smart magnify; keys with IME-first routing; the full `NSTextInputClient` (marked text, `attributedSubstringForProposedRange:actualRange:`, `firstRectForCharacterRange:actualRange:` per range, `characterIndexForPoint:`, `replacementRange`, `selectedRange` never NSNotFound in simple mode, `unmarkText`, `doCommandBySelector:`); AccessKit host methods; file drops; size/scale/theme reporting; live resize toggling `presentsWithTransaction`; caption hit regions (window drag, double-click per `AppleActionOnDoubleClick`, `_opaqueRectForWindowMoveWhenInTitlebar`); the standard Edit actions (`copy:` etc.) as the Cmd shortcuts they stand for. |
| `keymap` (pure) | `kVK_*` → egui `Key`, USB HID usage (`0x07xxxx`), location; `NSEventModifierFlags` → `Modifiers`; modifier-key press/release from `flagsChanged:` device bits; Option-as-Alt policy; GPUI's layout key naming (Cmd layers of Dvorak-QWERTY, non-ASCII layouts, shifted digits); typed text filter. Tests. |
| `keyboard` | `TISCopyCurrentKeyboardLayoutInputSource` + `UCKeyTranslate` (dead keys as their spacing character). |
| `text_input` (pure) | `NSRange` semantics over `TextInputClient` (`ClientTarget`) or the client-less `SimpleIme` (reports `ImeEvent::Preedit`/`Commit`), key routing, insert classification (the IME's pass-through of a key vs. real text), UTF-16 helpers. Tests with a mock editor (composition, clamping, press-and-hold replacement, reconversion ranges, geometry). |
| `display_link` | `MacFrameSource` implements `presto_platform::frame::FrameSource` (`wait` blocks on a condvar fed by ticks; a paused link resumes on `wait`, CA links pause themselves after 500 ms without a waiter; window occlusion arrives as `FrameWait::Occluded`; no presentation feedback yet, `take_presented` is empty): `CAMetalDisplayLink` (14+, latency 1, `targetPresentationTimestamp`, optional drawable sink for wgpu P3), `NSView.displayLink` (14+), `CVDisplayLink` (≤13). CA links run on their own thread and run loop; pause/teardown are blocks performed there; the CV context is leaked on purpose (late callbacks are harmless). |
| `layer`, `cursor`, `clipboard`, `monitor`, `menu`, `a11y`, `time` | GPUI-style `CAMetalLayer` config, OS-layer sublayers (flipped geometry, transactions without actions); `NSCursor` mapping (private shapes only when the class answers them); `NSPasteboard` text; `NSScreen` in global top-left points; muda menu bar (`set_menu_bar`, default App + Edit menu, events → `MacAppHandler::menu`); AccessKit adapter host (inactive / placeholder / active, full trees kept until the first request, `a11y_update`); mach clock (`NSEvent` and CA timestamps convert by scaling). |

Public main-thread extras: `set_text_input_client`, `text_input_changed`, `set_option_as_alt`,
`set_mouse_coalescing`, `set_custom_title_bar`, `set_presents_with_transaction`,
`in_live_resize`, `frame_source`, `a11y_update`, `set_menu_bar`.

## Not done / stubbed (nothing returns a fake success)

- **Main-thread present during live resize**: the platform sets `presentsWithTransaction` and delivers
  `Resized` synchronously from `setFrameSize:`; handing the `SurfaceTexture` back to the main thread to
  present is the compositor's (and wgpu P4's) part.
- **Pointer history** is empty: macOS has no coalesced-events API; coalescing is off by default
  (`MacConfig::mouse_coalescing`) so every sample arrives as its own event.
- **CVDisplayLink** follows the main display only (no retargeting on screen change); CA links follow the
  layer/view.
- No drag source, no file dialogs (rfd), no notifications, no accent colour, no `surrounding_text` use
  (AppKit asks through `attributedSubstring` instead), no `InputPurpose` → secure input (password fields
  should call `EnableSecureEventInput`; left for the editor lane to decide).
- In simple IME mode (no client) `replacementRange` cannot be honoured (no document); press-and-hold
  accents need a client.

## Not run / CI

- Nothing ran on a Mac interactively: windows, IME (Japanese/Chinese/Korean, dead keys, press-and-hold,
  dictation, Character Viewer), VoiceOver, tablets, display links and live resize are unverified on
  hardware (platform.md §8 checks).
- Local: `cargo check -p presto-platform-macos --target aarch64-apple-darwin --all-targets` (cross-check,
  no link) and the Linux build (empty crate).
- CI presto run 40226739671 on bef22840: `cargo check` passed on macos-latest, ubuntu and windows;
  `presto-platform-macos` unit tests passed on macOS. `cargo test` failed on other lanes' targets
  (`presto-compositor --test engine`, `--test tree_damage`, `presto-core --lib`, `presto-widgets --lib`:
  slider tests), not this crate.
- The facade constructor is `MacPlatform::new().run(handler)` (and `run_mac` with the macOS callbacks).

## Additions outside the crate

- Decided by the coordinator (2026-10-25), made here as additive changes:
  - `presto_platform::PlatformCx::set_text_input(id, Option<Box<dyn TextInputClient>>)`, whose default
    falls back to `set_ime` with the client's caret rect and purpose; this backend bridges it to
    `NSTextInputClient`.
  - `WindowDesc.custom_title_bar` (default false); this backend applies it at creation.
  - Native menu bar (asked by WIDGETS-VIEWS through the coordinator): `presto_platform::MenuDesc
    { menus: Vec<MenuNode> }`, `MenuNode::{Submenu { label, enabled, items }, Item { id, label,
    shortcut, enabled, checked: Option<bool> }, Separator}`, `PlatformCx::set_menu_bar(Option<&MenuDesc>)
    -> bool` (default `false`: no native bar) and `AppHandler::menu_activated(cx, id)` (default no-op).
    The macOS backend builds muda menus after the standard app menu (`Primary` → Cmd; group labels
    should arrive as disabled items). `MacAppHandler::menu` is gone (replaced by `menu_activated`).
    views' `MenuModel` → `MenuDesc` is for WIDGETS-VIEWS to write (labels without `&`, `Check::Radio`
    as `checked: Some(..)`, invisible entries left out).
  - OS-layer resize (asked by CANVAS): `begin_os_layer_resize`/`end_os_layer_resize` open and commit a
    Core Animation transaction (actions off; balanced on layer removal and window close); sublayers
    get top-left contents gravity and `masksToBounds`.
  - Option press/release: `flagsChanged:` already emits `Key::AltLeft`/`AltRight` key events.
  - presto-base docs: `KeyEvent.physical` is `page << 16 | usage`; `WindowEvent::Moved` is the content's
    top-left in global points.

- `THIRD_PARTY_NOTICES.md`: an "Apache-2.0 ported source" table with the GPUI row (other platform lanes
  will add rows; merge them into one table).
- `Cargo.lock`: muda 0.21.2, objc2-* 0.3.2 crates, dispatch2, block2 (macOS target only).

## Questions for the coordinator (each with a recommendation)

1. **`Monitor` and `WindowDesc.position` in device pixels.** macOS's desktop is in points with per-screen
   scales; I report each monitor's position and size as its global points times its own scale, and read
   `WindowDesc.position` at the main screen's scale. *Recommendation:* fine for single-DPI setups; if
   mixed-DPI placement matters, add points-based fields to `presto-platform`.
2. **Text-input client API.** The b1 `PlatformCx` only has `set_ime(ImeState)` (winit model). A full
   `NSTextInputClient` needs synchronous access to the document, so this backend takes
   `set_text_input_client(window, Option<Box<dyn TextInputClient>>)` (a free function). *Recommendation:*
   add `fn set_text_input(&mut self, id, client: Option<Box<dyn TextInputClient>>)` to `PlatformCx`
   (platform.md §3.1 already lists `set_text_input`), with the semantics in `lib.rs`'s docs: keys the IME
   passes through still arrive as key events with text; IME edits go to the client.
3. **`physical` key code format.** presto-base says "USB HID usage when known"; I use `page << 16 | id`
   (`0x00070004` for A, as Chromium). *Recommendation:* fix the format in presto-base's doc so win32 and
   wayland agree.
4. **`WindowEvent::Moved` units.** I report the content's top-left in global points (presto geometry is
   points). *Recommendation:* document points in presto-base.
5. **macOS-only callbacks** (`reopen`, `menu`, `live_resize`) live on `MacAppHandler: AppHandler`
   (`run_mac`). *Recommendation:* keep them there; `menu` could move to `AppHandler` if Windows/Linux
   menus ever go through muda.
6. **Custom title bar.** `WindowDesc` has no transparent-title-bar flag; this backend offers
   `set_custom_title_bar(window, on)`. *Recommendation:* add `WindowDesc.custom_title_bar` to presto-platform.
7. **Default frame source.** `CAMetalDisplayLink` acquires a drawable per tick; until wgpu P3 lets the
   compositor present it, `FrameSourceKind::best_timing()` picks `NSView.displayLink` (14+) or
   `CVDisplayLink`. *Recommendation:* switch the default to `MetalDisplayLink` when P3 lands.
