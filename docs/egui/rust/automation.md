---
title: Automation and end-to-end tests
description: "Drive a svelters app from tests: the Session API, #[svelters_automation::test], svelters-run, @svelters/test and the svelters-e2e runner."
status: available
verified_at: d0542a3a
sources:
  - crates/svelters-automation/src/{lib,session,clock,e2e,server,client}.rs
  - crates/svelters-automation/src/bin/svelters-run.rs
  - crates/svelters-automation/tests/e2e.rs
  - packages/test/            # @svelters/test, svelters-e2e
  - plans/milestones/AU1-automation-core.md
  - plans/milestones/AU1b-seeds-clock-test-macro.md
  - plans/milestones/AU2-automation-transport.md
  - plans/milestones/AU2b-ts-runner-mount.md
  - plans/milestones/CV1-canvas-seams.md   # drag, pointer_move, wheel; panes
---

# Automation and end-to-end tests

`svelters-automation` drives a running app the way a user does: find nodes with locators, click, type,
wait until the app settles, compare a text snapshot of the scene. The same calls work **in process**
(kittest, fast, no window) and **out of process** (a socket to the app).

| You want | Use |
|---|---|
| Rust tests of a whole app, with seeds and virtual time | `#[svelters_automation::test]` |
| TypeScript tests, one app process per worker | `@svelters/test` and `svelters-e2e` |
| One component alone, with props | `mount` (TS fixture) |
| Your own Rust harness (`EmbedHarness`, cadatomic's `KittestDrive`) | `Session` over a `Driver` |
| Agents or tools talking to a running app | `Server` + the JSON-RPC methods |

Automation is **off by default** in every build. Until the app calls `svelters_automation::enable()`,
`Session::new`, `Server::start` and every call fail with `automation.disabled`. Test helpers enable it
for you.

## Rust: `#[svelters_automation::test]`

Feature `run` on `svelters-automation` (a dev-dependency). From `crates/svelters-automation/tests/e2e.rs`:

```rust
use svelters_automation::e2e::prelude::*;

#[svelters_automation::test(app = "../../fixtures/counter-egui/src/Clock.svelte", seed = 7)]
fn a_30_s_timer_fires_on_virtual_time(d: &mut E2e) -> TestResult {
    expect(d.get_by_text_exact("Fired: no")).to_be_visible()?;
    d.get_by_role("button", name("Start")).click()?;
    d.clock().run_for(29_000.0)?;
    expect(d.get_by_text_exact("Fired: no")).to_be_visible()?;
    expect(d.get_by_text("Fired: after 30")).timeout(5_000.0).to_be_visible()?;
    Ok(())
}
```

- Attributes: `app` (an app directory, an `App.svelte`, or a built `dist/app.js`; relative to the crate),
  `seed`, `iterations`, `clock = "virtual" | "real"`, `step_ms`.
- The body runs once per seed `seed..seed+iterations`. `SEED=` and `ITERATIONS=` override them. A failure
  prints `FAILED with SEED=s` and the replay command.
- **In process** by default (a kittest `TestApp`, every grant, a virtual clock). With
  `SVELTERS_E2E=process` the same body runs against `svelters-run` (or `SVELTERS_APP=<bin>`).
- `E2e`: `get_by_role(role, name("…"))`, `get_by_text`, `get_by_text_exact`, `get_by_label`,
  `locator(selector)`, `root(key)`, `press`, `type_text`, `wait_settled`, `snapshot`, `call`,
  `clock().run_for(ms)` / `now_ms()`, `seed()`.
- `expect(locator)`: `to_be_visible/hidden/attached/detached/enabled`, `to_have_text`,
  `to_contain_text`, `to_have_value`, `to_have_count`, `to_match_scene_snapshot`, `.timeout(ms)`.
  Snapshots live in `<crate>/tests/snapshots/<name>.yaml`; `SVELTERS_UPDATE_SNAPSHOTS=1` rewrites them.

## TypeScript: `@svelters/test` and `svelters-e2e`

```js
// counter.e2e.js (from packages/test/test/e2e/)
import { test, expect } from '@svelters/test';

test.use({ app: '../fixtures/counter-egui', stepMs: 16 });

test('the counter counts', async ({ app }) => {
  const inc = app.getByRole('button', { name: 'Increment' });
  await inc.click();
  await inc.click({ tier: 'semantic' });
  await expect(app.getByText('Count: 2', { exact: true })).toBeVisible();
});

test('a timer on virtual time', async ({ app }) => {
  await app.getByRole('button', { name: 'Start' }).click();
  await app.clock.advance(30_100);
  await expect(app.getByText('Fired: after 30')).toBeVisible();
});
```

Run: `SVELTERS_RUN=$PWD/target/debug/svelters-run npx svelters-e2e tests/` (`--workers N`,
`--grep RE`, `--seed N`, `--iterations N`, `--retries N`, `--shard I/N`, `--reporter list|dot|json|junit|github`,
`--list`). It finds `*.e2e.{js,mjs,ts,mts}`; `.ts` runs on Node's type stripping.

- `test(title, [opts], fn)`, `test.use`, `test.describe`, `test.skip`, `test.only` (per file). No
  `beforeEach`/`afterEach` yet.
- Options: `app`, `bin`, `clock`, `stepMs`, `args`, `seed`, `iterations`, `timeout` (wall ms per iteration,
  default 30 s), `retries`. A pass on retry is reported as **flaky**.
- **One app process per worker.** Between tests it calls `app.reset({ seed })`, which rebuilds the app
  in a fresh host on a new virtual clock. A test that crashes or hangs the app fails alone; the next test
  gets a new process.
- `expect` is evaluated **in the app** after each settle, with the timeout in virtual ms. There is no
  client polling.
- `app.health()` and `app.interrupt()` use a second connection, so they work while the UI thread is
  stuck (an infinite loop in a handler).

### `mount`: one component alone

```js
test('Panel counts and takes new props', async ({ mount }) => {
  const c = await mount('../src/Panel.svelte', { props: { title: 'One' }, size: [320, 200] });
  await c.getByRole('button', { name: 'Inc One' }).click();
  await expect(c.getByText('One clicks: 1', { exact: true })).toBeVisible();
  await c.setProps({ title: 'Uno' });                       // state kept
  await expect(c.getByText('Uno clicks: 1', { exact: true })).toBeVisible();
});
```

`mount` takes a path (the host compiles it live), not an imported component. One mount per host; it
shares the test's clock and seed.

## Locators

Selectors are the same in Rust, TS and JSON. Engines chain with `>>`:

| Engine | Matches |
|---|---|
| `css=` (default) | svelters-style's matcher on the scene; provider states as `:state(ns.name)` |
| `role=button[name="Save"]` | ARIA role and accessible name (`[name*=…]`, state names) |
| `text="Exact"` / `text=part` | text, exact, or a case-insensitive substring |
| `label=`, `anchor=`, `root=` | label text, `anchor` prop, root key |
| `key=k42` | the rows of the keyed `{#each}` item with key `k42` (`key=k42 >> anchor=row`; CV1) |
| `pos=x,y`, `nth=` | a point; the n-th match |

`locate` is strict: more than one match fails `node.ambiguous`. Not yet: regex names (`/…/`),
`component=`, `source=`, `a11y=` (they answer `capability.absent`).

## Determinism: seeds and the virtual clock

- **Virtual clock** (the default): timers, `performance.now()` and timer wakeups run on a
  `VirtualClock`. Each frame advances `step_ms`. Waits and timeouts count virtual ms, so a 30 s wait
  costs frames, not wall time. `#[js(worker)]` and `#[js(blocking)]` jobs run on the UI thread in seeded
  order.
- **Seed**: with a seed set, `Math.random` is a seeded generator in every engine the host makes (a full
  reload included). Without one, QuickJS's own function stays.
- `--clock real` keeps the wall clock; `--seed` then seeds only `Math.random`.
- From a session: `clock.now` → `{mode, ms, seed, session_ms}`, `clock.advance {ms}` (needs the `clock`
  grant).

## `svelters-run`: the reference host

```
svelters-run <app dir | App.svelte | dist/app.js> --automation tcp:0 --headless [--clock virtual|real] [--seed N] [--step-ms 16]
```

It compiles the app live, runs headless, steps frames only when a request asks, and prints
`SVELTERS_AUTOMATION=<endpoint>` once it listens. Without `<app>` it waits for `app.mount`. Extra
methods: `app.reset`, `app.mount`, `app.set_props`, `app.exit`. Windowed mode is not built.

## Your own harness: `Session`

```rust
// not run
let mut s = svelters_automation::Session::for_test(TestApp::from_source(dir)?);   // feature `kittest`
s.set_idle(move || barrier.is_idle());          // the host's own idle predicate holds waits
s.click(r#"role=button[name="Increment"]"#, Default::default())?;
s.wait_for("role=label", WaitState::Visible,
           Some(&Expect::Text(TextMatch::Exact("Count: 1".into()))), 5_000.0)?;
assert!(s.snapshot(None)?.contains(r#"- label "Count: 1""#));
```

- `Session::new(driver, Grants { read, act, tooling, clock })` (or `READ`, `ACT`, `ALL`); `for_test`
  grants everything. `Driver` is implemented for `harness::TestApp` and `harness::EmbedHarness`.
- Acts (`click`, `hover`, `focus`, `fill`, `set_value`, `press(chord)`, `type_text`) check actionability
  first (attached, visible, stable over two frames, enabled, hit at the centre) and reply after the
  settle. `ActOptions { tier, force, timeout_ms }`: `Tier::Synthetic` (default, egui input) or
  `Tier::Semantic` (AccessKit actions).
- Canvas acts (CV1, synthetic only, deterministic under the virtual clock): `drag(target, Drag, opts)`
  (a press, `steps` moves of one frame each, the release; `Drag::to(local).from(At::Local(p)).steps(n)
  .button(b).modifiers(m)` or `Drag::to_node(other)`), `pointer_move(target, At, modifiers, opts)` and
  `wheel(target, Wheel::lines(v) | Wheel::points(v), opts)` (egui's direction: positive `y` scrolls
  up; `.at(..)`, `.modifiers(..)`). Points are the target's local points (from the top left of its
  rect; outside it is allowed); `At::Center` is the default. See "Canvases" below.
