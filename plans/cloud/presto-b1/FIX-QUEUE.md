# presto-b1 fix queue (consolidation 1, 2026-10-25)

Branch `ci/presto-integration`, built from `ci/presto-base` 82e166e2 plus the eleven lane heads below, then (at the
coordinator's request) `ci/presto-base` 4756b7ca (CORE-3 e2b96936 and later) and the newer heads of three lanes (rows 12–14). Consolidation 1
fixed build breaks only; everything else is listed here for the coordinator to route.

## Merged heads

| # | Lane branch | Head | Merge |
|---|---|---|---|
| 1 | `ci/presto-runtime` | 4c65de8d | clean |
| 2 | `ci/presto-compiler` | 949dcf1d | clean |
| 3 | `ci/presto-text-input` | ee937bf2 | clean |
| 4 | `ci/presto-widgets-views` | ae617f24 | clean |
| 5 | `ci/presto-widgets-tier1` | c9e8694e | conflict in `presto-widgets/Cargo.toml` (quire-text + svelters-conformance dev-dep vs rfd + `dialogs` feature): kept both |
| 6 | `ci/presto-svelters-presto` | 025eca5c | clean (but a build break, see B2) |
| 7 | `ci/presto-egui-island` | 0dd5a347 | clean |
| 8 | `ci/presto-motion-css` | 3f223c27 | conflicts in `svelters-compiler/src/semantics/{backend,mod,overlay,spec}.rs`: kept COMPILER's `spec_hash`/`kind_id`/`a11y`/`ChangeClass`/`A11ySpec` and added MOTION-CSS's three capabilities |
| 9 | `ci/presto-win32` | 07bb50d1 | clean |
| 10 | `ci/presto-wayland` | fd2acebc | clean |
| 11 | `ci/presto-macos` | bef22840 | clean |
| — | `ci/presto-base` | 4756b7ca | clean |
| 12 | `ci/presto-text-input` | 300b2660 | clean |
| 13 | `ci/presto-widgets-tier1` | d3f510ef | clean |
| 15 | `ci/presto-app-test` | c091234d | clean (after base 4aa01eb3) |
| 14 | `ci/presto-svelters-presto` | 80189479 | conflict in `sync.rs`: took the lane's (it ports to the lent document, superseding B2) |
| — | `ci/presto-base` | bf119bdf | conflict in `presto-platform-wayland/src/time.rs` (my B4 vs WAYLAND's dd66a164): took WAYLAND's |
| 16 | `ci/presto-egui-island` | ac6d7385 | clean |
| 17 | `ci/presto-widgets-views` | 2226e000 | conflict in `presto-widgets/Cargo.toml` (optional presto-platform + `native-menu` vs TEXT-INPUT/TIER1 entries): kept all, then B5 |
| 18 | `ci/presto-macos` ff854b58, `ci/presto-win32` 305eb732, `ci/presto-wayland` dd66a164 | — | already in through base |
| 19 | `ci/presto-test-v2` | b0754e75 | conflict in `presto/src/app.rs`: test-v2's `AppHandler` (`arm_timer`, `*_one` helpers, timer pass) with APP-TEST's clipboard install in `init`, `flush_fast` after input and window events, and the hover-at-event-time branch in `input_one` |
| 20 | `ci/presto-canvas` | f208b779 | clean |
| — | `ci/presto-base` | a8154099 | clean (third round) |
| 21 | `ci/presto-text-input` | 093ae50f | clean |
| 22 | `ci/presto-widgets-tier1` | ce2f4ecc (contains 0876a38f) | clean |
| 23 | `ci/presto-svelters-presto` | 7bdbda9c | clean |
| 24 | `ci/presto-egui-island` | 1b3ca477 | clean |
| 25 | `ci/presto-motion-css` | 1a016796 | clean |
| 26 | `ci/presto-wayland` | 3c81128a | clean |
| 27 | `ci/presto-canvas` | 838cb29b (contains ae46a5b1) | clean |
| 28 | `ci/presto-app-test` | 790d8837 | conflict in `presto-test/src/harness.rs`: kept test-v2's harness, added app-test's `menu_activated` |
| 29 | `ci/presto-win32` | 8968799f | clean |
| 30 | `ci/presto-compiler` | c488d386, then dc3e8187 (c488d386 + integration 921d51c4, same ids.js/index.js; merged clean, record only) (RUNTIME 4c65de8d and MOTION-CSS 1a016796 in, F15 settled) | conflicts in native-runtime `ids.js`, `index.js`: took COMPILER's (one `setp`, RUNTIME's scalar path; B3's `setp_id` gone, nothing else used it) |
| 31 | `ci/presto-app-test` | 7bd9ee8f | conflicts in `Cargo.lock` (regenerated), `presto-test/Cargo.toml` (kept the island/automation deps, added presto-core and presto-widgets `native-menu` dev-deps), `presto-test/src/harness.rs` (kept the `pub(crate)` fields, added `native_menu_bar`), `presto-widgets/Cargo.toml` (comment only), `presto/src/app.rs` (kept test-v2's `*_one` helpers, folded in app-test's `sync_refresh` on scale change and move) |
| 32 | `ci/presto-widgets-views` | bbe7ea5a | clean |
| — | `ci/presto-base` | 606cc569 (CORE-3 8ca53bee absolute boxes, presto.yml setup-pnpm 67df412a) | clean; my duplicate setup-pnpm step removed |
| 33 | `ci/presto-macos` | c36101b4 | clean |
| 34 | `ci/presto-test-v2` | 9a4f1213 | clean |

## Build fixes made

| # | Crate | Fix |
|---|---|---|
| B1 | svelters-compiler | `compile.rs`: duplicate `use crate::semantics::capability;` after the COMPILER + MOTION-CSS merge (E0252); removed one. In the motion-css merge commit. |
| B2 | svelters-presto | `sync.rs` `install_sync_delivery`: CORE-3's `SyncDelivery` is now `FnMut(&mut Document, &JsEvent)`; the closure takes `_doc` and ignores it (E0593). Commit 095d27dc. |
| B3 | @svelters/native-runtime | `setp` exported twice (COMPILER's `ids.js` W-C1 id write vs RUNTIME's W-C3 typed scalar write in `index.js`); rolldown rejected the bundle ("Export has already been declared"), failing every workflow's JS build at bc7215cb. One `setp` now: an id a module declared with `ids()` goes through COMPILER's path (`setp_id`, by name through `set`), everything else through RUNTIME's. |
| B4 | presto-platform-wayland | `time.rs` test `presentation_and_commit_timer_round_trip`: `((7 << 32) + 11) s` in u64 nanoseconds overflows (deny `arithmetic_overflow`, lib test didn't compile on ubuntu presto). Now `(3 << 32) + 11` (still a non-zero high word). |
| B5 | presto-widgets | `Cargo.toml` duplicate key `presto-platform` (APP-TEST made it a required dependency for the clipboard; WIDGETS-VIEWS added it as optional for `native-menu`). Kept the required one; `native-menu = []` now only gates the code. |
| B6 | @svelters/style | js at 2f22c1e9: `unsupported` golden predates presto as a style toolkit (every "it is one on …" list now ends in presto). Regenerated with UPDATE=1; the diff only adds ", presto"; the other goldens (card, native, setters) were current. The svelters-cli `no such command: nextest` in the same job was fallout (install-action has no `if: !cancelled()`, also on master). |
| B7 | presto.yml | presto-test `automation` needs the pnpm workspace (`svelte` via native-runtime's aliases). Fixed in base 67df412a (setup-pnpm), which this round takes. |

## Queue

| # | Crate | Test or symptom | Owner | Suggested fix |
|---|---|---|---|---|
| F1 (fixed: COMPOSITOR 9a58f1c4, base 177731fd; passes) | presto-compositor | `tests/engine.rs` `a_fling_keeps_producing_frames_without_commits_then_goes_idle`: "2 frames for a 53-frame fling" (engine.rs:308) | COMPOSITOR | Present on `ci/presto-base` (no lane merge touched the crate). Likely the frame-source/BeginFrame back-pressure change (3cbd9595, 2b98a948) stops ticking an animation-only window; keep requesting frames while a fling is active. |
| F2 (fixed: COMPOSITOR 9a58f1c4, base 177731fd; passes) | presto-compositor | `tests/tree_damage.rs` `diff_damages_only_what_changed`: damage has two extra rects `(105,100)-(125,120)` beside the expected `(0,0)-(10,10)` (tree_damage.rs:235) | COMPOSITOR | Present on base. The caret-blink layer (9b26937d) or a fast-commit path damages a node that didn't change; diff should skip unchanged layers. |
| F3b (fixed: CORE-3 final, base 177731fd; passes) | presto-core | `tests::hooks::a_press_outside_an_open_surface_dismisses_it` (hooks.rs:798, "and is dispatched as usual") and `tests::hooks::one_selection_owner_under_the_gtk_rule` (hooks.rs:810, "off by default: both keep their selection"): new tests from base 4756b7ca | CORE-3 | Present on base (presto-core is identical to base). The OutsidePress and selection-owner halves. |
| F3 (fixed: CORE-3 final, base 177731fd; passes) | presto-core | `tests::hooks::checking_a_radio_unchecks_its_peers`: got `[true,false,false]`, want `[false,true,false]` (hooks.rs:377) | CORE-3 | Present on base. Since the lent-document sync delivery (d39eaf4b) the peer uncheck/new check doesn't land; check the order of the native default vs the peer hook. |
| F4 (fixed: CORE-3 final, base 177731fd; passes) | presto-core | `tests::hooks::a_drag_moves_from_source_to_target`: `DROP_TARGET` state not set on the target (hooks.rs:557) | CORE-3 | Present on base. The drag/anchor members: the drop target hook isn't reached or doesn't set the state. |
| F5 (fixed: CORE-3 final, base 177731fd; passes) | presto-core | `tests::style::paint_transitions_sample_on_the_ui_thread`: `Busy::ANIMATING` still set after the transition should have ended (style.rs:135) | CORE-2 | Present on base. Clear ANIMATING when the last transition finishes (transitionend path, f790788a). |
| F6 (fixed: WIDGETS-BASIC 966d4d75; `cargo test -p presto-widgets slider` 19/19 on the integration head) | presto-widgets | `controls::slider::tests::{dragging_the_thumb_changes_the_value, escape_while_dragging_restores_without_a_commit, inverted_page_clicks_and_commit_mode}`: 69.9999988 vs 70, 89.9999976 vs 90 | WIDGETS-BASIC (stopped; route to whoever owns presto-widgets controls) | Present on base since the cancelable `changed` (09fc1bfb): the value goes through an f32 (position math) without snapping. Round to the step (or compare with a tolerance in the tests). |
| F7 (done by the lane, 80189479) | svelters-presto | B2 is a build fix only: the lane still defers JS ops and ignores the lent document | SVELTERS-PRESTO | Port `deliver_cancelable` to the lent `&mut Document` (sync delivery decision, coordinator 2026-10-25), so reads inside a cancelable listener work instead of the "cannot be read" error. |
| F8 (fixed: APP-TEST c091234d, `host::install` in `CoreDocument::with_widgets`, clipboard as a document service, fast commits drained to the compositor) | presto | APP-TEST request 11: TextServices not wired (`crates/presto/src/app.rs:570` TODO) | consolidation 2 or APP-TEST | As decided: TextServices owns its `SystemClipboard` (`MemoryClipboard` headless) and a compositor handle clone; install it in `App::init`. Not done here (not a build break). |
| F9 (fixed: APP-TEST c091234d, `presto::run` picks Win32/AppKit/Wayland, winit fallback, `PRESTO_BACKEND`) | presto | WAYLAND Q1: the facade doesn't select the platform backend yet | consolidation 2 or APP-TEST | Target-cfg deps on presto-platform-{win32,wayland,macos} with default-on features and the winit fallback, selection in `App`. Not done here. |
| F10 (done: presto.yml checks `--features dialogs`, base) | presto-widgets | `dialogs` feature (rfd) is not built by any check here or in presto.yml (it builds default features) | WIDGETS-TIER1 | Add a `cargo check -p presto-widgets --features dialogs` step to presto.yml (rfd needs GTK or xdg-portal deps on Linux). |
| F11 (routed to consolidation 2) | presto_berth, presto_quire | Tests not run (berth `claude/presto-berth` a49111b tests/dock.rs + check.sh; quire 35b5063a/676b49bc, 43 tests); their masters not merged | consolidation 2 | Out of this brief (svelters-only branch); run and merge in consolidation 2. |
| F12 (superseded by the F1/F2 fix) | presto-compositor | COMPOSITOR 2b98a948 was merged into base before its CI finished (coordinator list) | COMPOSITOR | F1/F2 are the result; check the lane's CI run once fixed. |
| F13 (routed to consolidation 2) | presto-widgets | warning: unused import `StateBits` (`controls/box_.rs:12`) | WIDGETS-BASIC | Remove it (clippy is advisory now). |
| F14 (fixed in widgets-tier1 0876a38f, merged; recheck on CI) | presto-widgets | Windows only (presto windows-latest, run 40229342780): `dialogs::native::tests::an_installed_backend_answers_and_cancel_is_none` at `dialogs/native.rs:271`: `left: Ok(Some("/out\\x.kicad_pcb"))`, `right: Ok(Some("/out/x.kicad_pcb"))`; lib 502 / 1 | WIDGETS-TIER1 | The test joins with `Path::join` (backslash on Windows) but expects `/`; build the expected value with `Path::new("/out").join("x.kicad_pcb")` or compare as paths. |
| F15 (settled by COMPILER c488d386/dc3e8187, merged) | @svelters/native-runtime, svelters-compiler | B3 settles the `setp` clash by routing; the two lanes should agree on one contract (do spec stable ids reach `host.setp` directly once hosts take them, and is W-C3's typed-scalar `setp` a separate op name?) | COMPILER + RUNTIME | Decide whether the compiler emits a distinct op for W-C3 writes (e.g. `$n.setv`) and keep `setp` for W-C1 ids, or retire `setp_id` once `host.setp` takes spec ids. |
| F16 | svelters-testkit (macos-appkit) | bc7215cb, run 40229342651: `vis1` `own_widgets_match_vscode` panicked at `crates/svelters-testkit/tests/vis1.rs:54`: `fixtures/gallery-egui/dist/embed.js is missing: run pnpm --filter ./fixtures/gallery-egui build` | consolidation (recheck) | Most likely downstream of B3 (the bundle builds failed). Recheck on the next run; if it stays, the appkit job needs the gallery-egui build step. |
| F17 (gone: windows-winui green on 2f22c1e9, run 40241055307) | svelters-winui (windows-winui) | bc7215cb, run 40229342723: `cargo test -p svelters-winui --test generated`: `WinUI init failed: LoadLibraryW(Microsoft.WindowsAppRuntime.Bootstrap.dll): The specified module could not be found. (HRESULT 0x8007007E)` | consolidation (recheck) | Green on master. Recheck on the next run; if it stays, compare the bootstrap fetch step with master's. |
| F18 | svelters-style | No `overflow` shorthand: `overflow: hidden` in app CSS is a no-op. presto-widgets' base.css uses `overflow-x`/`overflow-y` only (scroll containers fine); the `overflow` utility expands to both | svelters-style (fix lane or consolidation 2) | Add the shorthand (one or two values onto overflow-x/overflow-y). |
| F19 | presto-test | GPU only (PRESTO_GPU_TESTS=1; ubuntu, macos, windows, run 40241095094): `island_render` `an_island_pass_reaches_the_pixels` panicked at `tests/island_render.rs:48`: `blue after the next pass: [255, 0, 0, 255]` | TEST-V2 (sent by the coordinator), with EGUI-ISLAND | The island's second pass doesn't reach the pixels: check the island texture re-upload/damage after a repaint request. |

## Local results (one run, f5d7ab0c)

`cargo check --workspace --all-targets` excluding the GTK/Qt crates (svelters-gtk, svelters-qt-build, counter,
counter-qt, svelters-conformance, exports-example; the container has no GTK dev packages): clean. Win32 and macOS
crates are cfg-gated and were only checked on Linux; CI checks them.

`cargo test` on the eleven presto crates (passed / failed / ignored):

| Crate | Result |
|---|---|
| presto-base | 0 / 0 / 0 |
| presto-core (lib) | 103 / 3 / 0 (F3–F5) |
| presto-text | 40 / 0 / 0 |
| presto-anim | 37 / 0 / 0 |
| presto-frame | 9 / 0 / 0 |
| presto-compositor | caret 4, engine 16 / 1 (F1), headless 1, tree_damage 8 / 1 (F2) |
| presto-platform | 21 / 0 / 0 |
| presto-widgets | lib 495 / 3 / 0 (F6), text_pins 1 / 0, doc 0 / 0 / 2 |
| presto | latency 14, svelte 9 / 0 |
| presto-test | sim 17, harness 18, render 2 / 0 |
| svelters-presto | lib 6, deliver 5, oplog_replay 1, ops 11, run 2, spec 4, template 4 / 0 |

## Rerun after the base and new heads (presto-core, presto-widgets, svelters-presto only)

presto-core lib 114 / 5 (F3, F3b ×2, F4, F5); presto-widgets lib 503 / 0, text_pins 1 / 0; svelters-presto lib 6,
deliver 6, oplog_replay 1, ops 11, run 2, spec 4, template 4 / 0. The other crates are unchanged since the first run.

## Rerun after base 177731fd (via 4aa01eb3; the five base failures and the slider tests only)

`cargo test -p presto-core --lib tests::` 119 / 0; compositor `engine a_fling` 2 / 0 and `tree_damage diff_damages` 1 / 0;
`cargo test -p presto-widgets slider` 19 / 0 (the CORE-3 report of three failing slider tests does not reproduce here).

## After APP-TEST c091234d

Workspace check clean (GTK/Qt crates excluded). `cargo test -p presto -p presto-test`: latency 14 / 0, harness 20 / 0,
render 2 / 0.

## After base bf119bdf and the second lane heads (crates the merges touched)

presto latency 14; presto-canvas 13, million 2, text 3; compositor caret 4, engine 17, headless 2, tree_damage 10, viewport 4;
presto-core 120; presto-egui-island host 3, input 9, island 11, render 2; presto-platform 21; presto-platform-wayland 54;
presto-render 26, canvas 4, gpu 13; presto-test automation 2, harness 20, island 7, island_render 3, queries 8, render 2,
snapshot 2; presto-widgets 505, text_pins 1. All 0 failed. macOS/Win32 crates are cfg-gated (CI only).

## Third round (e5f2ef3a, crates the merges touched)

presto latency 14; presto-anim 38, svelte 9; presto-core 120; presto-egui-island host 4, input 9, island 11, render 2;
presto-motion motion 10, svelte_fidelity 8; presto-platform-wayland 54; presto-test automation 2, harness 22, island 7,
island_render 3, queries 8, render 2, snapshot 2; presto-widgets 505, text_pins 1; svelters-presto lib 6, deliver 6,
motion 4, oplog_replay 1, ops 11, run 2, spec 4, template 4. All 0 failed.

## Fourth round (compiler, app-test, widgets-views)

presto latency 14; presto-core 120; presto-platform 21; presto-test automation 2, harness 22, island 7, island_render 3, menu 3,
queries 8, render 2, snapshot 2; presto-widgets 508, text_pins 1. All 0 failed. `pnpm -r --filter '!@svelters/docs' build`
exit 0; native-runtime tests 159 / 159.

## Round 5 (base 606cc569, macos c36101b4, test-v2 9a4f1213, B6)

presto latency 14; presto-core 126; presto-test automation 2, harness 22, island 8, island_render 3 (GPU path skipped
locally), menu 3, queries 8, render 2, snapshot 2 and the rest; all 0 failed.

CI at 2f22c1e9 (svelters) / 0890801c (presto, same code): linux-qt success 40241055328, windows-winui success
40241055307, js failure 40241055330 (B6), presto failure 40241095094 (B7, F19); linux-gtk and macos-appkit were still running
when round 5 was pushed.

## CI

See the coordinator report (filled in below once the runs finish).
