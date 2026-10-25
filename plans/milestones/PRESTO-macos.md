# PRESTO-macos: presto's own macOS platform backend (presto batch 3, lane MACOS)

Branch `ci/presto-macos` (from `ci/presto-base` at efe20dd5). Session `session_01TjQo3Ks6gxG7GdDLdgUZ86`.
Brief: `plans/cloud/presto-b3/LANE-macos.md`. Crate: `crates/presto-platform-macos` (empty on other OSes:
`#![cfg(target_os = "macos")]` at the crate root).

## What landed

| Module | What |
|---|---|
| `shim` | The b1 brief's platform trait (`AppHandler`, `PlatformCx`, `run`) and the types the brief names but doesn't define (proposals for `presto-platform`, see below). |
| `app` | `PrestoApplication` (NSApplication subclass: forwards Cmd+keyUp, which AppKit swallows), `PrestoAppDelegate` (Cmd-Q / Quit always vetoed → `MacAppHandler::quit_requested`, default sends `CloseRequested` to every window; reopen → `MacAppHandler::reopen`), main-thread state, the re-entrancy-safe event queue (every callback posts; the handler is never re-entered), `PlatformCx` (windows, OS layers, cursor, IME state, clipboard, `NSTimer` timers in common modes, GCD-based coalescing `Waker`, clock, monitors, hit regions, exit via `NSApp.stop`). `run_mac(handler, MacConfig)` adds the macOS callbacks; `run` wraps a plain `AppHandler`. |
| `window` | `PrestoWindow` (NSWindow subclass: borderless windows can be key; forwards `accessibilityFocusedUIElement` to the view), `PrestoWindowDelegate` (close → `CloseRequested`, move, focus + AccessKit focus, occlusion), `WindowDesc` → style mask, custom title bar (transparent, full-size content), transparency, min/max size, position in global top-left points. |
| `view` | `PrestoView`: layer-hosting `CAMetalLayer`; mouse (all buttons, enter/leave via a tracking area), tablet pen (`tabletPoint:`/`tabletProximity:`, subtype TabletPoint: pressure, tilt, pointer id per device), precise scrolling with `phase`/`momentumPhase`, magnify, rotate, smart magnify; keys with IME-first routing; the full `NSTextInputClient` (marked text, `attributedSubstringForProposedRange:actualRange:`, `firstRectForCharacterRange:actualRange:` per range, `characterIndexForPoint:`, `replacementRange`, `selectedRange` never NSNotFound in simple mode, `unmarkText`, `doCommandBySelector:`); AccessKit host methods; file drops; size/scale/theme reporting; live resize toggling `presentsWithTransaction`; caption hit regions (window drag, double-click per `AppleActionOnDoubleClick`, `_opaqueRectForWindowMoveWhenInTitlebar`); the standard Edit actions (`copy:` etc.) as the Cmd shortcuts they stand for. |
| `keymap` (pure) | `kVK_*` → egui `Key`, USB HID usage (`0x07xxxx`), location; `NSEventModifierFlags` → `Modifiers`; modifier-key press/release from `flagsChanged:` device bits; Option-as-Alt policy; GPUI's layout key naming (Cmd layers of Dvorak-QWERTY, non-ASCII layouts, shifted digits); typed text filter. Tests. |
| `keyboard` | `TISCopyCurrentKeyboardLayoutInputSource` + `UCKeyTranslate` (dead keys as their spacing character). |
| `text_input` (pure) | `NSRange` semantics over `TextInputClient` (`ClientTarget`) or the client-less `SimpleIme` (reports `ImeEvent::Preedit`/`Commit`), key routing, insert classification (the IME's pass-through of a key vs. real text), UTF-16 helpers. Tests with a mock editor (composition, clamping, press-and-hold replacement, reconversion ranges, geometry). |
| `display_link` | `MacFrameSource` (`FrameSource`): `CAMetalDisplayLink` (14+, latency 1, `targetPresentationTimestamp`, optional drawable sink for wgpu P3), `NSView.displayLink` (14+), `CVDisplayLink` (≤13). CA links run on their own thread and run loop; pause/teardown are blocks performed there; the CV context is leaked on purpose (late callbacks are harmless). |
| `layer`, `cursor`, `clipboard`, `monitor`, `menu`, `a11y`, `time` | GPUI-style `CAMetalLayer` config, OS-layer sublayers (flipped geometry, transactions without actions); `NSCursor` mapping (private shapes only when the class answers them); `NSPasteboard` text; `NSScreen` in global top-left points; muda menu bar (`set_menu_bar`, default App + Edit menu, events → `MacAppHandler::menu`); AccessKit adapter host (inactive / placeholder / active, full trees kept until the first request, `a11y_update`); mach clock (`NSEvent` and CA timestamps convert by scaling). |

Public main-thread extras: `set_text_input_client`, `text_input_changed`, `set_option_as_alt`,
`set_mouse_coalescing`, `set_os_layer_frame`, `remove_os_layer`, `set_presents_with_transaction`,
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

## Not run

Nothing was run on macOS from this container. `cargo check -p presto-platform-macos --target
aarch64-apple-darwin --all-targets` passes locally (cross-check, no link), and `cargo check` for Linux
(empty crate). Tests were written, not run (user's instruction). CI: see the report sha.

## Additions outside the crate

- `THIRD_PARTY_NOTICES.md`: an "Apache-2.0 ported source" table with the GPUI row (other platform lanes
  will add rows; merge them into one table).
- `Cargo.lock`: muda 0.21.2, objc2-* 0.3.2 crates, dispatch2, block2 (macOS target only).

## Questions for the coordinator (each with a recommendation)

1. **Shim types for `presto-platform`.** I defined `WindowDesc` (title, size, min/max, position, resizable,
   decorations, `custom_title_bar`, transparent, visible), `OsLayerDesc { rect, opaque }`,
   `SurfaceTarget::CoreAnimationLayer`, `ImeState { area, purpose }`, `Clipboard { get_text, set_text }`,
   `Waker(Arc<dyn Fn>)`, `Monitor`, `HitRegion { rect, kind: Caption|Client|Minimize|Maximize|Close }`,
   `FrameSource { set_active }` + `VsyncTick { now, target, interval }`, `PlatformError`.
   *Recommendation:* the consolidation maps these onto PLATFORM's types; where PLATFORM has none, adopt
   these.
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
5. **macOS-only callbacks** (`reopen`, `quit_requested`, `menu`, `live_resize`, `a11y_activated`,
   `a11y_action`) live on `MacAppHandler: AppHandler`. *Recommendation:* the cross-platform ones
   (`quit_requested`, `menu`, a11y) move to `AppHandler` with defaults at consolidation.
6. **Default frame source.** `CAMetalDisplayLink` acquires a drawable per tick; until wgpu P3 lets the
   compositor present it, `FrameSourceKind::best_timing()` picks `NSView.displayLink` (14+) or
   `CVDisplayLink`. *Recommendation:* switch the default to `MetalDisplayLink` when P3 lands.
