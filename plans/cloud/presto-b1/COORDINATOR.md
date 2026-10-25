# presto build-out: coordinator record

Coordinator session: `session_01CqZ417NuvkRujZaYSm7Pwo` (started 2026-10-25). The user (2026-10-25): build the
full framework now, no M1 gate, GitHub CI for Windows/macOS, recommendations taken, minimal building and
testing until consolidation, "ready to go tonight".

## Wave 1 (batch presto-b1), launched 2026-10-25 18:23 UTC from `ci/presto-base` 6b13f4ae

| Lane | Branch | Session |
|---|---|---|
| CORE | `ci/presto-core` | `session_013K2YZSUHRdTwCmv2DuBCqE` |
| TEXT | `ci/presto-text` | `session_01MSpaSgqNxdmRY5R5WEUdho` |
| RENDER | `ci/presto-render` | `session_01YXr8ajAcdJzov6eqCHbEL2` |
| FRAME-ANIM | `ci/presto-frame-anim` | `session_01QXUoKfVdDsZJ9NiLeS5bWS` |
| COMPOSITOR | `ci/presto-compositor` | `session_01FvoComfRkHRa6Z2sPhbyz7` |
| PLATFORM | `ci/presto-platform` | `session_01UJ89x3UhYpz8PAmkjLFg9G` |
| COMPILER | `ci/presto-compiler` | `session_014u9BiDM8fXY3HswkrJESuw` |
| RUNTIME | `ci/presto-runtime` | `session_0165tCmCPXxjyHJbM8gXBxXG` |

Skeleton merges: the coordinator merges each lane's first compiling API push into `ci/presto-base` and tells
the dependent lanes (COMPOSITOR needs RENDER/FRAME-ANIM; wave 2 needs CORE/TEXT/PLATFORM/COMPOSITOR).

## Wave 2 (presto-b2) and early wave 3 (presto-b3), launched 2026-10-25 18:27 UTC from `ci/presto-base` efe20dd5

The user (2026-10-25): "You can run more than 8 in parallel". Wave 2 starts at once, coding against the b1
brief's signatures until the wave-1 skeletons are merged into `ci/presto-base`.

| Lane | Branch | Session |
|---|---|---|
| WIDGETS-BASIC | `ci/presto-widgets-basic` | `session_012eauTkCP7to4arcKCPJvRq` |
| WIDGETS-VIEWS | `ci/presto-widgets-views` | `session_01UQTt4dFy9hz4VEv6q2vmpg` |
| TEXT-INPUT | `ci/presto-text-input` | `session_017NL3B2S8QKfkboTAfbVntW` |
| WIDGETS-TIER1 | `ci/presto-widgets-tier1` | `session_01DjBCnf7oMK3XEzkghWoiQs` |
| SVELTERS-PRESTO | `ci/presto-svelters-presto` | `session_01497YqbhMLBiY7YWGtiLKAX` |
| APP-TEST | `ci/presto-app-test` | `session_012jLSJr3WKZ7XDr4KKJtnf1` |
| EGUI-ISLAND | `ci/presto-egui-island` | `session_01YP64x1tuvgBxa9JAtpuUpj` |
| MOTION-CSS | `ci/presto-motion-css` | `session_011YTnnfgQ149nNJtSdVuDWS` |
| WIN32 (b3) | `ci/presto-win32` | `session_01AJfMrChD4ZXtP37EMUYQ8Y` |
| WAYLAND (b3) | `ci/presto-wayland` | `session_01JDnE12Ws2QHwSfPYzesDaR` |
| MACOS (b3) | `ci/presto-macos` | `session_01TjQo3Ks6gxG7GdDLdgUZ86` |

## Later lanes

