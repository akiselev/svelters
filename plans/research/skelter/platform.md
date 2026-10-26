# skelter's platform layer: winit, shims, own backends, present paths

Part of the GUI toolkit proposal (`README.md` in this folder). Date: 2026-10-25. Status: proposal
for the user; nothing is built.

The user said "looks like we'll have to fork winit too" and asked for its defects, gaps and user
complaints to be researched. The evidence:
- `round2/winit.md`: winit 0.30.13 `e9809ef5`, 0.31.0-beta.4 `d597ced7`, master `0c737280`; the
  downstream projects; the patch sizes;
- `round2/wgpu-integration.md`: threading, surfaces, the resize handshake;
- `round2/ime-a11y-os.md`: IME, accessibility, OS integration;
- `round2/scheduler-latency.md` §4: the clock sources.

## 1. The recommendation in one paragraph

**Don't fork winit as the main strategy.**
- **Base:** skelter's platform layer starts on stable **winit 0.30.13** behind a `skelter-platform`
  trait.
- **Side-channel shims (~3k LOC):** small per-OS shims fill the gaps that block latency and text
  input, through the handles winit already exposes. This needs no fork, because a window subclass
  on Windows, our own app delegate on macOS, and our own Wayland event queue all reach underneath
  winit.
- **Own backends next:** then our own platform backends, one OS at a time, behind the same trait,
  **Windows first** (9–12k LOC).
- **X11 stays on winit.**
- **Upstream** the small fixes throughout.
- **The only fork** is a short-lived bridge: a registered winit fork carrying one patch, the full
  macOS `NSTextInputClient` (600–900 LOC), and only if macOS users need dictation and the Character
  Viewer before our own macOS backend exists.

## 2. Why, in evidence

**What is wrong with winit for us**, ranked (`round2/winit.md` §2):
1. **The Windows backend has no maintainer.** CODEOWNERS has marked `winit-win32` "(UNOWNED)" since
   2025-08-04, and 116 win32 issues are open. Windows is our primary platform.
2. **No frame clock or presentation feedback.** #2412 has been open since 2022, with 55 comments.
   `pre_present_notify` throttles only Wayland and Web. Neovide wrote its own vsync per platform.
