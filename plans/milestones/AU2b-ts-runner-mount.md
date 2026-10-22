# AU2b: the TS test runner, `mount` and `app.reset`

Goal: the items AU2 deferred to AU2b (`AU2-automation-transport.md`, "Not in AU2"): svelters' own TypeScript
runner (E2E-INSPECTOR §6 Q2, decided by the user), `mount` (§2.4) and `app.reset` (§2.2) on AU1b's
`VirtualClock`, and `app.clock` in `@svelters/test`. Coordinator decision (lane AU2b). Built on AU1b
(`AU1b-seeds-clock-test-macro.md`, "What AU2b needs").

## What landed

| Area | API | Notes |
|---|---|---|
| Runner | `svelters-e2e [paths…] [--workers N] [--grep RE] [--seed N] [--iterations N] [--retries N] [--shard I/N] [--bin PATH] [--reporter …] [--list]` (`packages/test/bin/svelters-e2e.mjs`, `runner/cli.js`) | Finds `*.e2e.{js,mjs,ts,mts}` (skipping `node_modules`, `target`, `dist`, dot dirs). Files are handed out to `--workers` child processes (default half the cores, capped at the file count) on a queue. `.ts` runs on Node's type stripping (`--experimental-strip-types` on Node < 23.6). No dependencies. Exit 0 / 1 (a failure) / 2 (usage). |
| Test API | `test(title, [opts], fn)`, `test.skip`, `test.only` (per file), `test.use(opts)`, `test.describe(title, fn)` (`runner/api.js`, re-exported from `@svelters/test`) | Options: `app` (dir, `App.svelte` or `dist/app.js`, relative to the test file; none = an empty host for `mount`), `bin`, `clock`, `stepMs`, `args`, `seed`, `iterations`, `timeout` (wall ms per iteration, default 30 s), `retries`. Fixtures: `{ app, mount, seed }`. |
| Seeds | `seed..seed+iterations`; `SEED`/`ITERATIONS` (or `--seed`/`--iterations`) override them | As `#[svelters_automation::test]`. Each seed is one iteration on `app.reset({seed})`. A failure prints `FAILED with SVELTERS_SEED=s` and `replay: SEED=s ITERATIONS=1 svelters-e2e <file> --grep '^<title>$'`. Retries rerun the same seed; a pass on retry is reported `flaky` (nondeterminism). |
| Workers | one app process per worker (`runner/worker.js`) | The app is launched on the first test and kept: `app.reset {seed}` between tests (skipped when the app is untouched at that seed, and for tests that destructure only `mount`, read from the function source as Playwright does). A different `app`/`bin`/`clock` config relaunches it. A test that kills the app (`transport.closed`, exit) or times out drops the process; the next test gets a new one. The failure carries the app's last stderr lines. |
| Reporters | `list` (default), `dot`, `json[:file]`, `junit[:file]`, `github` (annotations; on by default under `GITHUB_ACTIONS`) | Comma-separated. Every failure lists seed, error, app stderr and the replay command. |
| `app.reset` | svelters-run: `app.reset {seed?, timeout?}` → `{seed, frames}`; TS `app.reset({seed})` | Rebuilds the start app in a fresh host on a **new** `VirtualClock` (it cannot be reset in place), through `VirtualClock::config`, then settles. Same for `--clock real` (wall clock, `Math.random` seeded). |
| `mount` | svelters-run: `app.mount {module, export?, props?, size?: [w, h], seed?, timeout?}` → `{root: "mount", seed, frames}`, `app.set_props {props}`; TS `app.mount(path \| URL, {export, props, size, seed})` → `Mounted` (a locator over the host, `setProps`); the runner's `mount` fixture (paths relative to the test file, the test's seed) | Host side: `TestApp::mount_source(component, export, props, size, step_dt, cfg)` and `TestApp::embed_host(size, step_dt, cfg)` in svelters-egui's kittest runner: the component is loaded from source as a one-component embed entry (`Entry::Embed`, export = file stem by default, toolkit from the nearest package.json), mounted with `Props::json` into one slot filling the window, and stepped until live (a throwing mount fails `app.mount` with the error). The host is built with `VirtualClock::config`, so the component shares the clock and the seed. `set_props` is `Embed::set_root_props` (state kept). Without `<app>`, svelters-run starts on an empty embed host waiting for `app.mount`. |
| Session swap | `app.reset`/`app.mount` open a new `Session` (every grant) whose epoch is past the old one's | Refs taken before the swap read as stale. `server.set_interrupt` is re-pointed at the new host's flag. `session.hello`'s `app` gains `mount: true`. |
| `clock.*` in TS | `app.clock.now()`, `app.clock.advance(ms)`, `app.clock.runFor(ms)`, `app.grants` | `advance` fails `grant.denied` client-side when hello's `grants.clock` is false (the app also refuses it). `launch` gains `clock` (`virtual`/`real`) and `stderr` (`pipe` keeps the last 40 lines, which also go into a "exited before listening" error). |
| Fixture | `fixtures/counter-egui/src/Panel.svelte` | A `title` prop and its own click count, for `mount`/`setProps`. |
| Tests | `packages/test/test/runner.test.js` | On a mock host (`test/mock-host.mjs`, NDJSON JSON-RPC in Node, so the js job runs them without Rust): seeds and overrides, fixture names, args/discovery/replay, two workers with one launch each and resets between tests (`e2e-mock/pass`, one file in TypeScript using `mount`), a failing seed's print and replay, a crash and a hang failing alone, retries. With `SVELTERS_RUN`: the runner on the real host, `test/e2e/counter.e2e.js` (counter, reset between tests, a 30 s timer under `clock.advance`, the seed replaying `Math.random` across `app.reset`) and `test/e2e/mount.e2e.js` (Panel mounted with props, clicked, `setProps` keeping its count; a mounted component on the virtual clock). |
| CI | linux-gtk: step "e2e runner (svelters-run, counter fixture)" | `SVELTERS_RUN=$GITHUB_WORKSPACE/target/debug/svelters-run pnpm --filter @svelters/test test`, after nextest. Cheap: "Build tests" already builds svelters-run (svelters-automation's integration tests need the binary), so it costs the test time only (~10 s locally). The js job runs the mock tests through `pnpm -r test` and skips the real-host ones. |

## Added to the lane (coordinator): AU1b's follow-ups

| Item | Fix | Proof |
|---|---|---|
| `wait.for visible` read a `<stack>`'s inactive child as visible | `NodeRef::is_visible` (svelters-egui scene) also checks that no ancestor-or-self is a `stack` child its stack does not show (`stack_pick`, the layout's own choice). Every user gets it: `wait.for`, actionability, snapshots, the query `visible` filter, providers. | `svelters-automation/tests/e2e.rs::gallery_containers_page_switches` checks "Stack page two" hidden before the switch and "Stack page one" hidden after. |
| `input.set_value` had no handle target on a two-handle `range-slider` | `input.set_value {handle}`: `Session::set_value_handle` picks the AccessKit slider inside the node's rect whose label ends with `handle` (`"high"`), or the n-th handle left to right (`"1"`, or a number); `node.not_found` lists the handles when none matches. Rust `Locator::set_value_handle`, TS `setValue(v, { handle })`. | `gallery_inputs_page_binds_every_value` restores gallery.rs's `bind:high` step (`range: 10 to 60`). |
| `import.meta.glob` on the live loader | `svelters-modules/src/source/glob.rs`: after compiling a module, `SourceProvider` replaces each call with an object keyed `"./pages/a.svelte"` (sorted): eager → appended `import * as` (or `{ name as }` with `import:`), lazy → `() => import(..)`. Patterns: relative literals or arrays, `*`, `?`, `**`, `{a,b}`, `!` exclusions. `query`/`as`/`base` fail with a clear error. The file list is read when the importing module compiles. | `glob::tests` (unit), and both gallery e2e tests now run from source (`app = "../../fixtures/gallery-egui"`, not `dist/app.js`), in process and with `SVELTERS_E2E=process`. |

