# E3 handoff (reactive Rust exports)

Written at the coordinator's request (context limit). Branch `ci/e3`; code head **c6bd90cf** (this note is a
docs-only commit on top). The full record is `plans/milestones/E3-exports-reactive.md` (as built,
deviations, measurements, tests, merge notes, open items, conflict risks). Design: `docs/notes/egui-rust-exports.md`
§6.4/§6.6/§8, §11 Q7 (decided 2026-10-22: a dirty bit per field; recorded there).

## Current state

E3 is implemented and the acceptance is met. origin/master (E2 zero-copy buffers + FU2 GTK wake priority) **is
merged** into the branch (merge commit bd797eb9); conflicts resolved, see "Merge with E2 and FU2" in the
milestone file. What remains is only the completion record (final gate lines and CI run ids in the milestone
file) and the coordinator's merge into master.

## Done, with evidence

| Acceptance | Evidence |
| --- | --- |
| Stores join the R1 batch: 10k worker updates → one batch, measured vs per-event | `crates/svelters/tests/reactive.rs` `ten_thousand_worker_writes_are_one_batch` (1 batch, 1 field, checkpoint 0.14–0.30 ms release); GTK `examples/exports/tests/exports.rs` `store_burst_is_batched_event_burst_is_not`: 10k store writes → 1–3 effect runs, 1.7–5.9 ms vs 10k events → 10000 effect runs, 1.2–2.0 s (release, loaded machine; numbers in the milestone) |
| `bind:` on a writable store field with `on_write` (clamp delivered once, echo dropped) | GTK `telemetry_store_drives_a_bound_scale` (`<scale bind:value={telemetry.speed}>`, Rust write moves it, drag to 15 clamps to 10, no echo loop); log backend `writable_field_clamps_once_and_drops_the_echo`, `accepted_writes_echo_nothing_and_rejected_writes_throw`; compiler golden `crates/svelters-compiler/tests/r2/store-field-gtk.*` (lowers to `bind_prop`, no diagnostics under `--strict-native`). No compiler change was needed; E5 owns reading `exports.json` |
| State survives hot reload (StoreStatic, services, persisted objects; JS-created die unless persisted) | `persisted_objects_and_app_stores_survive_a_reload` (failed + good reload), `crates/svelters/tests/services.rs` |
| Services start at app start, stop at shutdown, not at reload | `services.rs` (eager start with first `Host`, lazy on import, same store after reload, `shutdown_services` joins) |
| Emitter events through the batch/checkpoint path | `events_reach_listeners_in_order_after_the_change_batch`, `an_event_flood_warns_once` |
| Sink/Stream | stream tests in `reactive.rs` (bounded/backpressure, early break, dropped-unread closed by GC, abort, error after items, returned streams, `batches()`); `packages/types/test/stream.test.mjs` |
| persist | reload test above; persist + E2 `Buffer` held across reload is diagnosed (warning, see below): `a_persisted_object_holding_js_memory_is_diagnosed_at_reload` |
| Typings + svelte-check | `typings_show_stores_values_events_and_streams`; example `pnpm test` 0 errors; a deliberately misused file gave 4 errors |
| Example | `examples/exports`: telemetry service → store, scale bound to `speed`, burst panel (next to E2's DSP panel) |
| Milestone record | `plans/milestones/E3-exports-reactive.md` |

## Local gates on c6bd90cf (re-run after the last code edit, `SVELTERS_SLOT_JOBS=4`)

- `cargo check --workspace --all-targets`: exit 0, no errors (pre-existing warnings only).
- `pnpm -r build`: exit 0.
- full-workspace nextest (headless, shared lock, `--test-threads 4`): **2464/2464 passed, 10 skipped**.
- `pnpm test` and `python3 tools/docs/build_ir.py` + `pnpm docs:test`: **still running when this note was
  written**; not yet observed on the merged tree. Before the merge, the example's `pnpm test` and
  `packages/types` tests passed. Re-run them (below) and record the result.

CI on c6bd90cf: all five workflows success (js 39673578928, linux-gtk 39673578839, linux-qt 39673578895,
macos-appkit 39673578900, windows-winui 39673578914). Earlier heads also green except the first WIP push
(stale committed typings, fixed in the next push).

## How to build and verify

```sh
export SVELTERS_SLOT_JOBS=4 PATH=/tmp/claude-1000/pnpmshim:$PATH   # shim: `exec corepack pnpm "$@"`
pnpm install                                   # fresh worktree
tools/dev/cargo-slot cargo build -p svelters-compiler
pnpm -r build                                  # fixtures must be built before nextest (else ~70 fixture failures)
tools/dev/cargo-slot cargo check --workspace --all-targets
flock /run/user/1000/svelters-nextest.lock setsid tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --test-threads 4
pnpm test
python3 tools/docs/build_ir.py && pnpm docs:test
# focused:
tools/dev/cargo-slot cargo nextest run -p svelters --test reactive --test services
tools/dev/headless tools/dev/cargo-slot cargo nextest run -p exports-example
# example typings after changing exports:
tools/dev/cargo-slot cargo run -p exports-example -- --dump examples/exports/app/.svelters/app
# measurements: SVELTERS_BURST=10000 cargo test --release -p exports-example --test exports -- --nocapture burst
```

## Known failures and caveats

- Pre-existing, not E3: `crates/svelters/tests/exports.rs` `identity_dispose_and_gc` fails under `cargo test`'s
  shared-process mode (a global drop counter raced by a concurrent test); passes under nextest and with
  `--test-threads 1`.
- A JS `Buffer<T>` kept in persisted state (or a store, service, static) pins the old runtime: E2's teardown
  waits 1 s and leaks it. E3 cannot copy for an opaque persisted object; it warns at reload (console) naming
  `Buffer::into_vec()`. A copy-at-reload path is E2's open item.
- Store fields with `#[js(bigint)]` are rejected; event payload / record objects are not wrapped into classes.

## Open design questions

- E5: compile-time diagnostics for read-only store fields and unknown members from `exports.json` (store props
  already carry `observability: "exact"`, `verified: "delivery"`).
- Per-item diffs for collection store fields (future work per the Q7 decision).
- Services start with the first `Host` (no `App::run` exists); shutdown is the app's explicit
  `shutdown_services(timeout)` call.
