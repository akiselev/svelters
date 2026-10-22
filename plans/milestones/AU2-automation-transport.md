# AU2: automation out of process (socket transport, reference host, clients)

Spec: `plans/research/E2E-INSPECTOR.md` §1.9, §2.2, §2.3, §5 (AU2 row), §6 (Decided). Builds on AU1
(`plans/milestones/AU1-automation-core.md`). Lane branch `ci/au2`.

## Decisions this lane follows

- **Off by default, compiled in** (user, via the coordinator, 2026-10-22). No cargo feature removes the transport.
  `Server::start` fails `automation.disabled` until the app has called `svelters_automation::enable()`, and it
  takes an open `Session`, so nothing listens before both. The `run` feature only adds the reference host binary.
- **Security is cut to off by default plus a local-only socket** (user). A Unix socket with mode 0600 (a stale
  file is replaced; a live server's path is refused), or TCP bound to 127.0.0.1 with non-loopback peers dropped.
  There is no token, auth or per-client policy. `session.hello`'s `token` is accepted and ignored. Later work
  slots in at `serve()` (the per-connection thread, before the first request is queued) and in `Endpoint`.
- **Our own TS runner, not `@playwright/test`** (§6 Q2). `@svelters/test` ships the client half (launch, locators,
  `expect`, scene snapshots). The runner is not in this lane (see "Not in AU2").

## What landed

| Area | API | Notes |
|---|---|---|
| Framing | `svelters_automation::rpc` | NDJSON JSON-RPC 2.0, cadatomic A8a-1a's framing. Protocol error codes travel in `error.data.code`; numeric codes are `-32700`/`-32600`/`-32601` (`method.unknown`)/`-32602` (`params.invalid`)/`-32000` (the rest). Lines are capped at 32 MiB. A request without `id` is a notification. `rpc::outcome(reply)` gives back exactly what `Session::call` returned. |
| Server | `server::Server::start(&session, Endpoint)`, `pump(&mut session)`, `pump_wait(&mut session, dur)`, `pump_session(wait, &mut SessionCall, &mut session)`, `pump_with(&mut FnMut(method, params))`, `set_interrupt(Arc<AtomicBool>)`, `endpoint()` | Threads: a listener, then a reader and a writer per connection. **The UI thread never touches the socket.** `pump` drains a channel with `try_recv` and hands replies to the writer thread. `pump_wait` is for a headless runner with nothing else to do. Requests from all connections run one at a time, in arrival order. `pump_session` puts app methods in front of the session's (the reference host's `app.exit`, a decorated `session.hello`); `pump_with` takes any dispatcher (cadatomic's, with the session mounted in it). Dropping the server stops the listener and unlinks the socket. |
| IO-thread methods | `app.health`, `app.interrupt` | Answered by the reader thread, so they work while the UI thread is stuck. `app.health` returns `{uptime_ms, running: {method, for_ms} \| null, last_pump_ms_ago, busy, settled, clock_ms, served}`. `app.interrupt` sets the host's `interrupt_flag` (`Host::interrupt_flag()`, passed through `set_interrupt`) and returns `{interrupted, running}`. Without a flag it fails `capability.absent`. Both fail `automation.disabled` when automation is off. |
| Endpoint | `server::Endpoint::{Unix(path), Tcp(port)}`, `Endpoint::parse("unix:/p" \| "/p" \| "tcp:PORT")` | `tcp:0` picks a free port; `Server::endpoint()` reports the real one. Windows has only `tcp:` for now (std has no named pipes). |
| Rust client | `client::Client::{connect, connect_retry, call, set_timeout}`, `client::App::{spawn, call, connect, alive}` | `Client::call(method, &params)` has `Session::call`'s signature, so one test body runs in process or out of process (`tests/ipc.rs` does exactly this). New error codes: `transport.closed` (dead app), `transport.io`, `transport.address`, `rpc.parse`, `rpc.request`. `App::spawn(bin, args, endpoint, timeout)` waits for `SVELTERS_AUTOMATION=<endpoint>` on stdout, connects, and kills the process on drop. This is the `Driver::Ipc` of §2.3, under a name that does not clash with the `Driver` trait; cadatomic's `RpcDrive` can wrap it. |
| Reference host | `svelters-run <app dir \| App.svelte> --automation <endpoint> --headless [--clock virtual] [--seed N] [--step-ms 16]` (bin in svelters-automation, feature `run`) | The kittest runner inside the binary: the app is compiled live from source (`TestApp::from_source_stepped`) and has no window. Frames run only when a request asks for them, at `--step-ms` of virtual time each. `--automation` is the process's opt-in (it calls `enable()`). It prints `SVELTERS_AUTOMATION=<endpoint>` once it listens. `app.exit` replies and then exits 0. `session.hello` adds `app: {headless, clock, seed, frame_ms}`. Without `--headless` it refuses: a windowed app mounts `Server` in its own frame loop, calling `pump` once per frame. It lives in svelters-automation, not `svelters-cli`, because the CLI must not depend on a toolkit crate (its Cargo.toml rule). |
| TS client | `packages/test` (`@svelters/test`): `launch({bin, app, seed, stepMs, endpoint})`, `attach(endpoint)`, `Client`, `App` (`call`, `root(key)`, `locator`, `getByRole/Text/Label`, `keyboard.press/type`, `waitSettled`, `snapshot`, `health`, `interrupt`, `close`), `Locator` (`click/hover/focus/fill/setValue`, `count`, `all`, `info`, `textContent`, `snapshot`, `nth`), `expect(locator)` (`toBeVisible/Hidden/Attached/Detached/Enabled`, `toHaveText`, `toContainText`, `toHaveValue`, `toHaveCount`, `toMatchSceneSnapshot`) | Plain ESM with `index.d.ts`, and no dependencies. Every `expect` is one `wait.for`, evaluated in the app after each settle, with the timeout in virtual ms: no client polling. `toMatchSceneSnapshot` runs `wait.settled`, then compares the YAML with indentation ignored. `health`/`interrupt` open a second connection so they never queue behind a hung call. Calls may overlap; replies are matched by id. |

Fixture: `fixtures/counter-egui/src/Hang.svelte` (a "Spin" button whose handler never returns, plus a "Bump" counter).

## Acceptance (E2E-INSPECTOR §5, AU2 row)

| Criterion | Test | State |
|---|---|---|
| The same test body in both modes, the same seed, the same YAML snapshot | `ipc::same_body_same_snapshot_in_and_out_of_process`: three clicks (synthetic and semantic), a `wait.for` and `scene.snapshot`, through `Session::call` and through `svelters-run`; the snapshots are equal | passes. The body is the counter fixture written as JSON calls, not §2.3's `#[svelters::test]` macro (AU1b) |
| A test that crashes the app fails alone, and the next one passes | `ipc::a_crashed_app_fails_alone_and_the_next_one_passes`: the app is killed mid-session, the call fails `transport.closed`, a fresh app passes | passes |
| A JS infinite loop is broken by `app.interrupt` | `ipc::interrupt_breaks_a_runaway_loop_from_the_io_thread`: a semantic click on Spin hangs the UI thread; `app.health` (IO thread) shows `input.click` running; `app.interrupt` breaks it; the app then clicks Bump and its `wait.for` passes | passes |
| Rust and TS send the same methods | `packages/test/test/client.test.js`: a mock NDJSON app checks the wire params for locators, acts and `expect`, out-of-order replies, `transport.closed`, and snapshot mismatch; with `SVELTERS_RUN=target/debug/svelters-run`, the counter through the real host | the mock tests run in CI's js job (`pnpm -r test`). The real-host test skips there because js has no Rust build; it passed locally |

Also: `rpc::tests::{requests_replies_and_codes_round_trip, lines_are_bounded_and_split}`.

Local runs, all passing: `cargo nextest run -p svelters-automation -E 'binary(ipc) | test(rpc::)'` (5 tests, 2 s), and
`SVELTERS_RUN=… node --test test/*.test.js` in `packages/test` (4 tests).

## Not in AU2 (deferred, with the lane that takes it)

- **`--clock real` and windowed mode in `svelters-run`.** Headless only, on the virtual clock. A windowed app mounts
  `Server::pump` in its frame loop (cadatomic: its own app; its A8a server can instead mount `Session::call` through
  `pump_with`).
- **`--seed` applied to `Math.random`** and E1 ordering. The seed is accepted and reported in `session.hello`, but
  seeding needs the host clock and seed hooks in `core.rs` (HOST owner, through the coordinator). This is AU1b's
  item; the flag is already plumbed.
- **The TS runner** (§6 Q2: our own: workers, `app.reset()` between tests, retries, the `SVELTERS_SEED=` print on failure,
  traces) and **`mount`** (`app.mount {module, export, props}` in the host, needed for TK1-style component tests). A
  follow-up lane (AU2b), or with AU5's traces.
- **`toHaveScreenshot`**. `capture.screenshot` returns only the size over JSON; pixels on the wire (base64 PNG) and
  kittest's wgpu renderer in CI come together.
- **`#[svelters::test]` with `SVELTERS_E2E=process`**: this needs the macro first (AU1b). `client::App` is the
  out-of-process half it will build.
- **`events`/`log`/`perf` streams and `a11y=`**. Neither is in the AU2 row. The streams need host hooks (outbox,
  console, TR3 profile). The server has no notification path yet: replies only. Notifications go through the same
  writer channel when the streams land. `a11y=` still answers `capability.absent`.
- **The last cached snapshot on the IO thread** and `app.stack` (§1.9): later, with ZED-LESSONS #3's watchdog.
- **Token, auth, policy** (cut by the user; see above). **Named pipes on Windows**: `tcp:` loopback for now.
- **Calls are serial across connections**: one session, one UI thread. Per-connection sessions (grants per client)
  come with the policy work.

## Handoff

- Files: `crates/svelters-automation/{Cargo.toml, src/lib.rs, src/rpc.rs, src/server.rs, src/client.rs,
  src/bin/svelters-run.rs, tests/ipc.rs}`, `fixtures/counter-egui/src/Hang.svelte`, `packages/test/**`,
  `pnpm-lock.yaml` (the new importer, 2 lines), and this record.
- CI: svelters-automation is already in linux-gtk's nextest `-p` list. The dev-dependency turns on `run`, so tests
  get `CARGO_BIN_EXE_svelters-run`. `@svelters/test` runs in js's `pnpm -r test`.
- `STATUS.md` is left for the coordinator.