3. **The Windows modal loops.**
   - `ControlFlow` and `WaitUntil` are ignored while resizing (#3272; its milestone slipped
     0.30 → 0.31 → 0.32).
   - Title-bar drags stutter, because each `WM_PAINT` re-arms a repaint (#4708, no PR).
   - The system menu freezes the loop (#1484).
   - Proxy wake-ups and `RedrawRequested` *are* delivered inside the move/size loop (source-read). So
     a compositor-thread clock that wakes the UI thread through the proxy survives it.
4. **IME.**
   - **Windows:** IMM32 only, with no TSF and no `WM_IME_REQUEST`.
   - **macOS:** `NSTextInputClient` is incomplete, so dictation drops text (#4666) and the Character
     Viewer is broken (#3342). Fix PRs #4785, #4784 and #4748 are open.
   - **winit cannot cancel a composition.**
5. **No mouse pointer history and no input timestamps.** Only pen and touch get frame history.
   Timestamps (#1194) are still a work in progress.
6. **Wayland gaps:**
   - no subsurfaces (planned for 0.32);
   - no `wp_presentation`;
   - no `xdg-toplevel-drag`;
   - no `Occluded` event, and a 100% CPU spin when frame callbacks stop (#4668).
7. **Smaller gaps:**
   - no maximize/minimize/fullscreen state events (#2334);
   - no `WM_NCHITTEST` hook for snap layouts with a custom title bar (#1548, #4368);
   - Windows DPI regressions (#4721, #4600);
   - AltGr vs Ctrl+Alt confusion (#2945);
   - Cmd-Q cannot be vetoed by default.

**Two corrections to the earlier notes:**
- **Raw input is not "registered always".** It defaults to `WhenFocused`, and
  `listen_device_events(DeviceEvents::Never)` turns it off at runtime. It only feeds `DeviceEvent`s,
  so no fork is needed for it.
- **D7 may not need a fork.** Wayland tear-off looks doable from our own event queue on winit's
  `wl_display` (§3.3).

**Why not a general fork.**
- **Churn.** 0.30.13 → 0.31.0-beta.4 touched 385 files (+47k/−32k), and 0.32 is already planned to
  break the API again: a `Surface` split, subsurfaces, window ownership moving to the event loop, and
  possibly `Window` becoming `!Send` again.
- **Size and cost.** A fork carrying everything we need would be 6–9k LOC, about 70% of it in the
  unowned win32 backend. Rebasing would cost 1–3 days per minor release, with a near-rewrite of the
  core-API patches at 0.32.
- **Declining activity.** Commits per year: 390 (2023), 328 (2024), 176 (2025), 133 so far in 2026.
- **Why not 0.31 yet.** Nobody large ships 0.31 yet. egui 0.37, Bevy, Slint, Alacritty, rerun,
  Neovide and Servo are on 0.30; iced forks 0.30.13.

**Why own backends, eventually.**
- **Zed:** "talking directly to the platform"; generic libraries work "only as long as you're fine
  with the platform being abstracted away".
- **WezTerm:** its own layer is about 21k LOC in total, 4.2–5.6k per OS. Its author on winit's key
  handling: "may not be as flexible or comprehensive as wezterm's ... could be a huge blocker".
- **Slint** wraps winit in ~9.7k lines of adapter with workarounds, and also ships Qt and LinuxKMS
  backends.
- **The counter-example:** Linebender archived its own layer, glazier ("You should prefer to use
  winit instead"). Own backends are a real cost, which is why they come one OS at a time, behind
  measurements, and only after the shims.

## 3. The platform trait and phase 1 (winit 0.30.13 plus shims)

### 3.1 The trait

`skelter-platform` exposes one interface, implemented first by the winit-plus-shims backend and
later by own backends per OS:

```rust
pub trait Platform {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn create_layer(&mut self, window: WindowId, desc: LayerDesc) -> OsLayer;   // DComp child visual / CAMetalLayer sublayer / wl_subsurface
    fn surface_target(&self, target: SurfaceOwner) -> RawSurfaceTarget;         // for wgpu, created on this (main) thread
    fn frame_source(&self, window: WindowId) -> FrameSource;                     // waitable / display link / frame callback (+ feedback)
    fn set_text_input(&mut self, window: WindowId, client: Option<TextInputHandle>); // IME client binding (architecture §11.3)
    fn clipboard(&mut self) -> &mut dyn Clipboard;                               // + primary selection on Linux
    fn start_drag(&mut self, data: DragData, image: Option<DragImage>);
    fn pointer_capture(&mut self, global: bool);                                 // tab tear-off
    fn monitors(&self) -> Vec<Monitor>;                                          // work areas, scale, refresh
    fn set_hit_test_regions(&mut self, window: WindowId, regions: &[HitRegion]); // custom title bar, snap layouts
    fn a11y_adapter(&mut self, window: WindowId) -> &mut dyn A11yAdapter;
    // events arrive as `PlatformEvent { seq: InputSeq, os_time: Instant, .. }` with pointer history
}
```

The trait is internal to skelter. cadatomic and svelters never see winit types. That removes today's
coupling to egui `Key` names: skelter's key enum round-trips them (architecture §11.2).

### 3.2 Phase 1 backend: winit 0.30.13 plus shims

**Why 0.30.13.** It is the stable line everyone ships. An egui island does not need egui-winit:
skelter translates its own events into egui `RawInput`. So there is no second winit event loop and no
version lock with egui. 0.31 waits until it is released and something we need is only there.

| Shim | What it does | Size | Replaces |
|---|---|---|---|
| **Windows window subclass** (`SetWindowSubclass` on winit's HWND) | `WM_NCHITTEST` hit-test regions (snap layouts with our title bar); skip `WM_PAINT` re-arm during `WM_ENTERSIZEMOVE…EXITSIZEMOVE`; `SetTimer` for JS timers inside modal loops; `GetMouseMovePointsEx` pointer history; `WM_IME_REQUEST` (reconversion, per-character candidate placement); DComp target on the HWND (`with_no_redirection_bitmap`) | ~1.2k | winit #1548/#4368, #3272/#4708, history, part of TSF |
| **Windows misc** | `DeviceEvents::Never` except while a 3D view captures the mouse; `DCompositionWaitForCompositorClock` / waitable on the compositor thread | ~0.2k | — |
| **macOS app delegate and view hooks** | our own app delegate (Cmd-Q veto, reopen); mouse coalescing off where history matters; `CAMetalLayer` sublayers on winit's NSView; `CAMetalDisplayLink` on the compositor thread | ~0.6k | — |
| **Wayland side queue** | our own event queue on winit's `wl_display`: `wp_presentation` feedback, subsurfaces for viewports (with `wp_viewporter`), occlusion detection from frame-callback starvation (100 ms rule), `fifo-v1`/`commit-timing-v1` | ~0.8k | winit's missing subsurfaces, presentation, `Occluded` |
| **Crates** | arboard + smithay-clipboard (clipboard, primary selection), rfd (dialogs), muda (macOS menu bar only), ashpd (Linux dark mode and accent through the portal), notify-rust, AccessKit platform adapters directly | — | winit 0.30 has no clipboard; `accesskit_winit` is stuck on 0.30 anyway, and we validate updates ourselves |

**Total:** about 3k LOC, all behind the trait.

### 3.3 Wayland tab tear-off (decision D7)

An experiment (~600 LOC) checks whether tear-off works from our own queue:
1. bind our own seat and pointer, for the press serial;
2. bind our own data device and the `xdg_toplevel_drag_v1` manager;
3. attach winit's exposed `xdg_toplevel` before its first commit.

Only KWin 6.7, Mutter 51 and Jay support the protocol. **If the experiment works, D7's winit fork is
not needed.** Retiring D7 is a coordinator decision recorded in DECISIONS.md.

### 3.4 Upstream batch

These small fixes go upstream rather than into a fork (~500 LOC in total):
- #4150, Windows wake-up coalescing (a draft exists);
- a fix for #4708, the modal-loop `WM_PAINT` re-arm;
- #3442;
- #2334, window-state events.

## 4. The present path and resize, per OS

The compositor thread owns every surface and never calls winit (architecture §7). Surfaces are
created on the main thread and moved over. Resize is the one synchronous handshake: the UI thread
sends `Resize { size, scale, seq }` and waits up to about one refresh for `ResizeDone`.

### Windows

- **Swap chain:** wgpu DX12 forced, with `Dx12UseFrameLatencyWaitableObject::DontWait`. We take the
  waitable and the swap chain through `as_hal::<Dx12>()` (`waitable_handle()`, frame statistics) and
  wait on it ourselves in the compositor's one multiplexed wait. Frame latency is 1.
- **DirectComposition:** our own device and tree.
  - Each window gets a root visual; each viewport or canvas gets a child visual through
    `SurfaceTargetUnsafe::CompositionVisual`. That variant is gated only by `#[cfg(dx12)]`.
  - wgpu calls `SetContent` but not `Commit`, so we commit after configuring.
- **Pacing:** the compositor clock on Windows 11 (`DCompositionWaitForCompositorClock`, boosted during
  gestures), falling back to the waitable. Windows 10 uses the waitable plus DXGI frame statistics,
  with `DwmFlush` as the last fallback. If `DwmFlush` returns in under 1 ms, sleep one interval
  instead (GPUI's rule).
- **Resize:** the UI thread, inside the subclassed `WM_SIZE`/`WM_NCCALCSIZE`, waits for the
  compositor to reconfigure, render, present and commit. Whether that is clean from every edge is
  checked on hardware.
- **Checks:** PresentMon must show "Independent Flip" or "Composed: Flip", never "Copy with GPU
  GDI". Present1 dirty rects with `FLIP_DISCARD` are checked on hardware (wgpu fork P1).

### macOS

- **Layers:** one CAMetalLayer per window plus sublayers, created and laid out by the UI thread
  (frame, `contentsScale`). Any layer change the compositor makes goes inside an explicit
  `CATransaction`.
- **Pacing:** `CAMetalDisplayLink` (macOS 14+) on the compositor thread, `preferredFrameLatency` 1,
  with animations sampled at `targetTimestamp`. The fallback is `NSView.displayLink`; `CVDisplayLink`
  creation is deprecated in macOS 15.
- **wgpu patches:** P3 lets us acquire from the display link's drawable, and P4 removes wgpu's
  off-main NSWindow/NSView access during acquire.
- **Live resize** follows the Firefox and GPUI model:
  1. Before the next acquire, the UI thread sets `presentsWithTransaction = YES`.
  2. The compositor configures, renders and submits, then hands the `SurfaceTexture` back.
  3. The main thread calls `queue.present`.
  4. Compositor commits stay suspended until the main thread returns to its run loop.
- **After input:** keep presenting for 1 s after high-rate input (ProMotion).

### Wayland

- **Swap chain:** Vulkan. Mesa clamps latency 1 to 2–3 images, so latency is bounded by present-wait
  through wgpu's Vulkan chain hooks, or by gating acquires on frame callbacks.
- **Pacing:** frame callbacks plus our own `wp_presentation` feedback; `commit-timing-v1` where
  available (Mutter, Weston).
- **Viewports:** subsurfaces of winit's toplevel on our own queue, each with a wgpu surface from its
  `wl_surface`. They run desync normally and switch to sync during a resize. On resize, the
  compositor drops any frame at the old size before presenting.
- **The remaining race:** a one-frame race with winit's own surface writes remains until our own
  Wayland backend locks all surface state, as Firefox's `WaylandSurfaceLock` does.

### X11

- winit plus Vulkan, with viewports rendered to textures (no OS layers) and full presents.
- Resize artefacts are accepted.
- Avoid XWayland on Wayland sessions, which adds about 3 ms.

## 5. Own backends (M5)

**The order is Windows, Wayland, macOS.** Each lands behind the trait while winit keeps the other
OSes. Sizes are from `round2/winit.md` §5, compared with GPUI's `gpui_windows` (14.8k), `gpui_linux`
(16.9k), `gpui_macos` (9.8k) and WezTerm (4.2–5.6k per OS).

| Package | Contents | Size | Takes from |
|---|---|---|---|
| **W-P3 Windows** | our own window procedure and modal-loop policy (render and timers continue through move, size and menus); `WM_POINTER` mouse with history and QPC timestamps; a TSF text store (Chromium's is ~1.8k lines; Firefox's 4.5k with per-IME quirks) plus `WM_IME_REQUEST` for ATOK/Mozc reverse conversion; DirectManipulation for precision touchpad pan, pinch and inertia (GPUI's `direct_manipulation.rs`, 361 LOC); DirectComposition built in; an OLE drag source that keeps dispatch running (winit's `DoDragDrop` shows stale content); per-monitor DPI v2 done right | 9–12k | GPUI Windows input and keyboard (~670 LOC), Chromium and Firefox TSF designs |
| **W-P4 Wayland** | on smithay-client-toolkit: `text-input-v3` (surrounding text, purpose, hints), `xdg-toplevel-drag-v1` tear-off, presentation feedback and subsurfaces locked with the toplevel state, fractional scale, the portal for theme and dialogs, our own CSD title bar where `xdg-decoration` is missing | 6–8k | GPUI's Wayland frame-loop state machine (ideas) |
| **W-P5 macOS** | our own NSView: the full `NSTextInputClient` (marked text, attributed substrings, `firstRectForCharacterRange`, `characterIndexForPoint`, `replacementRange`); tablet and pen events; an AccessKit host view without the subclassing adapter's close crash; display link per display | 7–9k | GPUI `NSTextInputClient` and `UCKeyTranslate` (~800 LOC), `display_link.rs` (586) |
| **X11** | stays on winit | — | — |

**If macOS users need dictation and the Character Viewer before W-P5,** the bridge is a registered
winit fork with one patch: the macOS `NSTextInputClient` completion (600–900 LOC). Fix PRs #4785,
#4784 and #4748 are open upstream, so the patch may simply backport them.

## 6. IME and text input by OS and phase

| Capability | Phase 1 (winit 0.30 plus shims) | Own backend |
|---|---|---|
| preedit and commit, one cursor rect | winit, all desktops | yes |
| per-character candidate placement, reconversion | Windows: the shim's `WM_IME_REQUEST`; macOS: no; Wayland: no | TSF; full `NSTextInputClient`; text-input-v3 |
| surrounding text, delete-surrounding | no (winit 0.30) | TSF; macOS; text-input-v3 |
| purpose and hints (number, password, url, code) | no | TSF input scopes; macOS; text-input-v3 |
| cancel or commit a composition from the app | Windows: shim (`ImmNotifyIME`); macOS: no (toggle IME off and on, as egui does) | yes |
| macOS dictation, Character Viewer, hanja | broken (winit #4666, #3342, #4524) | yes, or the one-patch bridge fork |
| IME-first key routing | skelter's own rule, all phases (architecture §11.2) | — |

## 7. Other OS integration

| Need | How |
|---|---|
| menus | custom-drawn, in native popups (winit 0.30 lacks them; layers until then); macOS menu bar through muda |
| custom title bar, snap layouts | Windows: the `WM_NCHITTEST` shim; macOS: transparent title bar plus traffic-light inset; Wayland: our CSD where `xdg-decoration` is missing |
| file dialogs | rfd (portal on Linux) |
| dark mode, accent colour | winit on Windows and macOS; ashpd on Linux |
| DPI and fractional scale | winit; own backends fix the Windows regressions |
| touchpad gestures | macOS and Wayland through winit; Windows through DirectManipulation (W-P3) |
| pen | Windows and Wayland through winit; macOS in W-P5 |
| notifications | notify-rust (UNUserNotificationCenter on macOS) |
| tab tear-off | Windows, macOS, X11 through pointer capture; Wayland through §3.3 |

## 8. Checks that need the user's machines [local]

These queue in REVISIT.md as the packages land:
- DComp resize cleanliness from every edge;
- Present1 dirty rects under `FLIP_DISCARD`;
- PresentMon present modes;
- setting `drawableSize` off-main on macOS;
- CAMetalDisplayLink on a non-main run loop;
- present-wait through wgpu's Vulkan chain API on Mesa and NVIDIA;
- IME acceptance with Japanese, Chinese and Korean IMEs, dead keys, and macOS accents and dictation;
- screen readers (NVDA, Narrator, VoiceOver, Orca);
- Wayland tear-off on KWin and Mutter.
