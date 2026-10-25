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
| 14 | `ci/presto-svelters-presto` | 80189479 | conflict in `sync.rs`: took the lane's (it ports to the lent document, superseding B2) |

## Build fixes made

| # | Crate | Fix |
|---|---|---|
| B1 | svelters-compiler | `compile.rs`: duplicate `use crate::semantics::capability;` after the COMPILER + MOTION-CSS merge (E0252); removed one. In the motion-css merge commit. |
| B2 | svelters-presto | `sync.rs` `install_sync_delivery`: CORE-3's `SyncDelivery` is now `FnMut(&mut Document, &JsEvent)`; the closure takes `_doc` and ignores it (E0593). Commit 095d27dc. |

## Queue

| # | Crate | Test or symptom | Owner | Suggested fix |
|---|---|---|---|---|
| F1 | presto-compositor | `tests/engine.rs` `a_fling_keeps_producing_frames_without_commits_then_goes_idle`: "2 frames for a 53-frame fling" (engine.rs:308) | COMPOSITOR | Present on `ci/presto-base` (no lane merge touched the crate). Likely the frame-source/BeginFrame back-pressure change (3cbd9595, 2b98a948) stops ticking an animation-only window; keep requesting frames while a fling is active. |
| F2 | presto-compositor | `tests/tree_damage.rs` `diff_damages_only_what_changed`: damage has two extra rects `(105,100)-(125,120)` beside the expected `(0,0)-(10,10)` (tree_damage.rs:235) | COMPOSITOR | Present on base. The caret-blink layer (9b26937d) or a fast-commit path damages a node that didn't change; diff should skip unchanged layers. |
| F3b | presto-core | `tests::hooks::a_press_outside_an_open_surface_dismisses_it` (hooks.rs:798, "and is dispatched as usual") and `tests::hooks::one_selection_owner_under_the_gtk_rule` (hooks.rs:810, "off by default: both keep their selection"): new tests from base 4756b7ca | CORE-3 | Present on base (presto-core is identical to base). The OutsidePress and selection-owner halves. |
| F3 | presto-core | `tests::hooks::checking_a_radio_unchecks_its_peers`: got `[true,false,false]`, want `[false,true,false]` (hooks.rs:377) | CORE-3 | Present on base. Since the lent-document sync delivery (d39eaf4b) the peer uncheck/new check doesn't land; check the order of the native default vs the peer hook. |
| F4 | presto-core | `tests::hooks::a_drag_moves_from_source_to_target`: `DROP_TARGET` state not set on the target (hooks.rs:557) | CORE-3 | Present on base. The drag/anchor members: the drop target hook isn't reached or doesn't set the state. |
| F5 | presto-core | `tests::style::paint_transitions_sample_on_the_ui_thread`: `Busy::ANIMATING` still set after the transition should have ended (style.rs:135) | CORE-2 | Present on base. Clear ANIMATING when the last transition finishes (transitionend path, f790788a). |
| F6 (fixed at 4756b7ca/d3f510ef: the slider tests pass) | presto-widgets | `controls::slider::tests::{dragging_the_thumb_changes_the_value, escape_while_dragging_restores_without_a_commit, inverted_page_clicks_and_commit_mode}`: 69.9999988 vs 70, 89.9999976 vs 90 | WIDGETS-BASIC (stopped; route to whoever owns presto-widgets controls) | Present on base since the cancelable `changed` (09fc1bfb): the value goes through an f32 (position math) without snapping. Round to the step (or compare with a tolerance in the tests). |
| F7 (done by the lane, 80189479) | svelters-presto | B2 is a build fix only: the lane still defers JS ops and ignores the lent document | SVELTERS-PRESTO | Port `deliver_cancelable` to the lent `&mut Document` (sync delivery decision, coordinator 2026-10-25), so reads inside a cancelable listener work instead of the "cannot be read" error. |
| F8 | presto | APP-TEST request 11: TextServices not wired (`crates/presto/src/app.rs:570` TODO) | consolidation 2 or APP-TEST | As decided: TextServices owns its `SystemClipboard` (`MemoryClipboard` headless) and a compositor handle clone; install it in `App::init`. Not done here (not a build break). |
| F9 | presto | WAYLAND Q1: the facade doesn't select the platform backend yet | consolidation 2 or APP-TEST | Target-cfg deps on presto-platform-{win32,wayland,macos} with default-on features and the winit fallback, selection in `App`. Not done here. |
| F10 | presto-widgets | `dialogs` feature (rfd) is not built by any check here or in presto.yml (it builds default features) | WIDGETS-TIER1 | Add a `cargo check -p presto-widgets --features dialogs` step to presto.yml (rfd needs GTK or xdg-portal deps on Linux). |
| F11 | presto_berth, presto_quire | Tests not run (berth `claude/presto-berth` a49111b tests/dock.rs + check.sh; quire 35b5063a/676b49bc, 43 tests); their masters not merged | consolidation 2 | Out of this brief (svelters-only branch); run and merge in consolidation 2. |
| F12 | presto-compositor | COMPOSITOR 2b98a948 was merged into base before its CI finished (coordinator list) | COMPOSITOR | F1/F2 are the result; check the lane's CI run once fixed. |
| F13 | presto-widgets | warning: unused import `StateBits` (`controls/box_.rs:12`) | WIDGETS-BASIC | Remove it (clippy is advisory now). |

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

## CI

See the coordinator report (filled in below once the runs finish).
