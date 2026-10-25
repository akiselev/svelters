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