| Lane | Branch | Session | Note |
|---|---|---|---|
| SVELTERS-PRESTO (2) | `ci/presto-svelters-presto` | `session_012kp44HgxNR751yz6GvNAH5` | continues the first session's hand-off (port to the real Document) |
| CORE-2 | `ci/presto-core-2` | `session_01QDYxkmxLL2rFtDDfSbu4ow` | style table, DESC_NONPASSIVE, CORE's not-done list (CORE's a07c3189 wins on overlap) |
| BERTH | berth `claude/presto-berth` | `session_01FfrTeoE8knLYGyFeFf9Ss3` | presto_berth |
| QUIRE | quire `claude/presto-quire` | `session_01VFabqgKPiq5AmCkua2ntGG` | presto_quire |
| CADATOMIC (b3) | cadatomic `claude/presto-adoption` | `session_01EHpySAKp1WzwLa6e6y6vV8` | W-D1, brief `plans/cloud/presto-b3/LANE-cadatomic.md` |
| CANVAS (b3) | `ci/presto-canvas` | `session_01RS7HGPNSY8j4mR36Fty4ZJ` | W-V1/W-V2 presto side |
| TEST-V2 (b3) | `ci/presto-test-v2` | `session_01FZ1DXSABohru158URTe1mR` | W-T2 |
| CONSOLIDATION-1 | `ci/presto-integration` | `session_016SQDMJubiR73cQAPuXCT1k` | brief `CONSOLIDATION-1.md` |
| CORE-3 | `ci/presto-core-3` | `session_012X4RdtAUrgfNuRfD1Vh4ge` | WIDGETS-BASIC core requests (a)–(j) plus the shared drag/anchor members |

Merged into `ci/presto-base` so far: platform skeleton, frame-anim, compositor skeleton, core (full batch-1 scope and
the request work a07c3189), text, render (full API, then GPU references 90959a28), the widgets substrate (WIDGETS-BASIC d46dcd96),
CORE-2 (8d45666c), FRAME-ANIM test fixes (1ea8983c), platform try_create_window (5aab7622), CI fixes 14981893 and 2a12e530.
Done lanes: CORE, CORE-2, PLATFORM, RENDER, RUNTIME (4c65de8d, all five svelters workflows green; merged at consolidation).
Decisions: SVELTERS-PRESTO Q1–Q4 and CORE Q1–Q3 as in their records (coordinator, 2026-10-25).

## Waves 2 and 3 (original plan)

`plans/cloud/presto-b2/` (wave 2: widgets, svelters-presto, the app runner and test harness, the egui island,
native motion and CSS) launches when CORE's and TEXT's skeletons are in `ci/presto-base`. Wave 3 (cadatomic
adoption, presto_berth, presto_quire, OS layers and canvases, the wgpu fork, own platform backends) follows
the first consolidation.

## Consolidation

One consolidation per wave: merge lane branches into `ci/presto-integration` in the order base-crates → core
→ text → render → frame-anim → compositor → platform → compiler → runtime; fix build breaks; run the presto
and svelters CI; record failures in `plans/cloud/presto-b1/FIX-QUEUE.md`; the coordinator merges to master
when the five svelters workflows plus `presto` are green.

## For the consolidation (running list)

- BERTH done at berth `claude/presto-berth` a49111b: run `presto_berth` tests (tests/dock.rs, 11) and berth's
  check.sh, neither run yet. Gaps and 4 questions in berth `docs/PRESTO-BERTH.md` (pointer capture, window rects and
  z-order in PlatformCx, service access from A11yCx).
- TEXT-INPUT at 0030df69. Answered (coordinator, 2026-10-25): Q1 Monaco undo grouping on Windows/macOS, Q2 caret blink as
  per-window compositor config (sent to COMPOSITOR), Q3 pin floors after the first green run, Q4 text-edit stays on
  `presto_core::Kind`. Routed: R1/R2 to QUIRE, R4 to CORE-3, R6 (PRIMARY) to WAYLAND, TextServices install to APP-TEST.