## Decisions and deviations

- **Our own runner, no dependencies** (Q2). The runner is Node built-ins only (`child_process.fork`, IPC); no
  lockfile change.
- **`setProps` is a method (`app.set_props`)**, not a remount: §2.4's example expects the new title with the
  click count kept.
- **`mount` takes a path, not an imported component.** Node cannot import `.svelte`; `mount('../src/Panel.svelte')`
  (or a `URL`) is what the host compiles live. §2.4's `import Panel from …` form needs a loader hook; not done.
- **No `slot` parameter.** One mount per host (`app.mount` replaces the content); several slots per host (and
  `mount` next to a running app) are later, with cadatomic's needs.
- **`test.only` is per file**, and there are no `beforeEach`/`afterEach` hooks yet; the worker-scoped `app` and
  `reset` cover the fixture model §2.2 shows. `doc`/`sources` are cadatomic's (`@cadatomic/test`).
- **Fixture detection by source** (`({ app, mount })`), as Playwright does; a non-destructured parameter gets
  the app prepared.

## Not done (with where it goes)

- Traces on failure (§2.6) and `toHaveScreenshot`: AU5 / pixels over the wire.
- `expect.poll`, sharding across machines beyond `--shard`, a watch/UI mode.
- Windowed `svelters-run`, `events`/`log`/`perf` streams: as in AU1b.
- `import.meta.glob` and HMR: a page file added while the app runs appears when the importing module is next
  compiled (its own edit or a reload), not by itself (no directory watch).

## Handoff

- Files: `crates/svelters-automation/src/bin/svelters-run.rs`, `crates/svelters-automation/src/{session.rs, e2e.rs}`
  (`set_value_handle`), `crates/svelters-automation/tests/e2e.rs`, `crates/svelters-egui/src/runner/kittest.rs`
  (`mount_source`, `embed_host`), `crates/svelters-egui/src/scene/access.rs` (`is_visible`),
  `crates/svelters-modules/src/source/{glob.rs, mod.rs}`, `fixtures/counter-egui/src/Panel.svelte`, `packages/test/{index.js, index.d.ts,
  package.json, bin/, runner/, test/runner.test.js, test/mock-host.mjs, test/e2e/, test/e2e-mock/}`,
  `.github/workflows/linux-gtk.yml`, and this record. `STATUS.md` is left for the coordinator.
- Run: `SVELTERS_RUN=$PWD/target/debug/svelters-run node packages/test/bin/svelters-e2e.mjs packages/test/test/e2e`.
