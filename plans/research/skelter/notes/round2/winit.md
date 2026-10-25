# winit for tessel: defects, gaps, downstream evidence, fork or own platform layer (research notes, 2026-10-25)

Context: `cadatomic/docs/research/2026-10-25-own-gui-toolkit.md` §4, §7, §8 (tessel: a retained toolkit on wgpu,
UI thread = platform thread, compositor thread owns present/pacing; phase 1 on winit, then own backends per OS).
Builds on `research2/wgpu-integration.md` (§1, §7) and `research2/ime-a11y-os.md` (§1, §5). Where those notes
already cover a point, it is cited, not repeated.

Evidence labels: **[doc]** documented (changelog, docs, issue text by its author), **[src]** source-read in the pinned
tree, **[inf]** inferred, **[anec]** anecdotal (one user report, unverified). Reaction counts: GitHub's issue pages
returned "Reactions are currently unavailable" to the fetcher, so this note gives **comment counts** instead; none of
the reaction totals could be read (recorded failure, not zero).

## 0. Pins and sources

| What | Pin | Date |
|---|---|---|
| winit `v0.30.13` | `e9809ef5` | 2026-03-02 |
| winit `v0.31.0-beta.4` | `d597ced7` (branch `v0.31.x`) | 2026-10-24 |
| winit `master` | `0c737280` ("X11: fix 32 bit build") | 2026-10-24 |
| iced-rs/winit (iced's pin) | `48116469` (branch `2026-10-02`) | 2026-10-04 |
| lapce/winit (`floem-winit`, floem's pin) | `133268de` | 2025-10-07 |
| warpdotdev/winit `master` | `c09160d1` | 2025-02-06 |
| tauri-apps/tao `dev` | `2d9b2ba` (0.37.2) | 2026-10-23 |
| wezterm `main` (`window/` only) | `e2a98e1` | 2026-10-10 |
| slint `master` (`internal/backends/winit`) | shallow, 2026-10-25 | 2026-10-25 |

Clones were blobless/sparse in the scratchpad and have been deleted. GitHub REST/search API was not available to this
session (403, session not bound to the repo), so issue data comes from fetching github.com issue pages (2026-10-25).

**Release model [src]:** releases are cut from backport branches (`v0.27.x` … `v0.31.x`); `v0.31.0-beta.4` has 144
commits that are not on `master` (cherry-picks), and `master` has 145 not in the beta. A fork should base on
`v0.31.x`, not `master`.

---

## 1. Ranked: the defects that matter most for tessel

Severity: **S1** blocks a latency goal or a core IDE feature; **S2** visible defect, workaround exists; **S3** minor.
"Side channel" = fixable from outside winit through raw handles, a window subclass, our own Wayland queue, or the
macOS delegate, with no fork.

| # | Defect / gap | Platform | Upstream | Sev | Fix route for us |
|---|---|---|---|---|---|
| 1 | **Windows backend has no maintainer** (CODEOWNERS `#/winit-win32 (UNOWNED)` since `ca6f5239`, 2025-08-04, when notgull left) [src]; 116 open win32 issues [doc] | Win | – | S1 (strategic) | own Windows backend first (§8) |
| 2 | **No frame clock / vsync source / presentation feedback** (#2412, opened 2022-08-08, 55 comments, open, milestone 0.31 since 2024-01-15); `pre_present_notify` throttles only on Wayland/Web (#2896, merged 2023-08-04) [doc] | all | open 4 years | S1 | ours on the compositor thread (already the design); no patch |
| 3 | **Windows modal move/size loop**: `ControlFlow`/`WaitUntil` ignored while resizing (#3272, 2023-12-19, 14 comments, open, milestone moved 0.30→0.31→**0.32** on 2026-03-20) [doc]; continuous redraw makes title-bar drag choppy because each `WM_PAINT` re-arms `RDW_INTERNALPAINT` (#4708, 2026-09-23, open, no PR) [doc]; right-click title bar / system menu freezes the loop (#1484, 2020-02-27, open) [doc]; `request_redraw` ignored while the system menu is open (#4608, 2026-06-29) [doc] | Win | open | S1 | subclass HWND: bypass `WM_PAINT` re-arm during `WM_ENTERSIZEMOVE…EXITSIZEMOVE`, `SetTimer` for JS timers; frame starts come from our compositor's proxy wake, which *is* dispatched inside the modal loop (§4.1) |
| 4 | **IME: Windows IMM32 only** (no TSF, no `WM_IME_REQUEST`) [src, `winit-win32/src/ime.rs` 163 lines]; **macOS partial `NSTextInputClient`** (`selectedRange` NSNotFound, `attributedSubstring` nil, `firstRect` ignores range, `replacementRange` ignored) [src] → dictation drops text (#4666), Character Viewer (#3342, 0.31 milestone), Korean hanja (#4524); 38 open IME issues; API complaint #4412 [doc; detail in `ime-a11y-os.md` §1.3] | Win, mac | PRs open: #4785 (macOS surrounding text), #4784/#4748 (dictation/emoji), 2026-10 | S1 for CJK users | Windows: TSF in a subclass with winit's IME disabled; macOS: patch (fork) or upstream the open PRs |
| 5 | **No pointer history for the mouse**: Windows mouse comes from `WM_MOUSEMOVE` (no `EnableMouseInPointer`, no `GetMouseMovePointsEx`); pen/touch *do* get `GetPointerFrameInfoHistory` entries [src `winit-win32/src/event_loop.rs` 2147–2215]; macOS coalescing not exposed [src: no "coalesc" in `winit-appkit`]; **no input timestamps** (#1194, 2019-09-30, 29 comments, open; madsmtm WIP #4529 and branch `madsmtm/event-time`) [doc] | all | WIP | S1 for canvases/router/ink | side channel: `GetMouseMovePointsEx` from the subclass on each `WM_MOUSEMOVE`; `NSEvent.setMouseCoalescingEnabled(false)`; timestamps: carry #4529 or own backend |
| 6 | **No subsurfaces / child surfaces** (#3506, 2024-02-20, 14 comments, milestone **0.32**; RFC #3928, 0.32; dup #4342) and **no `wp_presentation`** [doc, src: protocol list §4.4] | Wayland (all) | 0.32 | S1 for viewports | own `wl_subsurface`s on our queue over winit's `wl_display` (`wgpu-integration.md` §2) |
| 7 | **No Wayland tab tear-off** (`xdg-toplevel-drag-v1` not bound; winit's `start_drag` owns its `wl_data_source`) [src]; no upstream issue exists (search 2026-10-25) [doc] | Wayland | none | S2 (D7) | **side channel probably suffices** (§4.4, experiment W-TD) |
| 8 | **macOS redraws delivered only at `BeforeWaiting`** (`app_state.rs` `cleared()`, observer priority `CFIndex::MAX-1`) [src]; continuous-input lag reports (#1418, 2020-01-24, 42 comments, open, last comment 2026-09-09; #2240) [doc/anec] | mac | open | S2 | don't render in `RedrawRequested`; compositor thread with `CAMetalDisplayLink` |
| 9 | **No window-state events** (maximize/minimize/fullscreen) (#2334, 2022-06-13, open; Slint simulates one) [doc, src slint]; Wayland never emits `Occluded`, discards `xdg_toplevel` SUSPENDED (egui #5136, fixed eframe-side #8631 2026-10-06) [doc]; Wayland loop spins at 100% with Poll when frame callbacks stop (#4668, 2026-08-19, maintainer: expected) [doc] | all / Wayland | open | S2 | poll `is_maximized` on `SurfaceResized`; own Wayland occlusion from our 100 ms frame-callback timeout (proposal §8.2) |
| 10 | **Windows custom titlebar**: no `WM_NCHITTEST` hook → no snap-layout flyout without decorations (#1548, 2020-04-26, open; #3954; #4368 "abstraction over window input areas", 2025-09-30, open; PR #4450 cross-platform custom decorations, 2025-12-21, open) [doc, src] | Win | open | S2 | `SetWindowSubclass` (`ime-a11y-os.md` §4) |
| 11 | **accesskit_winit pinned to winit 0.30** (0.34.1 → `winit = "0.30.5"`); 0.31 PR AccessKit#656 draft since 2025-12-04 [doc] | all | draft | S2 | use AccessKit platform adapters directly |
| 12 | **DPI**: Win11 `ScaleFactorChanged` requested size ignored since #4341 (#4721, 2026-09-28); mixed-DPI drag grows windows in 0.30.13 (#4600, 2026-06-20) [doc] | Win | open | S2 | own backend / patch |
| 13 | **Keyboard**: Ctrl+Alt vs AltGr on Windows (#2945, Slint workaround; #3012; #2898, 22 comments); key-repeat lag report (#4043, 2024-12-16, no maintainer reply) [anec]; modifiers checked on every mouse message (#4239, S-perf, 2025-05-20) [doc]; no keyboard-layout API (#2678, 2023-02-08, 22 comments) [doc]; dead keys + arrows on macOS (#4537) [doc] | Win, mac | open | S2 | keymap layer resolves on `physical_key`/`key_without_modifiers` as Slint does |
| 14 | **Multi-window redraw starvation on Windows** (#4460, 2026-01-05, dup of #3648) [doc] | Win | open | S2 (moot if compositor drives frames) | compositor thread |
| 15 | **Proxy wake can overflow the Windows message queue (10 000 posted msgs)**; coalescing PR #4150 draft since 2025-02, milestone 0.31 [doc] | Win | draft | S3 for us (we coalesce on our side) | our own atomic "wake pending" flag before `wake_up()` |
| 16 | **Raw input**: registered by default (`DeviceEvents::WhenFocused` = `RIDEV_DEVNOTIFY`, mice *and* keyboards) [src `raw_input.rs` 133–160, `event_loop.rs` 244]; **can be switched at runtime** with `ActiveEventLoop::listen_device_events(DeviceEvents::Never)` (`RIDEV_REMOVE`, `event_loop.rs` 504–506) and is used only for `DeviceEvent`s, not `KeyboardInput` [src] | Win | API exists | S3 | **correction to the proposal §7.3**: no fork needed; call `Never` at start, `WhenFocused` while a 3D view captures |
| 17 | **No precision-touchpad gestures on Windows** (pinch/pan/rotate only macOS, Wayland, iOS) [src]; pinch arrives as Ctrl+wheel (#3705) [doc]; `ScrollSource` (wheel vs touchpad vs momentum) only macOS+Wayland (beta.4) [doc] | Win | none | S2 | DirectManipulation in own backend (`gpui-port.md`: copy+adapt) |
| 18 | **Cmd-Q** goes through the default menu's `terminate:`; winit only observes `willTerminate` (no veto, no `CloseRequested`) [src `menu.rs` 71, `app_state.rs` 177]; 0.31 lets the app install its own application delegate [doc changelog beta.1] | mac | – | S2 (unsaved docs) | own `NSApplicationDelegate` `applicationShouldTerminate:` (side channel) |
| 19 | **Startup**: `Focused(false)` at startup on macOS 0.30.12 (#4371, fixed on master); erroneous `Resized` on startup (#2094, 35 comments, open) [doc] | mac, Win, X11 | partly | S3 | – |

---

## 2. Project health

### 2.1 Releases [src: tag dates]

| Line | First | Last patch | Notes |
|---|---|---|---|
| 0.27 | 2022-07-26 | – | |
| 0.28 | 2023-02-02 | 0.28.7 2023-09-28 | |
| 0.29 | betas 2023-06-30/08-16; 0.29.2 2023-10-21 | 0.29.15 2024-03-13 | new keyboard API |
| 0.30 | 2024-04-27 | 0.30.13 2026-03-02 | `ApplicationHandler` trait |
| 0.31 | beta.1+2 2025-11-16, beta.3 2026-09-05, beta.4 2026-10-24 | – | backend split, dyn traits; **no final after 11 months of betas** |

Cadence: a minor roughly every 10–14 months; 0.31 has been "beta" for 11 months and its milestone is 70% done
(61/86 closed, 25 open, incl. #2412 frame pacing, #3977, #3442, #2921, #3592) [doc, milestones page 2026-10-25]. 0.32
milestone (13 items) already queues more breaking API: `Window` redesign for surface roles (#3928), subsurfaces
(#3506), "Make `Window` `!Send + !Sync`" (#3435), "Make the window be owned by the event loop" (#3434), coordinate
systems (#3891), `ModifiersChanged` rework (#2768) [doc].

### 2.2 Activity [src: `git log origin/master`]

- Commits per year: 2022 216, 2023 390, 2024 328, 2025 176, 2026 (to 10-24) 133. **Halved since 2023.**
- Last 13 months by month: 2025-10 13, -11 17, -12 3, 2026-01 5, -02 7, -03 32, -04 1, -05 **0**, -06 7, -07 24, -08 7,
  -09 24, -10 26 (≈13/month).
- Authors, last 12 months: Mads Marquart (madsmtm) 27, Olivier Goffart (Slint) 14, Timon 11, Kirill Chibisov
  (kchibisov) 11, Diggory Hardy (Kas) 9, Martin Marmsoler 7, then ~20 one-to-three-commit authors.
- Commits per backend, last 12 months: appkit 44, core 39, wayland 37, win32 36, x11 28.
- Open: **519 issues, 205 PRs** (2,497 closed PRs) [doc, 2026-10-25]. Open by platform label: win32 116, x11 100,
  appkit 87, wayland 39.
- **CODEOWNERS** [src `.github/CODEOWNERS`]: Android MarijnS95; AppKit/UIKit madsmtm; XKB, Wayland, X11 kchibisov;
  Web daxpedda; Orbital jackpot51; **Windows unowned** (since 2025-08-04). Effectively three active platform
  maintainers for desktop, and kchibisov owns three of the four desktop backends ("I do only Linux stuff in the end",
  #2412, 2023-09-24 [doc]).
- Slint's engineers (Goffart, Hausmann) are now regular contributors (non_exhaustive sweep, CI, Windows icons, url
  removal) [src] — the healthiest sign; Slint ships a winit backend in production.
- The PR inflow in Oct 2026 is large (25 opened in the first three weeks of October, several 0.30 backports) but
  review throughput is the bottleneck (205 open) [doc, inf].

### 2.3 The 0.31 redesign (what changed for downstream) [doc `winit/src/changelog/v0.31.md`]

- `ActiveEventLoop` and `Window` are **traits** (`Box<dyn Window>`, `&dyn ActiveEventLoop`, `cast_ref` to the backend
  type); `run_app(impl ApplicationHandler)`; `Event` removed.
- Crate split: `winit-core` (7.8k LOC), `winit-common` (3.2k), `winit-win32` (12.9k), `winit-appkit` (8.7k),
  `winit-wayland` (9.9k), `winit-x11` (11.8k), plus android/uikit/web/orbital [src, LOC at beta.4].
  Per-platform `WindowAttributes*` structs replace extension traits.
- `user_event` → `proxy_wake_up` with no payload; `can_create_surfaces` split from `resumed`.
- Pointer overhaul: `PointerMoved/Entered/Left/Button` with `PointerKind`/`PointerSource`/`ButtonSource`, `Touch`
  folded in, pen on Windows/Wayland/Web/iOS (not macOS, not X11; X11 pen PR #2647 open since 2023-01-23).
- IME: `request_ime_update` with capabilities, surrounding text, `DeleteSurrounding`, hints (Wayland only).
- New: native popups with `WindowPositioner` (beta.3), data-transfer clipboard/primary/DnD source+target (beta.4,
  PR #4571), `ScrollSource`, macOS `ApplicationHandlerExtMacOS::standard_key_binding`, macOS own app delegate allowed,
  X11 smooth resize via the sync extension, `is_macos_activation_click`.
- Size of the change: `v0.30.13..v0.31.0-beta.4`: 385 files, +47,352/−32,000 [src]. For comparison
  0.29.15→0.30.0: 294 files, +17,224/−18,470. Even between betas: beta.3→beta.4 (7 weeks) +6,568/−3,738, of which
  `winit-wayland` +3,302/−2,515, `winit-win32` +584/−210.

### 2.4 Downstream adoption of 0.31 (as of 2026-10-25) [src: Cargo.toml of each]

| Project | winit |
|---|---|
| egui/eframe 0.37.0 | `0.30.13` |
| bevy (main) | `0.30` (several windowing PRs labelled S-Blocked, inferred waiting on 0.31) |
| iced (master) | fork `iced-rs/winit@48116469` = 0.30.13 + patches |
| Slint 1.19 | `0.30.2+` |
| alacritty | `0.30.9+` |
| rerun | `0.30.13` |
| Neovide | `=0.30.13` |
| Servo | `0.30.13` |
| floem / Lapce | `floem-winit` = **master snapshot** 2025-10-09 (only one commit on top: "jump to latest winit and use calendar versioning") |
| AccessKit `accesskit_winit` 0.34.1 | `0.30.5` (0.31 PR draft) |

**Nobody large ships 0.31.** Choosing 0.31 for tessel means being an early adopter of a beta whose final date is
unknown; choosing 0.30.13 means a second migration later. cadatomic's egui island uses egui-winit 0.37 = winit 0.30
(`ime-a11y-os.md` lockfile row); one process cannot run two winit event loops, so tessel's winit version must equal
the egui island's, or the island must be fed input by tessel instead of by egui-winit [inf].

### 2.5 Receptiveness to platform-specific extensions [doc, inf]

- Against callbacks: "winit is not a callback driven and adding callbacks is just a mess" (kchibisov, #2120,
  2022-10-31). The accepted pattern is per-platform handler extension traits (`ApplicationHandlerExtMacOS`, #3851).
- Subviews were first "out of scope" (madsmtm, #3506, 2024-02-20), later re-scoped to 0.32 design work.
- Timestamps: maintainers "unsure of the use cases" (madsmtm, #1194, 2026-03-18) but started #4529.
- Frame requests without rendering: "I generally don't mind the feature once direct access to the Wayland backend
  exists" (kchibisov, #3977, 2024-10-31).
- Poll spinning on Wayland: "expected behaviour" (#4668).
- Accepted recently from outside: popups, DnD source, clipboard, Wayland gestures, resize increments, blur via
  `NSVisualEffectView`. Large features land, but take 6–18 months and ride a breaking release [inf].
- Upshot: a frame-clock API is plausible only as Wayland-first and only after 0.31; subsurfaces wait for 0.32's
  `Surface` split. Neither is on a timescale tessel can wait for [inf].

---

## 3. Catalogue by area (cross-platform)

### 3.1 Event loop model, redraw scheduling, timers

- `RedrawRequested` is winit's only frame hook. Windows: `RedrawWindow`→`WM_PAINT`; macOS: drained in a
  `BeforeWaiting` observer after all other observers (`winit-appkit/src/event_loop.rs` 465–500, `app_state.rs`
  363–385) [src]; Wayland: frame-callback throttled when `pre_present_notify` is called (#2896) [doc].
- macOS live resize: beta.4 redraws immediately on `viewFrameDidChange`/`viewDidChangeBackingProperties`
  (`redraw_during_live_resize`, view.rs 170–200) [src] — an improvement over 0.30; stale-hover PR #4681 open.
- `WaitUntil`: Windows high-resolution waitable timer; Linux timerfd via calloop; macOS `CFRunLoopTimer`
  (`wgpu-integration.md` §7) [src]. Stalls inside Windows modal loops (#3272) and with `WaitUntil` not waking on
  Linux Mint X11 (#4461, 2026-01-05) [doc].
- `pump_events` re-entrancy: macOS panics "tried to handle event while another event is currently being handled"
  (#3992, 2024-11-10, open; PR #4796 "fix the panic when a nested run loop runs inside an event handler", 2026-10-24)
  [doc] — matters when JS opens a native modal (file dialog) from a handler.
- No way to register file descriptors into winit's Unix loop (#3592, 0.31 milestone, open) [doc]; our
  compositor/worker wakes go through the proxy instead [inf].
- Downstream: Neovide implements its own vsync per platform — `VSyncWinDwm` (DwmFlush thread),
  `VSyncWinSwapChain` (waitable), `VSyncMacosDisplayLink`, `VSyncTimer`, and `WinitThrottling` on Wayland
  (`neovide/src/renderer/vsync/mod.rs`, main 2026-10-25) [src]; their render-loop PRs #1870 (221 comments) and #1977
  (120) [doc]. bevy #21773 (sluggish resize, 2025-11-07): the delay was FIFO `get_current_texture` blocking with no
  external throttle; mahkoh: graphics APIs don't throttle presentation on their own, apps need an external pacing
  source (also in #2412, 2026-02-07) [doc]. This is exactly tessel's compositor-thread design.

### 3.2 Threading

- 0.31 `Window: Send + Sync` but macOS methods marshal to the main queue (`maybe_wait_on_main`) — deadlock risk if the
  compositor calls winit while the UI thread waits on it (`wgpu-integration.md` §1.2) [src]. 0.32 may make `Window`
  `!Send + !Sync` again (#3435) [doc].
- Windows: `with_any_thread` exists (event loop off the main thread) [src]; irrelevant (UI thread = platform thread).

### 3.3 Multi-window

- Windows: only one window redraws when each requests redraw from `RedrawRequested` (#4460/#3648) [doc].
- Owned/child windows on Windows: decorations lost (PR #4753 open), focus/foreground mismatch (#4345) [doc].
- WezTerm: "winit never used to support opening more than 1 window at a time" was one historical blocker (wez,
  wezterm#4553, 2024-01-12) [doc] — fixed long ago, but shows why WezTerm stayed out.

### 3.4 DPI / scale changes

- Windows per-monitor v2, Win11 DPI-drag fix in 0.31 [doc]; open regressions #4721, #4600, #4304 (fullscreen across
  DPI change) [doc]. X11: `scale_factor` heuristics (#2231, 16 comments) [doc]. Wayland: scale/configure races
  (#2921, 0.31 milestone) [doc]. macOS: `set_outer_position` uses the current monitor's scale (#4440) [doc].

### 3.5 Resize behaviour and flicker

- egui #903 (2021-11-19, 44 comments, open): macOS fixed by `presentsWithTransaction` during resize (egui-wgpu PR
  #7641, feature `macos-window-resize-jitter-fix`); **Windows unresolved with every backend** ("Windows needs a
  separate fix", aspcartman 2026-04-26) [doc]. Windows content shifting to the top-left until moved (wiz21b,
  2025-02-03) [anec].
- Windows undecorated: maximize flashes without resizing (#4691); resizing with decorations disabled (PR #4188, open
  since 2025-04-06) [doc].
- X11 smooth resize via the sync extension landed in 0.31 [doc].
- tessel needs the DComp/`presentsWithTransaction`/sync-subsurface resize handshake regardless
  (`wgpu-integration.md` §1.5) [inf].

### 3.6 Fullscreen

- macOS: no keyboard events in exclusive fullscreen (#1992); can't ignore the notch (#4162); toggle segfaults in
  example (#4480) [doc]. Windows: exclusive mode-switch rejection panics (#4737); fullscreen+topmost mouse lag
  (#4273) [doc]. rerun: top-bar buttons unclickable fullscreen on macOS (rerun#10280) [doc]. S3 for an IDE.

### 3.7 IME

See `ime-a11y-os.md` §1.2–1.4 (complete list). Additions here: macOS IME locked on after first enable fixed in beta.3;
discard marked text on disable, click forwarding in beta.4 [doc]. Windows `KeyboardInput` still emitted while IME
allowed (#4508) [doc]. Glazier (Linebender's abandoned layer) README: "Some of the text input handling code is more
complete than the equivalent in winit, and so might be useful for reference" [doc,
https://github.com/linebender/glazier].

### 3.8 Keyboard

- Layout API missing (#2678). Windows: Ctrl+Alt/AltGr confusion (#2945, #3012, #4555), Alt+letter emits
  `CursorLeft` (#4465), dual-function keys (#4427), proposal to use raw input for physical keys (#4233), modifier
  checks on mouse floods (#4239), freeze with Punto Switcher fixed beta.3 [doc]. macOS: Caps Lock not reported
  (#4769), sticky-mods locations wrong (#4232), Ctrl-/ beeps (#4093), lldb hang on first Cmd+key (#3760) [doc].
- System shortcut inhibition (Alt-Tab, Super) not supported (#2776, 2023-04-24) [doc]; Wayland
  `keyboard-shortcuts-inhibit` not bound [src: not in protocol list].
- Key repeat: Windows lag report (#4043) [anec]; Wayland `KeyState::Repeated` handling patched in iced's fork
  (d6588d65, 2026-10-03) [src].
- Slint resolves shortcuts from `key_without_modifiers` on Windows Ctrl+Alt (winitwindowadapter.rs ~1470) [src].
- WezTerm: "winit's key event processing is different and may not be as flexible or comprehensive as wezterm's.
  For a terminal emulator where the users are very demanding about keys, this could be a huge blocker" (wez,
  2024-01-12) [doc].

### 3.9 Pointer

- History/coalescing: §1 row 5. Timestamps: #1194. Click count missing (#3899, 0.31 milestone) [doc].
- Cursor grab: Windows grab bugged in bevy (#4473), macOS grab semantics (#1093, 2019), `set_cursor_position`
  affects deltas on macOS (#999), Wayland locked + set position error (#4557) [doc].
- Capture across windows: Windows `SetCapture` on press (event_loop.rs 1079) [src]; no global cursor query in winit
  (tao has `cursor_position`, `monitor_from_point`) [src].
- Cursor icons: `cursor-icon` crate, animated/URL sources, macOS 15 resize cursors (beta.3) [doc]. Custom HiDPI cursor
  on macOS open (#4094) [doc]. WezTerm adopted `cursor-icon` from winit (wezterm#8100, 2026-08-27) [doc].

### 3.10 Touch, pen, touchpad gestures

- Pen: Windows `WM_POINTER` + `GetPointerPenInfo`, Wayland tablet-v2, iOS; not macOS, not X11 [src].
- Gestures: macOS magnify/rotate/smart-magnify/pressure; Wayland pinch/pan/rotate/hold (PR #4338, 2025-09-07;
  #4585); none on Windows/X11 [src, doc].
- Inertial scrolling on Linux missing (#4238) [doc].

### 3.11 Drag and drop, clipboard, menus

- 0.31: typed DnD target and source; Windows source is modal `DoDragDrop` and shows **stale content** for its
  duration ("Let DefWindowProcW validate the region and show stale content for the duration of the drag",
  event_loop.rs ~1390) [src]; X11 source missing (`ime-a11y-os.md` §3) [src].
- Clipboard + primary selection: beta.4 [doc]. Menus: none beyond the macOS default menu; downstream uses `muda`
  (floem, Tauri) [src].

### 3.12 Decorations, titlebars, window state

- Windows snap layouts need `HTMAXBUTTON` from `WM_NCHITTEST`; winit has no hook; `with_msg_hook` sees only
  *posted* messages before `DispatchMessage` (`winit-win32/src/lib.rs` 207–235), not sent ones like `WM_NCHITTEST`
  [src]. rerun built custom window buttons for Windows/Linux (rerun#1094) [doc].
- macOS traffic-light inset: PR #4466 open since 2026-01-18; tao has `set_traffic_light_inset` [doc, src].
- Wayland CSD via `sctk-adwaita` when no `xdg-decoration` (Mutter) [src]; min-size errors with CSD (#4559) [doc];
  opt-out PR #4724 [doc].
- Window state: #2334 (no events); remember position/size (#941); `xdg-session-management-v1` (#4636, 2026-07-19)
  [doc].

### 3.13 Focus and activation

- Wayland `xdg_activation` bound [src]; activation tokens missing input serial (PR #4782, 2026-10-20) [doc];
  activation of existing windows (#3633) [doc]. X11 spurious focus on map (PR #4763), Cinnamon focus (#4479) [doc].
  Windows: hidden+maximized creation breaks focus (#4586) [doc].

### 3.14 Startup time

- Wez: "wezterm's use of xcb led to a very smooth and snappy startup compared to other frameworks" (2024-01-12) [doc].
  No measurements of winit startup found [recorded gap]. Windows hidden-window ghost flash (#4643) [doc].

### 3.15 Accessibility hooks

- winit has none; `accesskit_winit` subclasses on macOS and hooks the wndproc on Windows; pinned to 0.30
  (`ime-a11y-os.md` §2) [src]. macOS Voice Control (#4218) [doc].

### 3.16 Child windows, subsurfaces, embedding

- #159 child windows (2017, 40 comments, open), #1161 wrap an existing window (30 comments), #3506/#3928 (0.32),
  Wayland child embedding #4342 (duplicate of #3928) [doc]. X11 embedded window focus (#4789, 2026-10-22) [doc].

---

## 4. Per platform

### 4.1 Windows (`winit-win32`, 12.9k LOC, unowned)

What winit does well: per-monitor-v2 DPI, `WS_EX_NOREDIRECTIONBITMAP` (`no_redirection_bitmap`), backdrop/corner/
border colour, pen/touch with frame history, `with_msg_hook`, `drag_window`/`drag_resize_window`, native popups
(computed positioner) [src].

Gaps and defects for tessel: unowned backend (#1); modal loops (#3); IMM32 only (#4); mouse history (#5); no
`WM_NCHITTEST` (#10); DPI regressions (#12); keyboard (#13); multi-window redraw (#14); wake flood (#15); no touchpad
gestures (#17); OLE drag stale paint (§3.11).

Modal-loop mechanics [src `winit-win32/src/event_loop/runner.rs` 386–400]: an event is delivered immediately when
the handler is not on the stack, and buffered otherwise. Inside a title-bar move/size loop the handler is not on the
stack, so **`proxy_wake_up` and `RedrawRequested` are delivered** during the loop; what stops is winit's own wait
(`WaitUntil`, `about_to_wait`). Inside a modal started *from* a handler (`MessageBoxW`, `TrackPopupMenu`,
`DoDragDrop`) everything is buffered and the window shows stale content (#4708 comment by datbird 2026-10-17: 860 ms
CPU/s re-arm loop under `MessageBoxW`) [doc]. Consequences for tessel [inf]: (a) frame starts posted by the compositor
work during resize; (b) JS timers must not depend on `WaitUntil` — schedule them from the compositor/timer thread via
the proxy; (c) never open a native modal from inside a dispatch; post it to run after the handler returns.

Downstream Windows patches: iced (older branches) carried "Emulate `AboutToWait` event on Windows while resizing"
(commit referenced from #3272, 2024-02-07) [doc]; Warp's fork has branches `ben/fix-resize-redraw`,
`andy/fix-windows-dpi-changed`, `david/fix-double-borrow-crashes-on-resize` [src: branch names only, anec].

### 4.2 macOS (`winit-appkit`, 8.7k LOC, madsmtm)

Strengths: active owner; tracing spans around `sendEvent:`/run-loop activities (2026-03) [src]; 0.31 container
contentView (winit's view is a subview; RWH handle unchanged) [doc]; own app delegate allowed; `standard_key_binding`.

Gaps: redraw at `BeforeWaiting` (#8); NSTextInputClient (#4); no coalescing control, no tablet events (#5, §3.10);
re-entrancy panic (#3992); Cmd-Q (#18); mouse lag reports (#1418, #2240) [anec]; traffic-light inset (PR #4466).
Side channels available: own `NSApplicationDelegate`; `NSEvent.setMouseCoalescingEnabled`; a CAMetalLayer sublayer
on winit's view (proposal §8.1); adding our own `NSView` beside winit's view inside the container for text input
(then keyDown goes to our first responder, not winit) [inf].

### 4.3 Linux X11 (`winit-x11`, 11.8k LOC, kchibisov)

Mixed Xlib/x11rb ("Xlib must die", #3198, 0.31 milestone) [doc]; XIM only, Fcitx crashes (#3761, #3875) and leaks
(#4525) [doc]; no pen (PR #2647 open since 2023); no Present-based frame timing; smooth resize via sync extension
(0.31) [doc]. 100 open issues. S2 overall: X11 is the lowest-priority platform in the proposal (§8.2).

### 4.4 Linux Wayland (`winit-wayland`, 9.9k LOC, kchibisov)

Protocols bound in beta.4 [src, `grep protocols::` in `winit-wayland/src`]: xdg-shell (toplevel, popup/positioner),
fractional-scale-v1, viewporter, xdg-activation-v1, text-input-v3, pointer-constraints, relative-pointer,
pointer-gestures, tablet-v2, primary-selection, xdg-toplevel-icon-v1, ext-background-effect-v1, KDE blur
(plasma); via sctk 0.21: data-device, decorations, seat/keyboard (xkb).
**Not bound:** `wp_presentation`, `wl_subcompositor`/subsurfaces, `xdg-toplevel-drag-v1`, `fifo-v1`,
`commit-timing-v1`, `keyboard-shortcuts-inhibit`, `xdg-session-management`, `xdg-dialog`, `idle-inhibit`
(#3351) [src].

Defects: Poll spin (#4668), no Occluded/suspended (egui #5136), configure races (#2921), CSD min-size (#4559, #4715),
resize increments axis (#4713), compositor restart (#3427) [doc]. Only 39 open issues — the best-maintained desktop
backend.

**Tear-off without a fork (experiment W-TD) [inf, protocol from https://wayland.app/protocols/xdg-toplevel-drag-v1]:**
the protocol needs (1) a `wl_data_source` used *only* for DnD, passed to
`xdg_toplevel_drag_manager_v1.get_xdg_toplevel_drag` before `wl_data_device.start_drag`; (2) the press serial; (3)
`attach(xdg_toplevel, x, y)` on the new window "before mapping it". On our own `wl_event_queue` over winit's
`wl_display` we can bind our own `wl_seat`→`wl_pointer` (the compositor sends button events, with serials, to every
`wl_pointer` of the seat), our own `wl_data_device_manager`/`wl_data_device`, and the toplevel-drag manager; take the
origin `wl_surface` from the raw window handle and the new window's `xdg_toplevel` from
`WindowExtWayland::xdg_toplevel()`; create the new winit window, `attach`, then let the compositor thread commit its
first buffer (that commit maps it). Risks: winit's own data device also sees our drag's enter/motion/drop on our
windows (filter by our private MIME type); serial mismatch if winit's and our pointer disagree on the press (they
cannot, same compositor event). Support today: KWin 6.7, Mutter 51, Jay only. Size ~500–700 lines. If this works,
**register D7 (fork winit for Wayland tear-off) is not needed.**

---

## 5. Downstream complaints and patches

| Project | What it complains about / patches | Evidence |
|---|---|---|
| **iced** | Fork `iced-rs/winit`, branch `2026-10-02` on 0.30.13: macOS URL handling (`handleUrl` outside `NSApplicationDelegate`, by Richard), Wayland wheel as lines via `value120`, macOS 15 frame-resize cursors, Wayland `KeyState::Repeated`, sctk 0.21, Windows `Sync` for `EventLoopProxy`/`MonitorHandle` restored, dependency dedupe (12 commits, mostly backports). Earlier branches: "Emulate AboutToWait on Windows while resizing" (2024), `v0.29.x` fullscreen options. Small, backport-style fork. | [src] `git log v0.30.13..iced/2026-10-02`; iced `Cargo.toml` line 249 |
| **tao (Tauri)** | "a fork of winit which replaces Linux's port to Gtk" — needed for webkit2gtk and menus/tray/shortcuts; Linux backend is GTK3 (3.9k LOC), no native Wayland; extras: `set_progress_bar`, `set_badge_label`, `set_traffic_light_inset`, `set_undecorated_shadow`, `set_overlay_icon`, `set_always_on_bottom`, `set_content_protection`, global `cursor_position`, `monitor_from_point`, `with_menu`, RTL; Windows 9.8k LOC, macOS 7.0k. Still on the pre-0.29 API shape. | [doc] tao README; [src] `src/window.rs`, `src/platform/*.rs`, LOC at `2d9b2ba` |
| **floem/Lapce** | Tracks a winit **master snapshot** (`floem-winit`, 2025-10-09) to get unreleased 0.31 features; older `lapce/winit` branches (`composition-event`, `new-keyboard`) show IME/keyboard patches historically. | [src] floem `Cargo.toml` line 65; `git log` |
| **Slint** | winit backend ~9.7k LOC of adapter. Workarounds: #2990 (maximize button on non-resizable), #2334 (simulated window-state event), #4371 (macOS focus), #2945 (Ctrl+Alt vs AltGr), Windows zero-size→occluded, web CSS size. Also ships Qt and LinuxKMS backends (Qt for "windowing system integration, rendering, and native widget styling"; LinuxKMS needs no compositor). | [src] `internal/backends/winit/winitwindowadapter.rs` 819, 1395–1414, 1474; [doc] Slint backends page |
| **egui/eframe** | Resize jitter #903 (macOS fixed by transaction present; Windows open); Wayland repaint when hidden #5136 (fixed eframe-side #8631); IME interrupt hack (toggle `set_ime_allowed`); Korean IME zero-width range; `NamedKey::Process` filter. | [doc] issues; `ime-a11y-os.md` §1.2 |
| **bevy** | macOS high CPU (#10261, 94 comments, open); sluggish resize (#21773 — pacing, not winit); Windows cursor grab (winit #4473); title-bar drag stutter (bevy#25903 → winit #4708); CustomCursor size (#17276). | [doc] |
| **Neovide** | Built its own vsync sources per platform because winit has none; long keyboard saga (#445 non-QWERTY, 153 comments; #1899/#1789 new keyboard API); macOS lag on startup mouse move (#2073). | [src] `src/renderer/vsync/mod.rs`; [doc] issues |
| **rerun** | Custom window buttons on Windows/Linux (#1094); macOS fullscreen top bar (#10280). | [doc] |
| **Warp** (terminal) | Maintains `warpdotdev/winit` with fix branches (Windows DPI change, resize redraw, double-borrow crash on resize, X11 WM hints/attention panics, drop-handler `QueryInterface`). | [src] branch names only [anec] |
| **WezTerm** | Never adopted winit; own `window` crate. See §6. | [doc] wezterm#4553 |
| **Linebender (counter-example)** | Abandoned its own layer (glazier, ex-druid-shell): "Linebender no longer uses Glazier ... You should prefer to use winit instead." | [doc] glazier README |

---

## 6. Why some projects own their platform layer

- **WezTerm** (wez, wezterm#4553, 2024-01-12) [doc]: "moving to winit is a significant rewrite of the windowing layer,
  so it is not to be attempted lightly"; reasons: one window at a time (historic), "winit's key event processing is
  different and may not be as flexible or comprehensive as wezterm's ... For a terminal emulator where the users are
  very demanding about keys, this could be a huge blocker"; "wezterm's use of xcb led to a very smooth and snappy
  startup"; "I don't think there is a huge amount gained simply by switching to winit ... most of the heavy GUI stuff
  is done in opengl or webgpu anyway". Size of `window/src/os` at `e2a98e1` [src]: macOS 4,790, Wayland 5,129
  (on sctk), Windows 4,216, X11 5,579 (xcb + xkbcommon), common 1,519 — ~21k LOC total, with GL context creation
  included.
- **Zed/GPUI** ("Linux when?", Thorsten Ball & Mikayla Maki, 2024-05-07,
  https://zed.dev/blog/zed-decoded-linux-when) [doc]: "we want to use each platform as best as we can ... That often
  means talking directly to the platform"; cross-platform libraries work "only as long as you're fine with the
  platform being abstracted away"; GPUI is "a framework that allows you to talk directly to the platform whenever you
  need"; effort "to get to 120FPS" with "triple-buffering, syncing up with the OS and the display". winit is not
  mentioned. Sizes (caller): gpui_windows 14.8k, gpui_macos 9.8k, gpui_linux 16.9k.
- **Makepad** [src, inf]: `makepad-platform` vendors even `smallvec`/`bitflags` under `libs/` and owns Metal, D3D11,
  Vulkan, GL, Android, iOS, wasm and Quest backends; the motive appears to be dependency control and uniform
  live-reload/GPU targets rather than a specific winit defect. No primary statement found [recorded gap].
- **Slint (partly)**: winit for desktop, but Qt for native styling/integration and LinuxKMS for compositor-less
  embedded [doc]. Its engineers now improve winit upstream instead of forking [src].
- **tao**: GTK for webkit2gtk plus desktop features (menus, tray, shortcuts) [doc].
- Pattern [inf]: projects with an *editor-grade* text/keyboard requirement and their own frame timing (WezTerm, Zed)
  own the layer; generic toolkits and engines (egui, bevy, iced, Slint, Linebender) stay on winit and accept the
  gaps or carry small forks.

---

## 7. Options with costs

### (a) winit 0.31 + side channels (no fork)

What side channels cover [src/inf, per §1]:
- **Windows** (`SetWindowSubclass` on winit's HWND, ~1.2–1.8k LOC total): `WM_NCHITTEST` snap layouts (~400);
  `WM_PAINT` bypass during move loops + `SetTimer` timers (~150); `GetMouseMovePointsEx` history per
  `WM_MOUSEMOVE` (~200); `WM_IME_REQUEST` (`IMR_QUERYCHARPOSITION`, `IMR_RECONVERTSTRING`, `IMR_DOCUMENTFEED`)
  answered from our text client (~400); raw input via `listen_device_events` (0); DComp visuals (proposal §8.2).
  TSF (~2–3k) is possible only with winit's IME disabled for the window, i.e. replacing winit's IME wholesale.
- **macOS**: own app delegate (Cmd-Q veto), coalescing off, CAMetalLayer sublayers, CAMetalDisplayLink (~400). Not
  coverable cleanly: winit's `NSTextInputClient` (lives on winit's view class) — options are an own text-input
  `NSView` in the container (we then also handle keyDown for that view, ~800–1,200) or runtime method replacement
  (fragile).
- **Wayland** (own queue over winit's `wl_display`, ~1.5–2k): subsurfaces, `wp_presentation`, fifo/commit-timing,
  toplevel-drag (§4.4), occlusion by frame-callback timeout.
- Cost: no rebase; but each side channel fights winit's internal state (resize geometry races on Wayland,
  `wgpu-integration.md` §1.5; IME message ownership on Windows) and breaks silently when winit changes internals
  (e.g. 0.31 changed macOS `contentView`) [inf]. Upstream fixes reach us only with the next release; 0.30→0.31 took
  ~19 months (0.30.0 2024-04-27 → 0.31 still beta).

### (b) Carry a fork (patch series on `v0.31.x`)

Concrete patch list (sizes inferred from the touched code):

| P | Patch | Crates | Size | Upstream status |
|---|---|---|---|---|
| P1 | Windows `WM_NCHITTEST` hook / hit-test regions (snap layouts, caption buttons) | win32 + core API | 300–400 | #4368, PR #4450 open |
| P2 | Windows mouse history (`GetMouseMovePointsEx`) + coalesced samples on `PointerMoved` | win32 + core | 250–350 | none |
| P3 | Input timestamps on all pointer/key events | all backends + core | 400–600 | #4529 WIP |
| P4 | Windows modal-loop handling: suppress `WM_PAINT` re-arm during move/size and modal dialogs, emulate `about_to_wait` with `SetTimer` | win32 | 150–250 | #3272 (0.32), #4708 |
| P5 | Windows `WM_IME_REQUEST` (char position, reconversion, document feed) + per-range rect query | win32 + core IME API | 400–600 | none |
| P6 | Windows TSF text store (replacing IMM32 path behind a flag) | win32 + core | 2,000–3,000 | none |
| P7 | macOS full `NSTextInputClient` (selected range, attributed substring, first rect per range, character index for point, replacement range) via a synchronous text-client query | appkit + core | 600–900 | PRs #4785, #4784, #4748 open |
| P8 | macOS mouse coalescing switch + tablet (`NSEventSubtypeTabletPoint`) pen | appkit | 300–400 | none |
| P9 | Window-state events (maximized/minimized/fullscreen/suspended) | all + core | 200–300 | #2334, PRs #2926/#2929 stale |
| P10 | Wayland: expose seat, input serials, data-device hooks; or bind `xdg-toplevel-drag-v1` with a tear-off API | wayland + core | 400–700 | none (D7) |
| P11 | Wayland: frame request without rendering, `Occluded` from suspended, presentation feedback passthrough | wayland | 200–400 | #3977, #3442 (0.31 milestone) |
| P12 | Wake-up coalescing on Windows | win32 | 50 | PR #4150 draft |
| P13 | Windows precision-touchpad gestures (DirectManipulation) | win32 | 700–900 | none |
| P14 | Windows DPI regressions (#4721, #4600) | win32 | 100–300 | open |

Total ≈ 6–9k LOC of patches, ~70% of it in `winit-win32`. Rebase cost [inf from §2.3]: the 0.31 line alone moved
`winit-win32` by +584/−210 in seven weeks and `winit-core` by +347/−149; P1–P3, P5–P7, P9 add **core API**, which is
what 0.32 plans to reshape (`Surface` roles, window ownership, `!Send`). Expect 1–3 engineer-days per beta/minor for
the Windows patches and a near-rewrite of the core-API patches at 0.32. The fork also forks `accesskit_winit` (or
drops it) and pins egui's island to our fork (`[patch.crates-io]` works for egui-winit only if versions match).

### (c) Own platform layer per OS, incrementally behind a trait

- Shape [inf]: `tessel-platform` trait (window, input, IME text-client, frame clock, surfaces/layers, clipboard/DnD,
  a11y adapter hooks). Backend `winit` implements it for all OSes first; `win32` replaces it on Windows; later
  `wayland`, `appkit`. One process uses one backend per OS, so the egui island must be fed from tessel's input (not
  egui-winit) once a platform leaves winit.
- Sizes (reference points):

| OS | winit backend | GPUI | WezTerm | tao | tessel own (inferred) |
|---|---|---|---|---|---|
| Windows | 12.9k | 14.8k | 4.2k | 9.8k | 9–12k incl. TSF (~2.5k), DComp, DirectManipulation |
| macOS | 8.7k | 9.8k | 4.8k | 7.0k | 7–9k incl. full NSTextInputClient |
| Wayland | 9.9k (sctk) | part of 16.9k | 5.1k (sctk) | GTK | 6–8k on sctk, incl. subsurfaces, presentation, toplevel-drag |
| X11 | 11.8k | part of 16.9k | 5.6k | GTK | stay on winit |

- Cost: ~22–29k LOC for three OSes plus a permanent maintenance load; but every gap in §1 becomes fixable, and the
  frame clock, layer tree and text client stop being side channels [inf]. GPUI/Zed and WezTerm show that a small
  team can keep such layers alive; Linebender's glazier shows the opposite failure (no users → archived) [doc].

### (d) Contribute upstream

Worth upstreaming regardless (they help us on any option and are small): wake coalescing (#4150), modal-loop
`WM_PAINT` fix (#4708), Wayland `Occluded`/suspended (#3442), window-state events (#2334), macOS text-input PRs
(#4785/#4784/#4748 review/test), Windows DPI regressions. Not realistic upstream on our timescale: frame clock
(#2412, 4 years), subsurfaces (0.32), TSF, pointer history API. Windows being unowned cuts both ways: reviews are slow,
but a contributor who sustains Windows work could become its owner [inf].

---

## 8. Recommendation and order

**Strategy: (a) now, (c) for Windows next, (d) continuously; a fork (b) only as a short-lived bridge for macOS text
input.** Reasons: our primary platform's upstream backend is unowned and accumulates the defects that hit an IDE
hardest (modal loops, IME, DPI, keyboard, pointer history); the fork patch series is ~70% Windows and adds core API
that 0.32 will reshape, so a fork would mostly be a slow, expensive own Windows backend; everything else in the
proposal already lives beside winit (compositor thread, frame clock, DComp/CAMetalLayer/subsurfaces).

Order and sizes (inferred):

1. **Pick the winit line together with the egui island.** Stay on 0.30.13 until egui moves (or feed egui input from
   tessel), then 0.31 final. Do not build on a 0.31 beta. (decision, no code)
2. **Side-channel shims, phase 1** (~3k LOC): Windows subclass (P1, P2, P4, P5 equivalents, raw input off),
   macOS delegate/coalescing, Wayland own queue for subsurfaces + presentation + occlusion. Behind the
   `tessel-platform` trait from day one so the shims become the own backends' code.
3. **Experiment W-TD: Wayland tear-off by side channel** (~600 LOC). If it works, retire register D7's fork. If it
   fails (only reason expected: winit's data device consuming our drag), carry P10 as a ~400-line patch.
4. **Upstream batch** (~500 LOC + review time): #4150, #4708 fix, #3442, #2334 — reduces what any later fork or
   shim must carry.
5. **Own Windows backend** (`tessel-platform-win32`, 9–12k LOC, 2–3 engineer-months): message loop with our modal
   policy, `WM_POINTER` for mouse with history and timestamps, TSF (+ IMM32 fallback), `WM_NCHITTEST`, DComp tree,
   DirectManipulation, per-monitor DPI, OLE DnD without stale paint, AccessKit Windows adapter. Port from GPUI's
   `gpui_windows` where licences allow (`gpui-port.md`), and winit-win32 for the keyboard layout tables.
6. **macOS text input**: if users need it before an own macOS backend, carry **one** fork patch (P7, ~800) or
   help land #4785/#4784; otherwise wait for step 8.
7. **Own Wayland backend** (6–8k on sctk) when the shims fight winit's surface state (resize races) more than they
   save; subsumes P10/P11.
8. **Own macOS backend** (7–9k), last, matching the user-base order in the proposal (§8.1).
9. X11 stays on winit indefinitely.

## 9. Corrections to earlier notes

- Proposal §7.3/§8.1 "raw input registered always": registered by default when focused, but
  `listen_device_events(DeviceEvents::Never)` removes it at runtime [src]; no fork needed.
- Register D7 (fork winit for Wayland tear-off) may be unnecessary (§4.4 W-TD) [inf].
- "Windows modal loops stall it": the stall is of winit's wait/timers and of anything opened from inside a handler;
  proxy wake-ups and `RedrawRequested` are still delivered during title-bar move/size loops [src runner.rs].

## 10. Recorded gaps

- Reaction counts on issues not readable (GitHub page fetch); comment counts used.
- No winit Matrix/Discord meeting notes found publicly (search 2026-10-25).
- No primary statement from Makepad on why it owns its platform layer.
- No startup-time measurements of winit vs own layers.