- COMPOSITOR 2b98a948 merged into base (d58a0fa0) before its CI finished; check it.
- APP-TEST 30dd49e2: TextServices not wired (TODO in `crates/presto/src/app.rs` App::init, request 11). Decided
  (coordinator, 2026-10-25): TextServices owns its own `SystemClipboard` (`MemoryClipboard` headless) and a compositor
  handle clone for caret fast commits; the consolidation wires it.
- APP-TEST requests 1–10 (`plans/milestones/PRESTO-app-test.md`), decided (coordinator, 2026-10-25): 1–5 to CORE-3
  (additive Document entries; IME purpose rides with the area), 6–8 and 10 to COMPOSITOR (`frame_args`, a new
  `ToUi::FrameTimings` variant for compositor-side stamps, confirm BeginFrame back-pressure, `ui_frame_skipped` →
  `Signal::UiFrameSkipped`; then drop APP-TEST's empty-commit stopgap), 9 fixed in the workflow (crates/presto added).
- CI load (coordinator, 2026-10-25): the five svelters workflows skip pushes that touch only presto crates and presto lane records (paths-ignore); presto.yml covers those. A consolidation's or master's push still runs all six.
- 19:35 UTC: 65 runs queued; cancelling the stale runs of done lanes failed (403, the session lacks actions write). Superseded runs on a lane branch are still cancelled by each workflow's concurrency group.
- WAYLAND 755fcaf6 (R6 PRIMARY in Clipboard, cross-lane in presto-platform). Q1 decided (coordinator, 2026-10-25): the
  facade `crates/presto` selects the backend (target-cfg deps on presto-platform-{win32,wayland,macos}, default-on
  features, winit fallback); no presto-platform-api split now. Consolidation wires the selection in `App`.
- QUIRE done at quire `claude/presto-quire` 676b49bc (crate presto_quire, 43 tests not run; quire has no CI, so the
  consolidation runs them). Decided: Q1 kind-owned layers/fast commits (to CORE-3), Q2 TextLayout cluster accessor (to
  TEXT), Q3 4 new host events approved, Q4 egui-free quire view crate as a follow-up, Q5 svelters git dep OK.
- FRAME-ANIM done once presto CI on a9503acb passes its crates.
- QUIRE R1/R2 at quire 35b5063a; R2 (composition over a selection replaces it) approved (coordinator, 2026-10-25). TEXT-INPUT re-pins quire-text to it. Consolidation merges quire claude/presto-quire and berth claude/presto-berth into their masters after their tests and checks pass.
- CORE-3 checkpoint eb669f1e merged (eb833fe0). Decided (coordinator, 2026-10-25): Q1 svelters-presto queues JS writes from call_sync and applies them after dispatch; Q2 Layout writes restyle, no skip flag yet. CORE-3 order now: APP-TEST 1–5, TEXT-INPUT R4, QUIRE kind layers, then its item 4.
- SVELTERS-PRESTO ea2074dc (ignore CI on b03c384b). Q8 decided (coordinator, 2026-10-25): two-phase dispatch (Document::begin_dispatch → Dispatch::next(&mut doc) → finish_dispatch runs the default unless prevented), superseding the CORE-3 Q1 answer; CORE-3 does it first.
- WIDGETS-BASIC stopped at 03eb08df (merged). Open: cancelable `changed` after CORE-3's two-phase dispatch (message session_012eauTkCP7to4arcKCPJvRq), OutsidePress core half (CORE-3), questions 1–5 in its record; its tests have no CI result yet.
- CORE-3 d39eaf4b merged. Sync delivery decided (coordinator, 2026-10-25), overruling two-phase: the lent document (`set_sync_delivery(FnMut(&mut Document, &JsEvent) -> Outcome)`, deliverer taken out for the call), because a kind's own cancelable signals are raised inside the native default and two-phase couldn't prevent them.
- 19:53 UTC: consolidation 1 launched, session_016SQDMJubiR73cQAPuXCT1k, branch ci/presto-integration, brief CONSOLIDATION-1.md.
- 19:55 UTC: wave-3 lanes CADATOMIC, CANVAS, TEST-V2 launched. Not launched: wgpu fork patches P1–P4 (a fork is made only when a release blocks work; none does yet). CORE-3 e2b96936 merged (96426b4a).
- RENDER final 054196d7 (merged f0c33126). CI 40225795440: presto-render's 26 unit and 13 GPU tests pass on lavapipe,
  WARP/DX12 and Metal. Decided (coordinator, 2026-10-25): upload references as CI artifacts; colourless coverage
  icons take currentColor; per-layer instance caching only after a measurement. Red in that run, routed:
  presto-core `paint_transitions_sample_on_the_ui_thread` (CORE-3); three presto-widgets slider float-equality
  tests (WIDGETS-BASIC); presto-platform `os_time_is_in_the_past` (fixed in 47bbd1e4, which postdates that run).
- CORE-3 342cf6be merged: nested sync events are queued and delivered (not cancelable) after the causing listener
  returns; light dismiss core half (`Kind::light_dismiss`, `outside_press`). Not done: compaction, incremental
  layers/hits, OS file drops, GTK single selection owner.
- CADATOMIC decisions (coordinator, 2026-10-25): cadatomic and presto_berth name presto-* by branch ci/presto-integration (CADATOMIC may push that manifest change to berth claude/presto-berth); adapter/limits hooks in CompositorConfig (sent to COMPOSITOR); berth pin moves to claude/presto-berth on claude/presto-adoption only, in its own commit.
- Consolidation 1 interim: ci/presto-integration 65ead449 (code f5d7ab0c, from base 82e166e2), all 11 lanes merged,
  workspace check clean. FIX-QUEUE routed (coordinator, 2026-10-25): F1/F2 COMPOSITOR; F3–F5 CORE-3; F6 already fixed
  (966d4d75, base c65b2e79); F7 done (svelters-presto 80189479); F8 TextServices + F9 backend selection to APP-TEST;
  F10 presto.yml checks presto-widgets `dialogs` (this commit); F11 berth/quire at consolidation 2; F13 at consolidation 2.
- CORE-3 final ceaf16b8 and COMPOSITOR 9a58f1c4 merged (177731fd): F1–F5 fixed (presto-core --lib 119/119 locally). Slider tests disputed (CORE-3: fail; WIDGETS-BASIC: 19/19 pass after 966d4d75); consolidation 1 settles it. Done lanes now: CORE, CORE-2, CORE-3, PLATFORM, RENDER, RUNTIME, WIDGETS-BASIC, BERTH, QUIRE.
- MACOS a706b24f (code bef22840): presto CI check green on all OSes, its unit tests pass on macOS; nothing run on Mac hardware. Q1–Q7 decided as recommended (coordinator, 2026-10-25); MACOS adds PlatformCx::set_text_input, WindowDesc.custom_title_bar and the key-code/Moved docs; WIN32/WAYLAND adopt after.
- APP-TEST c091234d closes F8 (widget services via CoreDocument::with_widgets, clipboard service, fast-commit drain) and F9 (presto::run backend selection, winit fallback, PRESTO_BACKEND). It contains integration 65ead449, so it merges into ci/presto-integration, not base.
- WIN32 a051ddbd (own backend, no winit; CI 40226717129 on 07bb50d1: crate tests pass on all three OSes). Decided (coordinator, 2026-10-25): (a) WIN32 adds OsLayerCommitter::new; (b) DXGI waitable through before_present/wait, no FrameSource widening; (c) DeleteSurrounding-before-Commit documented in presto-base; (d) DwmExtendFrameIntoClientArea [local]. Open for consolidation 2: drag-source, popup and fullscreen APIs in presto-platform. WAYLAND 880ec338 adopted the shared API.
- FRAME-ANIM final (a9503acb, fully in base): presto-frame and presto-anim pass on all three OSes (run 40226563028). Same run: svelters macos-appkit failed (counter-mac backend). Consolidation 1 must check whether integration reproduces it. Left out: filter values (AnimValue::Filter), 3-D transforms, invariant 6, EDF GPU ordering.
- EGUI-ISLAND b1a9d379 (code 0dd5a347): every target passes on all three OSes. Decided: Rotate clockwise-positive (doc in presto-base), UI-thread island rendering, Q4 EguiIslandKind + mount API now (for CADATOMIC/presto_berth), PrestoDrag shim onto core DnD. linux-gtk eframe_idle idle-CPU failure called load flake by the lane; consolidation to confirm.
- WIDGETS-VIEWS ae617f24: 19 view kinds; presto-widgets 370/3 in run 40227999085 (the 3 are the since-fixed slider tests). Decided (coordinator, 2026-10-25): Q1 300 ms latch (consolidation 2 checks it against Chromium's source), Q2 inner windows in the overlay stack, Q3 NSMenu owned by MACOS from MenuModel (menu-bar hidden on macOS), Q4 disabled rows reachable (disabled_focusable=false skips), Q5 platforms emit AltLeft/AltRight, Q6 common scroll event, Q7 menu roots not in Tab order, Q8 anchor prop kept. host.rs PLACING/parts_of to spec-driven: WIDGETS-BASIC.
- MACOS ff854b58 merged: native menu bar API in presto-platform (MenuDesc/MenuNode, PlatformCx::set_menu_bar -> bool, AppHandler::menu_activated). Consolidation 2: crates/presto routes menu_activated to the menu-bar kind; WIN32/WAYLAND keep the default (false).
- CADATOMIC milestone 1 (claude/presto-adoption): the whole app in one EguiIsland under presto, seen live under Xvfb; native status bar; CancelClose bridge; key pre-dispatch; adapter/limits ported; builds on ci/presto-integration. WIDGETS-BASIC df3c476d merged (spec-driven parts/places_children, host tests 12/12). EGUI-ISLAND gets physical_key mapping and the device-reset path.
- 20:48–22:10 UTC: every lane stopped on the account's 5-hour usage limit. 22:12 UTC: all lanes with work left resumed (CORE-3 also gets Document::start_drag for EGUI-ISLAND Q5; APP-TEST gets the menu_activated routing). The limit is shared by about 18 sessions, so lanes were told to avoid reruns on unchanged code and keep turns short.
- MACOS done: presto run 40230302721 on ff854b58 (contains base 177731fd+) green on all three OSes, check and test: the first fully green presto run.
- WIN32 done 305eb732 (run 40230205775 green on all three OSes), merged into base. TEXT-INPUT done 300b2660 (run 40228643140: presto-widgets green on all three OSes; pins 228/243 GTK, 226/243 Qt). MOTION-CSS done 4a4374fe (svelters workflows green; presto-motion passes). CORE-3 done (presto-core 119/119 and presto-widgets 248/248 on all three OSes in 40228786756). Decided (coordinator, 2026-10-25): a presto-base Easing variant for svelte/easing curves (MOTION-CSS); SVELTERS-PRESTO wires OP_ANIMATE native descriptors and SVL_STYLE_SHEET_BIN into presto; APP-TEST sets set_single_selection under GTK and CaretBlink from TextPolicy.
- TEXT done (code f0236878, record 690b998; run 40226699805: 40 presto-text tests pass on all three OSes, masks
  byte-identical across OSes). Decided: stay on skrifa 0.44 until parley moves; measure dilation amounts [local];
  consolidation 2 moves skrifa/png to the workspace table.
- COMPOSITOR done (code 9a58f1c4, record 4906597c; run 40228927630: every compositor target passes on all three OSes,
  GPU tests included). Q1–Q7 decided as recommended (coordinator, 2026-10-25): consolidation 2 adds `color` and a
  `transform: PropNodeId` to FastCommit::Caret; CSS transform replaces (core omits the local CSS transform while a
  compositor transform animation runs); core never sends UI-placed animations; a11y, cursor and ime_area stay on the
  UI thread (APP-TEST confirms); UiFrameState answers every BeginFrame; APP-TEST sets refresh and ppi from the
  monitor.
- APP-TEST CI on c091234d (contains integration 65ead449): its own crates pass (latency 14, harness 20, render 2 on
  Metal, lib 13). Integration breaks found: (1) pnpm build "Duplicated export 'setp'" in
  experiments/native-compiler/handwritten breaks all five svelters workflows (consolidation 1 fixes it);
  (2) presto-platform-wayland time.rs:84 u64 overflow in a test (WAYLAND).
- WIDGETS-TIER1 done 0876a38f (table, tabs, pickers, dnd, dialogs, zoom; tier-1 tests pass on Linux and macOS; a Windows
  path test fixed in 0876a38f). Open items for consolidation 2 in its record: picker overlay layers, prepare can't emit,
  EventCx can't emit on a child, drag-preview hook, table scrollbars on the views scroll layer, hex/cell editors.
- WIDGETS-VIEWS done 2226e000: spec-driven parts/places_children, scroll through EventCx::scroll_to → core scroll_fast,
  MenuModel::to_menu_desc behind `native-menu`, menu-bar `native-bar`. Runner wiring sent to APP-TEST (via Kind::call).
- TEXT-INPUT done 093ae50f (base merged; code as green 300b2660). Q5 for consolidation 2: a RecordCx::fixed() (or similar)
  so a self_scroll kind can record its box outside the scroll layer; until then a field scroll re-records the node.
- CORE-3 done fa1d7758 (Document::start_drag), WAYLAND dd66a164 (overflow fix), both merged into base 21d341bb.
- CANVAS code complete f208b779 (presto-canvas, Viewport OS-layer promotion with resize handshake, Prim::Canvas, Slug text; locally 10^6 primitives = 542 µs CPU plan per frame (debug), orbit test 0 commits). Decided: Q1 camera handle now, FastCommit::CanvasCamera later; Q2 [local] gates into cadatomic REVISIT.md via CADATOMIC's branch; Q3 polygon AA follow-up.
- MOTION-CSS done ff5f7417: Easing::Svelte(SvelteEasing) in presto-base, presto-anim eval, exact compositor easings. Not mentioned: replacing presto-motion shim::css with presto_anim::css (consolidation 2).
- EGUI-ISLAND ac6d7385 (6c97eabe green on all six workflows): physical_key mapping (Ctrl+C on Cyrillic layouts), device_reset with a CPU texture mirror and a GPU test. start_drag adoption requested.
- MOTION-CSS final 1a016796 (shim::css replaced by presto_anim::css; presto-motion has no shims left). CANVAS ae46a5b1 (base merged). Backend resize hooks (begin/end_os_layer_resize) requested from WAYLAND, MACOS and WIN32.
- EGUI-ISLAND final 1b3ca477 (islands start presto drags via Document::start_drag). WAYLAND 3c81128a (resize hooks). APP-TEST 790d8837 (menu hook, single selection, caret blink); one more push requested.
- COMPILER b625d66b: W-C1/C2/C4/C5/C9 landed behind capabilities (no backend advertises any). Decided: Q1 egui flips templates first at consolidation 2, then ids; Q5 COMPILER's capabilities shape plus MOTION-CSS's constants. COMPILER owns the pnpm setp fix and Q2–Q4 (LISTEN_PASSIVE strip in on/off, an adopt entry, specHash() to JS) as cross-lane edits. WIN32 8968799f (resize hooks).
- SVELTERS-PRESTO 7bdbda9c: OP_ANIMATE native plans on the compositor via new Document::start_animation/cancel_animation, SVL_STYLE_SHEET_BIN via new Document::add_parsed_sheet/preload_utilities (cross-lane additive in presto-core; consolidation 2 reviews their shape). Rule hints wait on COMPILER's template-node map.
- Consolidation 1 resumed: integration f5a6f3c9. It had fixed setp itself in 07e50c77 (one setp; ids() via setp_id), so COMPILER now merges integration and settles F15 instead of re-fixing. F14 (Windows path test) is fixed in tier1 0876a38f; F16 (appkit vis1 embed.js) and F17 (winui bootstrap dll) to recheck.
- Integration e5f2ef3a: the round for CADATOMIC (IslandHost, menu API, TEXT-INPUT, TIER1 ce2f4ecc, CANVAS 838cb29b, SVELTERS-PRESTO 7bdbda9c, MOTION-CSS 1a016796, WAYLAND 3c81128a, APP-TEST 790d8837). Locally presto-core 120, presto-widgets 505, harness 22, motion 18 all pass; CI pending.
- APP-TEST closing 7bd9ee8f: native menu bar end to end (Kind::as_any in core; Kind::methods/call in the widgets shim and host; Document::call menu-activated), set_refresh from the monitor, ppi 96×scale (72×scale on macOS), headless SetMenuBar. Q12 (menu model from child elements) to WIDGETS-VIEWS. Q13 (Monitor::ppi, window→monitor query, MonitorChanged event in presto-platform) to consolidation 2.
- COMPILER c488d386: RUNTIME and MOTION-CSS merged; one setp (RUNTIME's index.js fast path, codegen emits it only for eligible scalar props), replacing consolidation's setp_id; LISTEN_PASSIVE kept out of hkey; adopt(); spec_hash registered by svelters-egui. Native-runtime tests 157/157. WIDGETS-VIEWS bbe7ea5a: menus from element children (Kind::folds_elements); host.rs fix (Hosted::prepare now calls the widget's prepare, so views lay out in real documents).
- CADATOMIC ea2194b: panes as islands in presto_berth's DockView (CADATOMIC_PRESTO_PANES=1); the Start pane seen live as an island in berth's native dock. Blocking presto-core bug sent to CORE-3: nested absolute boxes don't accumulate world positions; berth tab labels empty (maybe clip).
- CORE-3 8ca53bee merged: nested absolute boxes accumulate (taffy's containing-block rule), absolute boxes take their containing block's clip, CSS static maps to taffy Static; presto-core --lib 126/126. Finding: svelters-style has no `overflow` shorthand, so `overflow: hidden` (and WIDGETS-VIEWS' base.css scroll-area overflow, if it uses the shorthand) is a no-op (consolidation 2).
- CADATOMIC: lock on integration 0890801c; REVISIT [local] gates committed (a0fb8da); CORE-3 8ca53bee verified (presto_shell 3/3, dock placed correctly live). CADATOMIC fixes the empty presto_berth tab title on berth's branch, then does the palette modal.
- WAYLAND done 3c81128a: all six workflows green (presto 40239616922, js 40239616939, gtk 40239616899, qt 40239616920, appkit 40239616889, winui 40239616903). The head contains base, the backend and CANVAS ae46a5b1. Its questions 2–8 go to consolidation 2.
- WIDGETS-TIER1 final ce2f4ecc: presto green on all three OSes (40239360492), js/gtk/qt/winui green; macos-appkit 40239360509 red in step 'appkit -> counter-mac backend', the second time (also FRAME-ANIM on a9503acb), while WAYLAND 3c81128a (which contains base) was green there. Intermittent; consolidation 1 checks the log and records it as a flake candidate with its cause.
- CADATOMIC 738f931: empty tab title fixed (the document had no presto_core::TextShaper; host::DocShaper adapter); native palette
  modal landed (presto_shell 4/4). Bug sent to COMPOSITOR: with a text-edit node in the window, the live window draws
  black except the caret (suspect caret-only damage against buffer age). TEXT adds impl presto_core::TextShaper for
  TextSystem.
- Naming: "presto" is taken on crates.io. The user proposed **skelter** (svelters-skelter); skelter and skelter-{core,base,
  render,widgets,platform,text,compositor,test} are free on crates.io, and skelter on npm. The rename is planned for
  consolidation 2, once, before the master merge, pending the user's confirmation.
