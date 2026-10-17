# P5 integration: working notes

## What exists

- `examples/counter` (`cargo run -p counter -- --backend gtk|qt ...`): loads `fixtures/<name>/dist/app.js` (or a directory path with `dist/app.js`) into `svelters_host::Host` and runs it on `svelters-gtk` or on `examples/counter-qt`'s `backend()`. Features `gtk` and `qt` (both default) let a build contain one toolkit only, which is how the per-backend binary sizes were taken.
- Without `--script`, `--screenshot` and `--report` it is an ordinary app that runs until its window closes. With any of them a driver (`examples/counter/src/driver.rs`) runs inside the toolkit loop:
  - `{"event": [h, "clicked", []]}` becomes a real button click (GTK `Button::emit_clicked`, Qt `QAbstractButton::click()` through the backend's `click` method);
  - `{"event": [h, "changed", [text]]}` types the text into the real entry (GTK `Editable::insert_text` per char after selecting all; Qt `QKeyEvent` per char);
  - `{"checkpoint": true}` waits until the host is idle (no queued events, no pending timers, 3 consecutive 10 ms polls);
  - at the end it reads the widget tree back from the toolkit (`dump` in the JSON report: `{d, class, text}` per widget in layout order), optionally saves a PNG, and either asks the backend to quit (`--end quit`) or closes the window natively (`--end close`) so that `run()` has to return by itself.
- `--timing` turns on the time split counters (below). `--report <json>` writes everything the tests assert on.
- Handles in scripts are backend handles. Both backends number handles 1, 2, ... in creation order, which is the order in the fixtures' `expected.oplog`.
- `examples/counter/tests/integration.rs`: 12 tests (counter, keyed-list incl. reverse/double reverse, keyed-stress, timers on both backends, if-toggle on GTK, the clean Qt error for `headerbar`, native-close on both). Run with `xvfb-run -a cargo nextest run -p counter` after `pnpm install && pnpm -r build`. Screenshots: `target/p5/<backend>-<fixture>.png` (plus `-reverse`, `-close` variants for the extra runs).

## Qt tooling hooks (generated backend, not ABI)

`tools/metadata/qt/templates/backend.cpp.in` gained non-ABI C entry points used by tests and the counter binary: `svl_qt_dump`, `svl_qt_grab_png` (moved from the verify template, null path = render only), `svl_qt_type_text`, `svl_qt_close`, `svl_qt_run_later`, `svl_qt_on_first_paint`, `svl_qt_set_timing`/`svl_qt_take_timing`. Safe wrappers: `counter_qt::hooks`. `counter_qt::run_as_test` implements the minimal libtest command line (`--list`, `--exact`, filters) so the `harness = false` Qt tests list and run under `cargo nextest` (one nextest test each: `counter-qt::generated verify_all`, `counter-qt::counter counter_demo`).

## Bugs found and fixed (all minimal)

1. **Host never scheduled a checkpoint after a native event** (`svelters-host/src/lib.rs`). `post_event` only queued the event; nothing woke the loop. The log backend and the P3/P4 test stubs checkpoint by themselves, so this only showed with a live toolkit. `post_event` now calls `backend.wake()` unless the host is in scripted mode.
2. **GTK `changed` carried no text** (`svelters-gtk/src/backend.rs`). GtkEditable::changed has no arguments, but the JS contract (and the Qt backend) delivers `[text]`; the keyed-list `draft` became `undefined` and the app then wrote `null` to `entry.text` (GTK-CRITICAL `gtk_editable_set_text`). `deliver` now appends the editable's text for `changed`.
3. **GTK natively destroyed window left its descendants live** (coordinator note). `native_destroyed` now invalidates every handle whose widget is a descendant of the window (children first), drops the table's strong reference and reports each through `on_destroyed`. Toplevels stay weakly held.
4. **Qt `run()` queued no checkpoint unless a wake was pending** (`backend.cpp.in`). A bundle that settles during `load_bundle` never woke the backend, so nothing ran in the loop. GTK does this in `activate`. `run` now always queues exactly one checkpoint (a `queued` flag avoids doubling when `wake` already queued one, which the P4 demo test counts).
5. **Shim `before()`/`after()` computed the index before detaching the moved node** (`packages/dom/src/index.js`), the first of the two bugs from `custom-renderer-trial.md` section 4.3. Keyed reorders produced the wrong order on the shim path. The new `keyed-stress` fixture fails on the shim without the fix and passes with it. The second bug (`before()` on a parentless node must be a no-op) was already right in the shim.
6. **Renderer was quadratic on reorders** (coordinator note). `children` arrays with `indexOf`/`splice` and a full `flat(parent)` scan per native insert are replaced by a doubly linked child list (`first/last/prev/next`) and a forward walk to the next realized sibling. Reverse of N rows on the TestHost, before to after: 1000 rows 212 ms to 46 ms, 2000 rows 843 ms to 65 ms, 10000 rows 790 ms (the survey measured ~85 s before, through Svelte). Unit tests: a seeded insert/remove/move stress compared against a replay of the host ops, and a scaling test (`packages/renderer/test/renderer.test.mjs`); `packages/renderer/test/reorder-bench.mjs` is the manual benchmark.

Renderer ordering check (task 6): the renderer computes the anchor after detaching the node, a parentless anchor target is an error only when the anchor has another parent, and fragments move their children in order. The 400-step randomized test and `keyed-stress` (reverse, insert middle/head/tail, remove, sort, rotate, filter) agree with the native order on both toolkits.

## Not done / open

- **Proxy values and bare boolean attributes** (survey findings): not fixed. Objects are rejected by `coerce` with a TypeError today, so a deep `$state` object is a clear error rather than a corrupt value. Bare boolean attributes arrive as `""` and cannot be told from an intentional empty string without per-prop types; that needs the M3 metadata (prop type `bool`), so the coercion belongs in the typed-prop layer, not in the renderer.
- **`.svelte` typecheck of element props**: the generated declarations are in `packages/renderer/types/{gtk,qt,index}.d.ts` (export `@svelters/renderer/types`, which wires `svelteHTML.IntrinsicElements` to `GtkElements`). They compile under `tsc --strict`. `svelte-check` is not installed in the workspace (no network in this lane), so fixtures are not checked against them yet.

## Baselines (2026-10-17)

Machine: shared 4-core Linux box (other lanes build and test at the same time, so absolute times are noisy), Xvfb, GTK 4.22.4 with the software GL renderer (llvmpipe), Qt 6.11. Release builds (`cargo build --release -p counter`, default profile: opt-level 3, no LTO, no strip). Raw reports: `target/p5/*.json`.

| What | GTK4 | Qt6 |
| --- | --- | --- |
| Cold start to first frame (counter fixture, median of 6; process start to first painted frame) | ~1.16 s (0.92-1.86 s). With `GSK_RENDERER=cairo`: ~0.69 s | ~0.20 s (0.18-0.27 s) |
| ... of which: backend init + bundle load + first mount (`load_ms`) | ~1.04 s | ~0.19 s |
| Release binary, one toolkit only (`--no-default-features --features gtk|qt`) | 3,395,848 B (2,646,192 stripped) | 2,561,112 B (2,161,248 stripped) |
| Release binary, both backends | 3,492,800 B | (same binary) |
| Peak RSS, counter fixture | 204 MB (137 MB with the cairo renderer) | 55 MB |
| Peak RSS after the 1000-row bench | 259 MB | 82 MB |

All binaries link GTK or Qt dynamically; the sizes exclude those libraries. The first-frame time on GTK is dominated by GTK/GL initialisation (`GtkBackend::new` + renderer start under Xvfb), not by QuickJS or the bundle: the host's own work for the counter fixture is a few ms.

Bundle sizes (`fixtures/*/dist/app.js`, ES module with Svelte runtime + renderer + app, `minify: false`):

| Bundle | bytes | gzip -9 |
| --- | --- | --- |
| counter | 147,097 | 36,602 |
| timers | 147,834 | 36,707 |
| if-toggle | 144,608 | 36,064 |
| keyed-list | 171,306 | 42,309 |
| keyed-stress | 168,258 | 41,536 |
| bench/keyed-bench | 166,660 | 41,780 |
| counter, esbuild `--minify` post-pass | 46,390 | 17,331 |
| keyed-list, esbuild `--minify` post-pass | 54,553 | 20,357 |

Vite's lib-mode ES output is not minified even with `build.minify` on (the package sets `minify: false`); the minified numbers are an `esbuild --minify` pass over `dist/app.js`. The host loads the unminified bundle today (QuickJS parse cost not measured separately).

### 1000-row keyed-list update: where the time goes

`bench/keyed-bench` (1000 rows, each a box with a label and a button, so 3000 widgets) driven by `counter --timing --script bench/keyed-bench/script.json`. Columns, in ms, for one click (release build):

- **JS**: QuickJS time inside entries into JS minus time inside host ops: the app handler, Svelte's flush/reconcile, and the renderer's own JS.
- **host/ABI**: time inside host ops minus time inside the backend calls: handle table, name-cache lookups, value conversion, ABI call. The two clock reads per op are included in this number (about 11000 ops: tens of ns each).
- **toolkit calls**: time measured inside the backend vtable functions (GTK `run_int` scope, Qt `TimeScope` in each `b_*`).
- **toolkit deferred**: the rest of the wall time from the click to the first idle poll: Qt's event handling, show/polish and layout after the checkpoint. On GTK the frame clock repaints after the idle poll (lower priority than timers), so this column stays near 0; use the render probe column there: the time to lay out and render the window synchronously after the step.

| Step (ops) | | JS | host/ABI | toolkit calls | toolkit deferred | render probe |
| --- | --- | --- | --- | --- | --- | --- |
| Fill 1000 (11000) | GTK | 1479 | 16 | 423 | ~0 | 75 |
| | Qt | 1907 | 18 | 226 | 2767 | 6 |
| Update every text (1000) | GTK | 320 | 2 | 23 | ~0 | 8 |
| | Qt | 334 | 2 | 12 | 108 | 8 |
| Reverse (1998) | GTK | 1741 | 4 | 144 | ~0 | 7 |
| | Qt | 1846 | 6 | 1393 | 5560 | 5 |
| Swap 2 rows (4) | GTK | 141 | 0 | 1 | ~0 | 7 |
| | Qt | 159 | 0 | 3 | 24 | 5 |
| Clear (4000) | GTK | 140 | 4 | 182 | ~0 | 2 |
| | Qt | 142 | 3 | 368 | 70 | 1 |

Findings:

1. **QuickJS time dominates** everything that touches the list, and the host/ABI layer is negligible (0.1-1.5 % of the JS time). The JIT research question (`quickjs-jit-cranelift.md`) therefore has a clear answer for this workload: only JS execution is worth accelerating.
2. **Reverse is quadratic inside Svelte, not in our code.** `reconcile` in Svelte's keyed each (`svelte/internal/client/dom/blocks/each.js`) spends 315 ms per 1000-row reverse on V8 (profile of 6 reverses: 1889 ms self time in `reconcile`; it re-scans the `stashed` list after each partial move). QuickJS needs 1.8 s for the same thing. Our renderer's share is linear after the sibling-link fix (46 ms to reverse 1000 rows through the bare renderer API on the TestHost). Swap and Update, which avoid that path, take 140-330 ms. For comparison, the same script on Node/V8 through the TestHost (`packages/testhost/bin/time-bundle.mjs bench/keyed-bench`): fill 450 ms, update 330 ms, reverse ~410 ms, swap ~120 ms, clear ~100 ms per step including op-log building, i.e. QuickJS is roughly 3-5x slower on this workload.
3. **Qt reorders are very slow in the toolkit** (reverse: 1.4 s in calls and 5.6 s deferred). The P4 `remove` descriptor reparents the widget to null after `QBoxLayout::removeWidget`, so every moved row is hidden and then re-shown, which repolishes and relayouts it. A move within one layout should skip the detach (M5). GTK moves cost 144 ms for the same operation.
4. Fill creates 3000 widgets in 0.4 s of GTK calls vs 0.23 s of Qt calls, but Qt then spends 2.8 s showing and laying them out after the checkpoint; the GTK equivalent (render probe) is 75 ms for the visible part.
5. Batching decision (backend-abi.md open question): 11000 ops cost 16-18 ms of host/ABI time in total, so batching to save ABI crossings is not justified by this measurement. Reducing op count could still help toolkits that relayout per op (Qt).

The instrumentation is off by default: `HostConfig::timing` (two `Instant` reads per host op and per entry into JS), `svelters_gtk::timing::set_enabled`, `counter_qt::hooks::set_timing`; `counter --timing` enables all three.

## Lifetime notes from the coordinator (ownership research) and how P5 handles them

- Qt: native close only hides the window; `quitOnLastWindowClosed` ends `run()` with all handles still live. The host is dropped right after `run()` returns and issues no further ops, so no stale-handle errors occur (`run_returns_after_native_close_qt`). No ABI change; the window subtree is not reported through `on_destroyed` on Qt (the widgets are not destroyed at that point).
- Qt `destroyed` fires before children die; `on_destroyed` in the host only removes the handle from the kind table (`Core::forget_handle`) and never enters JS, so it is safe there.
- GTK: see fix 3 above.
- GTK timing: assertions in the integration tests read the widget tree while the app is alive, never right after a click wait-out; no liveness assertions after close are made beyond `run()` returning and no host errors.