- Settled means `Embed::settled()` is empty, no injected input is pending, no foreground busy source is
  busy, and the host's idle predicate holds. A timeout names the busy reasons. Events, busy sources,
  latched conditions, clocks and the failure report: [Waiting on anything](waiting.md) (AU7).
- `set_tooling_roots(..)` hides tooling roots from queries unless `include_tooling(true)` (needs the
  `tooling` grant).
- Refs (`Ref { id, epoch }`) go stale after `bump_epoch()` or a reset (`node.stale`).
- `Session::call(method, params)` is the JSON schema: `scene.query`, `scene.locate`, `scene.node`,
  `scene.snapshot`, `a11y.tree`, `input.*`, `wait.settled`, `wait.for`, `capture.screenshot`,
  `journal.subscribe/take`, `clock.*`.

## Canvases: drags, moves and wheels at local points

A canvas is either a Svelte node (a `canvas`, a box listening to `onpointerdown`/`onpointermove`)
or a pane the host paints itself (`Embed::create_pane`, CV1): a pane's key is its `anchor`, so
`anchor=<key>` addresses it, and events on its parts reach the host's event filters as
`Target::Virtual`.

```rust
// not run
s.drag("anchor=board", Drag::to(egui::pos2(150.0, 140.0)).from(At::Local(egui::pos2(50.0, 100.0)))
    .steps(4).modifiers(egui::Modifiers::SHIFT), ActOptions::default())?;
s.wheel("anchor=board", Wheel::lines(egui::vec2(0.0, 2.0)).modifiers(egui::Modifiers::CTRL), ActOptions::default())?;
```

| JSON method | Params (besides the target and `ActOptions`' `tier`, `force`, `timeout`) |
|---|---|
| `input.pointer_move` | `at: [x, y]` (default the centre), `modifiers: "Shift+Ctrl"` or `["Shift"]` |
| `input.wheel` | `dx`, `dy` (egui's direction), `unit: "line" \| "pixel" \| "page"` (default `line`), `at`, `modifiers` |
| `input.drag` | `from: [x, y]`, one of `to: [x, y]`, `to_selector`, `to_ref`; `steps` (default 10), `button: "primary" \| "secondary" \| "middle"`, `modifiers` |

TypeScript: `locator.pointerMove({ at, modifiers })`, `locator.wheel({ dx, dy, unit, at, modifiers })`,
`locator.drag({ from, to: [x, y] | otherLocator, steps, button, modifiers })`.

A drag sends the move to `from`, the press, each step's move and the release in their own frames, so
a `pointermove` listener sees every step (SV50 coalesces moves within one frame only). Modifiers are
held with egui's `ModifiersChanged` around the act and carried on the press, release and wheel
events.

## Out of process: `Server`

```rust
// not run
svelters_automation::enable();
let server = Server::start(&session, Endpoint::parse("unix:/tmp/app.sock")?)?;
// once per frame, on the UI thread:
server.pump(&mut session);
```

- NDJSON JSON-RPC 2.0. The socket is local only: a Unix socket with mode 0600, or TCP on 127.0.0.1.
  There is no token or auth.
- The UI thread never touches the socket; `pump` drains requests one at a time, in arrival order.
  `pump_with` takes your own dispatcher.
- `client::Client` / `client::App::spawn` are the Rust client. `Client::call` has `Session::call`'s
  signature, so a test body runs either way.

## Mistakes we have seen

- Forgetting `enable()` in a shipped app and getting `automation.disabled`. It is meant to be off until
  the user opts in.
- Polling with sleeps: use `expect(…)` or `wait_for`. They run in the app on virtual time. For jobs,
  workers and network, wait on an event after a cursor: see [Waiting on anything](waiting.md).
- `getByText('Count: 1')` matching `Count: 10`: bare `text=` is a substring. Pass `exact: true`.

## Not done

Traces on failure and `toHaveScreenshot`; `log`/`perf` streams (the `events` stream is AU7's journal); windowed `svelters-run`;
`Origin::Tool` on writes a session causes; several mounts per host.
