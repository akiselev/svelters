# E2E-INSPECTOR: one automation protocol for end-to-end tests, the inspector and agents

Status: research and design, 2026-10-22. Docs only; nothing was built or run for it. Lane: research/e2e-inspector.

The user's request: "we need to make an end to end testing framework for easily writing tests. Probably goes hand in
hand with the inspector since a lot of functional requirements will overlap".

The user's correction, relayed by the coordinator: "I think you've got that backwards. E2e tests should be out of
process, inspector should be in process i think. Lets research tradeoffs for both decisions. Also agents will run
inside cadatomic too, including with the self modifying ui."

**Inputs (svelters):**
- TK1 (`plans/milestones/TK1-test-cases.md`, `crates/svelters-testkit`, `tests/cases`);
- TRACKS.md §2.1 TESTKIT (TK2, TK3);
- SG3 (`plans/milestones/SG3-queries-hit.md`), SG4, and ARCH-SCENE.md §6.3, §6.5, §7.4, §7.5, §9 and §11 (SG7);
- `svelters_egui::harness::EmbedHarness`, `TestApp` and `Embed::settled`/`BusyProbe` (`embed/settle.rs`);
- WG3 (`crates/svelters-conformance`: pins, XTEST) and WG3w/WG3m, in flight;
- TK1 snapshots, and VIS1 (DESIGN.md §6, in flight);
- `tools/oplog-compare.mjs`;
- CSS.md §2.2, §2.9 and D8;
- ARCH-EMBED.md (roots, slots, faults, `interrupt_flag`) and ARCH-SECURITY.md (T4, R-G);
- H1 and H3 (rollback, the `$state` registry), CX1 `check()`, QT-SURVEY QA11, ZED-LESSONS §6 and §9 #1;
- the earlier native-inspector note, `docs/notes/inspector-demo.md` (2026-10-19, GTK/Qt focus).

**Inputs (cadatomic, read-only):**
- `docs/plan/00-decisions.md` §4.12 row ("Agents start, drive, read and screenshot the GUI through an RPC server and an
  MCP sidecar from the first commit");
- `docs/best-practices/agent-driven-testing.md` (the `Drive` trait, layers L0–L3, `wait_idle`, the egui_inspection
  and egui_mcp tiers);
- `docs/plan/02-phase-a.md` A8a-1a, A8a-1b and A8a-2;
- `crates/cadatomic-cli/src/{drive,scenario,pixel,wait,protocol}.rs` and `mcp/`;
- `crates/cadatomic-app/src/pixel.rs` (the pixel tier, refused in release builds);
- `docs/research/2026-10-16-command-architecture.md` §1;
- `docs/research/2026-10-22-js-api-publishing-and-live-ide.md` §6.4, §6.6 and §6.8 (the live IDE, last good, staged
  apply, safe mode, agents in the same loop).

## 0. Summary

1. **One protocol and one server in the app; clients run wherever they suit.** Every precedent that has lasted puts
   the provider inside the app and lets the client live anywhere: UIA, AT-SPI and AX; Squish's hooked AUT;
   WebDriver; CDP; Flutter's VM service; React DevTools' backend. svelters owns the scene-level and Svelte-level
   methods (query, read, act, wait, observe, capture, clock, reload) in a new crate, `svelters-automation`.
   cadatomic mounts these methods into its existing A8a socket server and MCP sidecar, and adds its domain: commands,
   documents, selection, camera, `ide.sources.*`. (§1)
2. **E2E tests run out of process, as the user leans.** A test runner drives the shipped binary over the socket. It
   gets crash isolation, real windows and real input, and the same build that users get. Determinism and virtual
   time still hold across the process boundary, because every request is applied at a frame boundary and answered
   after the app settles, and the clock lives in the app. The in-process kittest layer stays as unit and component
   tests. One Rust `Driver` trait runs the same test body in either mode. (§2.1)
3. **Tests read like Playwright:**
   - locators by role, name, text, anchor (the test id), component and source;
   - auto-waiting on settled frames;
   - web-first `expect`, and text snapshots of the scene (Playwright's aria snapshots);
   - a trace on failure;
   - a recorder in the inspector.

   In TypeScript the runner is `@svelters/test`, built on the `@playwright/test` runner. In Rust it is `Driver`.
   `.svelte` cases (TK1) gain locator steps. (§2)
4. **The inspector runs in process, as the user leans.** It lives in a separate tooling `Embed`: its own QuickJS
   runtime and a built-in bundle that the live IDE cannot edit, shown in its own egui viewport window. It is written
   only against the protocol, so the same bundle can later run detached in a second process, for hung, headless,
   remote and CI apps. (§3)
5. **Agents inside cadatomic are protocol clients over the in-process transport.** They use the same methods and
   tools as the external MCP sidecar. For the self-modifying UI:
   - every source write replies only after the reload has settled;
   - locators survive reloads through compiler identity, anchors and role or name;
   - element refs carry a reload epoch;
   - a health check after each reload reverts to last good on failure (H1 rollback, root faults, cadatomic's staged
     apply);
   - the inspector and agent roots are protected. (§1.11)
6. **Real OS input is an execution mode, not a different test.** WG3's injectors (XTEST now, SendInput/UIA and
   CGEvent/AX from WG3w and WG3m) sit behind one `OsInput` trait, so the same test runs on semantic, synthetic egui
   or OS input. (§4)
7. **Six lanes, AU1–AU6, 32–42 lane-days.** AU1 replaces TK2 and absorbs SG7. cadatomic needs AU1 first (in-app
   agents and locators in its kittest layer), then AU2 (the mount into its server) and AU3 (reload-aware automation
   for the self-modifying UI). (§5)

## 1. One automation protocol

### 1.1 Shape and precedents

```text
           ┌──────────────────────── app process (cadatomic, or any svelters app) ───────────────────────┐
           │  UI thread: Embed(s) ─ scene ─ host/QuickJS ─ style engine ─ loader/HMR ─ egui/AccessKit     │
           │        ▲  frame-boundary queue (requests applied in logic, replies after settle)             │
           │  svelters-automation: Dispatcher { scene.*, svelte.*, style.*, a11y.*, input.*, wait.*,      │
           │                                    clock.*, journal.*, capture.*, reload.*, app.* }          │
           │        ▲ cadatomic Dispatcher { command.run, commands.list, ide.sources.*, ui.* (egui_mcp) }  │
           │   ┌────┴─────────────┬───────────────────────┬──────────────────────────┐                     │
           │   in-process Rust     in-process JS module     in-process async channel   IO thread: socket  │
           │   (kittest tests)     svelters:automation      (in-app agents)            server + health    │
           │                       (inspector panels)                                      │             │
           └───────────────────────────────────────────────────────────────────────────────┼─────────────┘
                     NDJSON JSON-RPC 2.0 over a Unix socket / named pipe, token, mode 0600 │
                     ┌──────────────────────────┬────────────────────────────────┬─────────┘
                e2e runner (TS / Rust)    cadatomic mcp sidecar (external agents)    detached inspector (later)
```

| Precedent | Provider (in the app) | Client (where it runs) | What we take |
|---|---|---|---|
| UIA / AT-SPI / AX | the app's accessibility provider | out of process | roles, names, patterns (Invoke, Value, ExpandCollapse) as semantic actions |
| Squish (Qt) | a hook injected into the AUT | `squishrunner`, out of process | object maps (multi-property "real names"), a recorder, and a runner kept apart from the app |
| WebDriver / Appium / WebdriverIO | a per-platform driver (WinAppDriver/UIA, XCUITest/mac2) | the test, over HTTP | one wire protocol over many platform drivers (§4) |
| Chrome DevTools Protocol | the renderer (DOM, overlay, input) | DevTools, Playwright, Puppeteer | domains with enable and notifications; the pick overlay is drawn by the page; `Input.dispatch*` |
| Flutter VM service + DevTools | `WidgetInspectorService` and the select-mode overlay in the app | DevTools, a separate web app | the widget/render tree split; select mode in the app; integration_test with WidgetTester in the app |
| React DevTools | a backend inside the page or app | the extension, or `react-devtools` standalone over a websocket (React Native) | one backend, several frontends (§3.1) |
| egui_inspection + egui_mcp (egui 0.35+) | `InspectionPlugin` (`GetTree`, `ApplyEvents`, `Settle`, `GetScreenshot`) | the MCP server | already cadatomic's pixel tier (A8a-2); kept for Rust-drawn egui |
| Cypress | test code runs inside the browser | in process | time-travel DOM snapshots per command (we take: a snapshot per step in traces). Its limits (one tab, no native dialogs) are the in-process cost |
| Zed `gpui::test` | `TestDispatcher` (seeded), `TestClock` | in process | seeds, `run_until_parked`, simulated clock (§1.6) |

### 1.2 Sessions, principals and origins

- **`session.hello {token?, client, principal?, grants?}`** returns `{protocol: 1, app, roots, viewports,
  pixels_per_point, capabilities}`. In-process clients skip the token.
- **Grants** are per session: `read`, `act`, `input.os`, `clock`, `style.override`, `svelte.state`, `reload`,
  `tooling` (see the inspector and agent roots), `eval` (later). Reads go through ARCH-SECURITY R-G: a root is
  returned only if the reader's principal may read its labels. In-app agents get their grants from cadatomic (U3).
- **Origin.** Every scene write a session causes is journaled with `Origin::Tool(SessionId)` (ARCH-SCENE §6.5 has
  `Origin`). The inspector then shows "changed by agent X", and a session can ignore its own writes
  (`ChangeFilter::origins`).
- **Self-exclusion.** Tooling roots (inspector, agent panel) are tagged `tooling`. Queries, snapshots, perf counters
  and the journal skip them unless asked (`include_tooling`), as inspector-demo.md §3 asks.

### 1.3 Query: selectors

One selector string for every client, parsed once into a `Query` over the scene (SG3 planner) plus the svelters-style
matcher (CS0, CSS.md §2.9). Engines are prefixed, Playwright-style, and chained with `>>`:

| Engine | Example | Resolves through | Priority (recorder) |
|---|---|---|---|
| `role=` | `role=button[name="Apply"]`, `role=treeitem[name=/Extrude \d+/][selected]` | the kind→role table (TK2's, generated from metadata) and the accessible name from the same function the walk uses for AccessKit (`a11y.rs`) | 1 (Testing Library: `getByRole` first) |
| `label=` | `label="Depth"` | `labelled-by` and the accessible name of inputs | 2 |
| `text=` | `text="Count: 1"`, `text=/count/i` | FX1 `text_prop` per kind; `:has-text` matches descendants | 3 |
| `anchor=` | `anchor=extents[0]`, `root=panel.task >> anchor=slot/extents[0]` | per-root anchor maps, O(1) (`by_anchor`, `by_path`); anchors are the test ids (ARCH-SCENE §4.2) | 4 (`getByTestId`) |
| `component=` | `component=TaskPanel >> role=button` | compiler identity (SV16/H4) through the dev component tree (§1.4) | 5 |
| `source=` | `source=src/Panel.svelte:12` | identity → template position; display only, because lines move | 6 |
| `css=` (default) | `cad-viewport`, `property-grid > .row:has(:focus)`, `[value="3"]` | the svelters-style matcher over the scene's `Element` impl, with `:role() :name() :text() :visible :component()` as test-only pseudo-classes | 7 |
| `root=` / `window=` | `root=panel.task`, `window="Settings"` | `root_by_key`, the viewport table | scope |
| `a11y=` | `a11y=tab[name="Properties"]` | the AccessKit tree of the last frame (the kittest query layer): Rust-drawn egui outside the scene, such as berth tab strips and plain egui panels | fallback |
| `pos=` | `pos=412,96@viewport:2` | `hit_test` (SG3); last resort, as cadatomic's selector ladder | last |

- **Locators are lazy**, as in Playwright: they re-resolve before every action and on every retry, so a reload, a
  keyed move or a re-mount between steps is harmless.
- **Strictness:** an action on a locator matching more than one node fails ("2 nodes match; add `>> nth=` or
  narrow the scope"). Reads may return many.
- **Element refs** (`{id: NodeId, epoch}`) are for the inspector and agents that already hold a node. A ref from an
  older reload epoch, or to a freed node, fails with `node.stale {locator}` (cadatomic pitfall 15: never "click
  the nearest thing").
- **Virtual rows** (SG5) resolve as `Target::Virtual(VNodeId)` through `VirtualChildren::find_anchor`, including
  rows that are off screen, and `scroll_into_view` realises them.
- **One tree.** `role=` and `name` come from the scene with the same function that fills AccessKit, so tests, the
  inspector and screen readers agree by construction. TK3 adds the check: for every scene node with an `author_id`,
  the scene's role and name equal the AccessKit node's. The AccessKit tree is queried directly only for what the
  scene does not hold (`a11y=`).

### 1.4 Read

| Method | Returns | Source |
|---|---|---|
| `scene.query {selector, scope?, limit?}` | `[NodeInfo {ref, tag, kind, anchor, role, name, root, viewport, rect, state[]}]` | SG3 `query` + matcher |
| `scene.node {ref, props?: [...]}` | props by name (typed `Value`), state bits by name, ext debug strings (ARCH-SCENE §4.6 `debug`) | `NodeRef`, `get_prop` |
| `scene.layout {ref}` | border, padding and content rects, the clip, the viewport, `pixels_per_point`, the layout boundary flag | walk/place, `screen_rect` |
| `style.computed {ref}` | computed values, matched rules with their source spans and layers, inherited-from | the CS engine `computed(h)` (CSS.md §2.9: "why is this red") |
| `a11y.node {ref}` / `a11y.tree {root}` | role, name, value, states, actions, set size and position | the scene projection; the AccessKit `TreeUpdate` for `a11y=` |
| `scene.snapshot {scope, format: yaml\|json, depth?}` | SG7's `SceneSnapshot`; YAML in the aria-snapshot shape (`- role "name" [state]`), used for text snapshots | SG7 (folded into AU1) |
| `svelte.tree {root}` | component instances: name, file, identity, props, owned top nodes, children | dev hook on `$n.hmr` instances (H1) |
| `svelte.instance {id}` | `$state` values by declaration (read thunks of H3's registry), `$derived` values, context keys | H3 `hmr_keep` registry, dev builds only |
| `svelte.source {ref}` | `file:line:col` of the element's template position | identity + source map (CX1 spans) |
| `app.roots`, `app.windows`, `app.health` | roots (key, root kind, visible, viewport, faulted, tooling), viewports, busy bits, last frame time | `Embed`, `BusyProbe`; `app.health` is answered from the IO thread (§1.9) |

Dev-only reads (`svelte.*`) require the compiler's dev hooks. Release builds answer `capability.absent`. The
cadatomic build that ships the live IDE compiles UI with `dev` hooks on for editable layers (§6 Q1).

### 1.5 Act: three tiers

| Tier | How | Speed and determinism | Tests what | Default for |
|---|---|---|---|---|
| **semantic** | AccessKit-style actions on the node: `focus`, `click`, `set_value`, `increment`, `expand`, `scroll_into_view`; or kind extensions `node.ext {ref, method, args}` (quire `set_text`, berth `dock`, cad viewport `pick_uid`) | fastest; no geometry; deterministic | the handler and the state, not hit-testing | agents (cadatomic's "semantic first" rule), setup steps |
| **synthetic** | egui `RawInput` events at the node's actionable point (pointer, keys, IME `Preedit`, wheel), with explicit timestamps in one frame so double clicks work (kittest #8697) | fast; deterministic under the virtual clock | egui's real hit path, focus, the event outbox | tests, the recorder |
| **os** | real OS events: XTEST, SendInput, CGEvent (§4) | real time; needs a private display or desktop | the toolkit's input, IME, DnD across windows, native dialogs | conformance, nightly runs |

**Actionability** before a synthetic or OS action, retried each settled frame until the timeout (Playwright):
- attached;
- visible (`is_visible`, which includes the root);
- stable (the same `screen_rect` over two frames);
- enabled (no `DISABLED` state);
- receives events: `hit_test(viewport, point)` returns the node or a descendant. SG3 already has this, including
  `HIT_TRANSPARENT` and overlay roots.

`force: true` skips the checks. Every act replies after the consuming frame and the following settle, with
`{frame, epoch, changed: ChangeSet.by_node summary}`, so a client rarely needs a second read (rerun's rule: replies
come after the effect).

### 1.6 Wait, the clock and seeds

- **Settled**, once for every client:
  - `Embed::settled()` is empty for every non-tooling embed (the Busy bits in `embed/settle.rs`, with `RELOAD` made
    live in AU3);
  - the app's registered busy sources are quiet (cadatomic's `IdleBarrier` already takes `BusyProbe`);
  - no injected input is pending;
  - two frames with unchanged rects for the target.

  `wait.settled {timeout}` fails with the chain of Busy reasons, as `EmbedHarness::run_until_settled` does.
- **Server-side waits.** `wait.for {selector, state: attached|visible|hidden|detached|enabled|stable, expect?:
  {text|value|count|prop|snapshot}, timeout}` is evaluated in the app after each settle, so `expect(locator)`
  needs no client polling and no sleeps. The TS `expect` sends one `wait.for` per assertion; Rust in process calls
  it directly.
- **Clock** (`clock.install {mode: real|virtual, start?}`, `clock.run_for {ms}`, `clock.fast_forward {ms}`,
  `clock.pause_at {t}`, `clock.resume`; Playwright's Clock API names).
  - Virtual mode extends the host's scripted virtual time (`Core::virtual_ms`, `advance_clock_to`) to embed mode.
    JS timers, rAF and `performance.now`, egui `RawInput.time`, TR2's animator and `TIMER_SOON` all read it.
  - Frames advance it by `frame_dt` (default 16 ms) only when the app asks for a frame.
  - Timeouts count virtual ms, so a 30 s wait on a stuck animation costs no wall time and fails the same way on
    every machine.
- **Seeds.** `--seed N` (or `clock.install {seed}`) seeds `Math.random` in test mode, the order of E1 completions and
  worker results (through ZED-LESSONS #1's `TestScheduler` when it lands), and cadatomic's model-id seed. The
  runner prints `SVELTERS_SEED=N` on failure. `iterations` reruns with seeds `N..N+k`.

### 1.7 Observe

| Stream (`*.subscribe` → notifications) | Content | Source |
|---|---|---|
| `journal` | `Change` records, coalesced per node, with `Origin`, filtered by `ChangeFilter` (scope, mask, kinds, props, origins); overflow sets `resync` | SG2 observers (pull, drained at frame end) |
| `events` | native events delivered to JS, handler errors, sync results | the event outbox, the host |
| `log` | console (DX1, through tracing), JS errors with source-mapped stacks, CX1 diagnostics, root faults, per-root `ErrorState` | host, Embed `FaultPolicy`, CX1 |
| `reload` | `ReloadReport {ok, hot, fallback, diagnostics, epoch}`, roots touched, `$state` kept or reset per declaration | H1, H3 |
| `oplog` | op-log lines (the `svelters-backend-log` format, `state #h {...}` lines, WG3) between `oplog.start` and `oplog.stop`, comparable with `tools/oplog-compare.mjs --states` | host ops |
| `perf` | per frame: frame time, scene time (layout, walk, passes), JS entries and time per root, ops per root, `SceneStats` deltas | TR3 profile, `SceneStats`, host timing |

### 1.8 Capture

- **`capture.screenshot {target: window|viewport|root|node, pixels_per_point?, max_edge?, mask?: [selector]}`.** A
  node or root is cropped from its `screen_rect`. Masks paint dynamic regions flat. `max_edge` defaults to 1568,
  cadatomic's model cap.
  - Headless mode renders through kittest's wgpu renderer, on lavapipe in CI (TK1).
  - Windowed mode uses `Context::request_screenshot` plus a repaint (cadatomic pitfall 4).
  - cadatomic's `screenshot.viewport` stays its own (A5a-3 readback).
- **`capture.metrics {scope}`** gives VIS1's `metrics.json` (computed style, rects, tokens), so VIS1's numeric diff and
  a test assertion read the same data.
- **`scene.snapshot` (YAML)** is the default regression artefact. Pixels are secondary (cadatomic decision 12;
  egui_kittest's own advice).

### 1.9 Transports

| Transport | Clients | Threading | Notes |
|---|---|---|---|
| in-process Rust (`Session::call_typed`) | kittest tests, cadatomic's Rust code | direct, on the UI thread between frames | no serialization; the `Driver::InProcess` backend steps the harness |
| in-process JS (`svelters:automation`, an LD1 native module provider) | inspector panels | E3/E1 calls into the dispatcher; values as `Json<T>` (RA2) | the inspector sees exactly what a remote client sees |
| in-process async channel | in-app agents (cadatomic), on worker threads; the model call is remote | requests queued to the frame-boundary queue; the reply after settle wakes the caller | same JSON schema as IPC, so agent tool code is shared with the sidecar |
| IPC: NDJSON JSON-RPC 2.0 over a Unix socket or named pipe | e2e runner, `cadatomic mcp`, detached inspector | an IO thread (std threads, no tokio, as in svelters) owns the socket; UI work goes through the queue | framing, token file and mode 0600 as cadatomic A8a-1a; cadatomic mounts the svelters `Dispatcher` into its server; svelters ships a minimal reference server for its own tests and plain apps (`svelters run --automation`) |

The IO thread answers a few methods without the UI thread, so a hung app can still be diagnosed:
- `app.health`: `BusyProbe` bits, the last frame's time, which root's JS turn is running and for how long;
- the last cached snapshot;
- `app.interrupt {root?}`, which sets `Host::interrupt_flag` (any thread) to break a runaway JS loop;
- `app.stack` (later, ZED-LESSONS #3's hang watchdog).

### 1.10 Who owns what (svelters and cadatomic)

| Piece | Owner | Note |
|---|---|---|
| Method schema `scene.* svelte.* style.* a11y.* input.* wait.* clock.* journal.* capture.* reload.* app.*`, selectors, settle, clock, origins, tooling roots | **svelters** (`svelters-automation`) | the scene and the Svelte runtime are svelters' |
| Kind automation extensions (`node.ext`) | the kind's owner: svelters kinds, berth, quire, cadatomic (`cad.viewport`) | `Widget::automation()` hook, registered with the kind (RA1) |
| Socket server, token, MCP sidecar, tool generation and naming (`.`→`_`), `rpc-methods.txt` | **cadatomic** (exists: A8a-1a, A8a-2) | adds svelters' methods to its list; svelters' reference server uses the same framing |
| `command.run`, `commands.list`, documents, selection, camera, `query.screen_point`, `ide.sources.*`, proposal branches, last good, staged apply, safe mode | **cadatomic** | the domain; the self-modifying loop's policy (§1.11) |
| egui_mcp `ui.tree`/`ui.click` (pixel tier) | **cadatomic** | kept for Rust-drawn egui; svelters' `a11y=` engine covers the same nodes for selectors; no name clash (svelters uses `scene.*`/`input.*`) |
| Deterministic mode | split | svelters: clock, seed, settle, headless runner; cadatomic: model seed, job barrier, fixtures |
| Scenario NDJSON (A8a-1b) | **cadatomic** | its `ui` steps take svelters selector strings; svelters' recorder can emit it (§2.6) |
| `@svelters/test` (TS), `svelters_automation::Driver` (Rust) | **svelters** | `@cadatomic/test` and cadatomic's `Drive` extend them with commands and document fixtures |

cadatomic's `Drive` trait (`commands_list`, `command_run`, `wait_idle`) and svelters' `Driver` meet as follows:
cadatomic's `KittestDrive` wraps svelters' in-process `Driver`, and `RpcDrive` sends svelters methods unchanged.

### 1.11 Agents inside cadatomic and the self-modifying UI

The live IDE (cadatomic `js-api…live-ide.md` §6) edits the app's own Svelte sources; the Rust loader (Q1, H1, H3)
compiles and swaps them with `$state` kept. In-app agents drive the same loop. An agent changes the UI it is
observing. The rules:

| Concern | Rule | Mechanism |
|---|---|---|
| Write, then observe | a source write replies after compile, swap, mount and settle, with the `ReloadReport` and diagnostics; a read after the reply sees the new UI | `reload.apply {paths}` (svelters) behind `ide.sources.write` (cadatomic); `Busy::RELOAD` live until roots re-settle |
| Refs across a reload | every swap bumps `epoch`; refs from an older epoch fail with `node.stale {locator}`; locators re-resolve | §1.3 |
| Stable selectors | prefer role and name, then anchor, then `component=` (identity: component id + template AST position + each-key path + instance path, SV16/H4); `source=` lines are display only; the recorder ranks by stability | compiler identity (CADATOMIC.md "ID" lane); anchors now |
| Settle after reload | `wait.settled` includes compile and swap (`RELOAD`), re-mounts (`MOUNTING`), H3 restores and the fallback full reload | `embed/settle.rs` |
| Breaking the UI the agent needs (1) | a compile error swaps nothing (CX1 diagnostics in the reply) | H1 |
| Breaking the UI the agent needs (2) | a throw on swap rolls back to the old version | H1 rollback |
| Breaking the UI the agent needs (3) | a throw later faults that root only; `remount` with last props | ARCH-EMBED `FaultPolicy` |
| Breaking the UI the agent needs (4) | a **health check** after each applied write: no new faults or `ErrorState`, no new JS errors, frame budget held, and the session's declared **probes** (locators that must still resolve and be actionable) pass; on failure, cadatomic reverts to last good and the reply says so | `reload.apply {probes, on_fail: revert}`; the revert is cadatomic's `ide.revert` |
| Breaking the UI the agent needs (5) | **protected roots:** the inspector and the agent panel run in the tooling `Embed` from a built-in bundle that is not in the editable overlay set; cadatomic's chrome-critical roots take staged apply (preview root first); the IDE's recovery surface is Rust (cadatomic §6.6) | `reload.apply` refuses protected modules unless `staged: true` |
| Runaway edit loops | the HMR swap budget (H1, 200 swaps) falls back to a full reload; per-session write rate limit; the user's kill switch in the Rust agent panel revokes the session and reverts its open proposals | H1, cadatomic U3 |
| Attribution | every op caused by the agent's acts and edits is journaled `Origin::Tool(session)`; the inspector filters by it | §1.2 |

**One protocol, four kinds of client:**

| Client | Runs | Transport | Grants | Mostly calls |
|---|---|---|---|---|
| kittest unit and component tests | the test binary | in-process Rust | all | everything |
| e2e runner | a separate process | IPC | test (adds `input.os`, `clock`) | everything |
| inspector | tooling `Embed` in the app (detached later) | in-process JS (IPC detached) | devtools (`tooling`, `style.override`, `svelte.state`) | read, observe, capture, overlay |
| in-app agent | a worker thread in cadatomic | in-process async | the agent's (U3, R-G filtered) | semantic act, read, wait, capture, `command.run`, `ide.sources.*` |
| external agent | `cadatomic mcp` sidecar | IPC | the agent's | the same tool list, generated from one table |

## 2. The test-writing API

### 2.1 Out of process or in process: the trade-offs

| Axis | Out of process (runner drives the app binary) | In process (kittest, test binary owns the app) |
|---|---|---|
| Fidelity | the shipped binary, its `main`, eframe/winit, real viewports, real AccessKit adapter, real GPU path | a harness around `Embed`/`TestApp`; kittest calls no `raw_input_hook` and applies only `InnerSize`/`Screenshot` viewport commands (cadatomic pitfalls 7, 19) |
| Speed | process start and bundle load per **worker**, amortised by `app.reset()` between tests (Playwright: browser per worker, context per test); per call a socket round trip, small next to the settle work (estimate; measure in AU2) | fastest: no spawn, no serialization; TK1 cases run in ms each |
| Determinism and virtual time | **kept**: requests apply at frame boundaries and reply after settle (lockstep), and the clock and seed live in the app; with `--headless` (a kittest-style runner inside the binary, no winit) it matches in-process; windowed adds window-manager timing | full: one thread, the harness owns time |
| Crash and hang | runner survives; a crash fails one test, collects DX1 crash context, and restarts the app; a hang is diagnosed through `app.health` and broken with `app.interrupt` from the IO thread; deadline kill | a panic or abort kills the test process (nextest isolates per test); a hang waits for nextest's slow timeout, with no view inside |
| Debugging | two processes; `--wait-for-debugger`; the trace (§2.6); `await app.pause()` opens the in-app inspector on the app under test (Playwright `page.pause`) | one debugger across test and app code |
| Parallelism | N app processes, each with its own headless runner or private display (Xvfb per worker, `SVELTERS_HEADLESS_FORCE` as WG3) | nextest processes; wgpu adapter creation should be serialized (cadatomic pitfall 12) |
| Multi-window and native input | real windows, deferred viewports, DnD across windows, rfd dialogs, IME and OS input (§4) | viewports through `EmbedHarness`/EM2 only; no OS windows, no OS input |
| CI cost | needs the app binary (cadatomic builds it anyway) and Node for the TS runner (the js job has it); GUI jobs under Xvfb/lavapipe | the crate's test binary, already built by nextest |
| Who writes them | anyone, TS or Rust; no Rust rebuild to change a test | Rust; a test change rebuilds the test crate |
| Agents and the recorder | same protocol as MCP, so a recorded agent session replays as a test | separate API |

**Recommendation.**
- **E2E tests run out of process**, as the user leans, against the binary built as shipped. They run in `--headless
  --clock virtual` by default in CI and windowed with OS input nightly (§4).
- **The in-process kittest layer stays** as unit and component tests (cadatomic L1/L2): components, kinds, panels and
  scene code, written with the same locators and `expect`. One Rust `Driver` trait has an `InProcess` backend and an
  `Ipc` backend, so a component test is promoted to an e2e test by switching the backend
  (`SVELTERS_E2E=process`), not by rewriting it.
- The shipped build's protocol is discussed in §6 Q1.

| Layer | Runs | Writes | Typical |
|---|---|---|---|
| L0 | `cargo test`, no egui | Rust | scene, compiler, loader, commands |
| L1 component | in process (kittest) | Rust `Driver`, TK1 `.svelte` cases, TS `mount` (through a test host) | one component or kind, props and events, snapshots |
| L2 app, headless | out of process, `--headless --clock virtual` | TS `@svelters/test`, Rust `Driver::Ipc`, cadatomic scenarios | flows across roots, docking, reload, agents |
| L3 app, windowed | out of process, real windows, optional OS input | the same files | multi-window, IME, DnD, native dialogs, conformance |

### 2.2 TypeScript: a cadatomic flow across roots, docking and the editor

```ts
// cadatomic/tests/e2e/rename-and-dock.spec.ts
import { test, expect } from '@cadatomic/test';           // extends @svelters/test with commands and documents

test.use({ app: { headless: true, clock: 'virtual', seed: 7, size: [1600, 1000], theme: 'dark' } });

test('rename a feature, dock the inspector into a new window, edit its source', async ({ app, doc }) => {
  await doc.open('fixtures/box.cad');                     // fixture: command.run debug.open_fixture, then wait.settled
  await app.command('body.box', { extents: ['10/1', '20/1', '30/1'] });   // semantic setup through the command path

  const tree = app.root('panel.model-tree');               // root= scope by RootKey
  await tree.getByRole('treeitem', { name: 'Box 1' }).dblclick();         // synthetic input, actionability first
  await tree.getByRole('textbox').fill('Base plate');
  await app.keyboard.press('Enter');
  await expect(tree.getByRole('treeitem', { name: 'Base plate' })).toBeSelected();   // one wait.for, evaluated per settle

  // berth: a semantic dock, then the same move by pointer (Rust-drawn tabs are reached through a11y=)
  await app.ext('berth').dock({ pane: 'inspector', into: 'new-window' });
  const win = app.window('Inspector');
  await expect(win.root('panel.inspector').getByLabel('Depth')).toHaveValue('30 mm');
  await app.locator('a11y=tab[name="Inspector"]').dragTo(app.window('main').locator('anchor=dock/right'));

  // quire: a kind extension plus real typing
  const editor = app.root('panel.editor').getByRole('textbox', { name: 'Panel.svelte' });
  await editor.ext('quire').setCursor({ line: 12, col: 4 });
  await editor.type('<!-- note -->');

  await app.clock.runFor(300);                             // transitions finish in virtual time; no sleeps
  await expect(tree).toMatchSceneSnapshot(`
    - tree "Model":
      - treeitem "Base plate" [selected]:
        - treeitem /Face \\d+/
  `);
});
```

- The `app` fixture spawns the binary with `--automation <socket> --headless --clock virtual --seed 7`, reads the
  token file and calls `session.hello`. It is worker-scoped; `app.reset()` between tests remounts roots and closes
  documents.
- On failure the runner writes a trace (§2.6) and prints `SVELTERS_SEED=7`.

### 2.3 Rust: one body, in process or out of process

```rust
// crates/svelters-egui/tests/e2e_panel.rs (or cadatomic-app/tests)
use svelters_automation::prelude::*;                       // Driver, Locator, expect, role, name

#[svelters::test(app = "fixtures/embed-egui", seed = 3, iterations = 20, clock = "virtual")]
fn two_panels_keep_their_own_clicks(d: &mut Driver) -> TestResult {
    let (one, three) = (d.root("p1"), d.root("p3"));
    one.get_by_role("button", name("Inc One")).click()?;
    one.get_by_role("button", name("Inc One")).click()?;
    three.get_by_role("button", name("Inc Three")).click()?;
    expect(one.get_by_text("One clicks: 2")).to_be_visible()?;
    expect(three.get_by_text("Three clicks: 1")).to_be_visible()?;
    d.clock().run_for(250)?;
    expect(one).to_match_scene_snapshot("two-panels")?;  // tests/snapshots/two-panels.yaml
    Ok(())
}
```

- By default `#[svelters::test]` builds `Driver::InProcess` over `EmbedHarness` (kittest).
- `SVELTERS_E2E=process` builds `Driver::Ipc` against the `svelters run --automation` test host, or against
  `SVELTERS_APP=<binary>`.
- `iterations` reruns with seeds `3..23`; a failure prints the seed.
- `Driver` also exposes the raw `call(method, params)` and typed reads (`scene()`, `svelte_tree()`).

### 2.4 `.svelte` component tests (TK1, extended)

```svelte
<!--
@case Selecting a row shows it in the header and survives a keyed reorder.
@toolkits egui
@clock virtual
@seed 3
-->
<script>
  import { Window, Column, Button } from 'svelters:testkit';
  let rows = $state(['a', 'b']), sel = $state(null);
</script>
<Window title="Rows">
  <Column>
    <label anchor="header">{sel ?? 'none'}</label>
    {#each rows as r (r)}<Button text={r} anchor="row-{r}" onclick={() => (sel = r)} />{/each}
    <Button text="Add" onclick={() => (rows = ['c', ...rows])} />
  </Column>
</Window>
<!--
@test
click role=button[name="Add"]
expect count role=button[name=/^[abc]$/] 3
click anchor=row-b
expect text anchor=header "b"
advance 250
expect snapshot root `
  - window "Rows":
    - label "b"
    - button "c"
    - button "a"
    - button "b"
`
snapshot "after-select"
-->
```

- New steps: `click/type/expect` take any §1.3 selector; a bare string keeps TK1's meaning (text).
- New steps: `expect count|text|value|state|snapshot`, `advance <ms>` (virtual clock), and `@clock`/`@seed` headers.
- Cases run in process on egui and through the log replay on `log-gtk`/`log-qt`, as TK1 does. Selectors there run on
  the host's style mirror (CSS.md §2.2).

TS component tests (Storybook-style, for UI authors who never write Rust) mount into a test host:

```ts
import { test, expect } from '@svelters/test';
import Panel from '../src/Panel.svelte';

test('Panel counts and takes new props', async ({ mount }) => {
  const c = await mount(Panel, { props: { title: 'One' }, size: [320, 200] });   // a fresh slot in the test host
  await c.getByRole('button', { name: 'Inc One' }).click();
  await expect(c.getByText('One clicks: 1')).toBeVisible();
  await c.setProps({ title: 'Uno' });
  await expect(c.getByRole('button', { name: 'Inc Uno' })).toBeVisible();
});
```

`mount` sends `app.mount {module, export, props, slot}` to a `svelters run --automation` host. That host compiles the
file live with our compiler through the loader, so no bundling step is needed.

### 2.5 A self-modifying UI test

```ts
test('an edit keeps $state, and a broken edit reverts', async ({ app, sources }) => {
  const panel = app.root('p1');
  const inc = panel.getByRole('button', { name: /^Inc / });
  await inc.click(); await inc.click();

  const ok = await sources.patch('src/Panel.svelte', ['Inc {title}', 'Plus {title}']);   // write + reload.apply
  expect(ok).toMatchObject({ ok: true, hot: ['src/Panel.svelte'], fallback: null });
  await expect(panel.getByRole('button', { name: 'Plus One' })).toBeVisible();           // locator re-resolves
  await expect(panel.getByText('One clicks: 2')).toBeVisible();                           // H3 kept it

  const bad = await sources.patch('src/Panel.svelte', ['{title}', '{title.nope.x}'], {
    probes: [panel.getByRole('button', { name: /^Plus / })], onFail: 'revert' });
  expect(bad).toMatchObject({ ok: false, reverted: true });
  await expect(panel.getByRole('button', { name: 'Plus One' })).toBeEnabled();
});
```

In cadatomic the `sources` fixture maps to `ide.sources.write` on a proposal branch. In plain svelters it writes the
file and calls `reload.apply`.

### 2.6 Fixtures, roots, time, snapshots, traces and the recorder

| Need | API | Notes |
|---|---|---|
| Fixtures | `@svelters/test` on the **`@playwright/test` runner**: `test.use`, worker fixtures (`app`), test fixtures (`mount`, `sources`, `doc` from `@cadatomic/test`), `beforeEach`, retries, sharding, reporters, `expect.poll` | the runner works for non-browser tests; we bring our own `app`, `Locator`, `expect` and trace (§6 Q2) |
| Several roots and windows | `app.root(key)`, `app.window(title \| id)`, `root=`/`window=` engines; `app.roots()` lists them with `tooling` hidden | ARCH-EMBED roots and keys, EM2 viewports |
| Docking (berth) | `app.ext('berth').dock/undock/float/split` (semantic) and drags between `a11y=tab[...]` and drop anchors (synthetic, or OS for cross-window) | berth registers its automation extension; tab strips are Rust-drawn |
| Code editor (quire) | `locator.ext('quire').setText/setCursor/select/diagnostics()` plus `type`/`press` for real keys and IME `Preedit` | quire's kind extension |
| Deterministic time | `clock: 'virtual'`, `app.clock.runFor/fastForward/pauseAt/resume` | §1.6 |
| Seeded randomness | `seed`, `iterations`, `SVELTERS_SEED` | §1.6; the full scheduler is ZED #1 |
| Snapshots, text | `toMatchSceneSnapshot` (YAML, partial match, regex names; Playwright aria-snapshot rules) | the default regression artefact |
| Snapshots, pixels | `toHaveScreenshot({ mask, maxDiffPixels })` through kittest/wgpu on lavapipe; `.new.png`/`.diff.png` as TK1 | few, small, masked (cadatomic decision 12) |
| Snapshots, VIS1 | `toMatchMetrics('vis/button')` over `capture.metrics` | VIS1's ±1 px / ΔE2000 ≤ 2 rules |
| Trace on failure | `trace: 'retain-on-failure'`: per step the action, selector, resolved ref, reply, a YAML snapshot before and after, the journal slice, log lines, the op-log slice, and an optional screenshot; one `.svtrace.zip` | opened in the inspector's replay mode (§3.3), which also serves Cypress-style time travel |
| Recorder and codegen | the inspector's Record button: interactions in the app become steps with the best locator (§1.3 priority; uniqueness checked) and assertions picked from the tree; output as TS, Rust `Driver`, TK1 steps or a cadatomic scenario NDJSON | Playwright codegen / Squish recorder; an MCP agent session records the same way |

## 3. The inspector

### 3.1 In process or out of process: the trade-offs

| Axis | In process (overlay or window inside the app) | Out of process (a separate app over the protocol) |
|---|---|---|
| Data access | direct: scene, journal, style engine, H3 registry, at frame boundaries, no snapshot copies | everything serialized; a 10k-node tree costs a snapshot per refresh |
| Live editing | CSS engine overrides and HMR in the same process, applied the same frame | the same methods, one round trip later; fine for edits, slower for scrubbing |
| Picking | the overlay (`ScenePass` + an overlay root, SG4) and `hit_test` (SG3) in the frame | still needs the in-app overlay (CDP and Flutter both draw it in the page or app) |
| Dogfooding | written in Svelte on our own widgets (tree view, virtual list, property grid, split view) | the same if the detached app is itself svelters |
| Setup | a command or F-key in the running app; no second binary | launch and attach (socket, token) |
| Risk: a buggy inspector | shares the process: a Rust panic or a runtime-wide JS fault can take the app down unless isolated (own `Embed` and runtime, provider panic per root) | isolated; a crash in the inspector costs nothing |
| Observer effect (perf) | its frames share the UI thread: layout and paint of its roots, journal observers, AccessKit when the a11y panel is on | only serialization and the observers it subscribes |
| Observer effect (focus, hover) | separate egui viewport: focus is per viewport (ARCH-SCENE §9), so the app keeps keyboard focus; overlay hover only in pick mode | none |
| Hung app | frozen with it | can show `app.health` (busy bits, the running JS turn and for how long, the last snapshot) and send `app.interrupt` |
| Crashed app | gone with it | keeps the last trace and snapshot |
| Multi-window | lists every viewport; pick works in any (`PassScope::AllViewports`) | the same over the protocol |
| Remote and headless | no (a headless app has no window to host it) | yes: CI apps, a headless app under test, cadatomic on another machine |
| Self-modifying UI | must be protected from the live IDE's edits (§1.11) | out of reach of the edits by construction |

**Recommendation.**
- **The inspector runs in process**, as the user leans, but isolated and written as a protocol client:
  - its own tooling `Embed`, with its own QuickJS runtime, `id_salt` and fault scope. An app runtime fault or full
    reload leaves it running; verify that two `Embed`s share one egui context cleanly in AU4;
  - a built-in, immutable bundle, protected from the live IDE;
  - shown by default in its own egui viewport window (EM2), dockable as a berth pane;
  - its data comes only through `svelters:automation`. Only the pick overlay and the box-model highlight are Rust
    `ScenePass`es.
- **The detached mode comes later (AU5)**: the same bundle in a second svelters process over IPC, for hung, crashed,
  headless and remote apps. It costs no rewrite because the panels only ever saw the protocol (React DevTools: one
  backend, two frontends).
- **Observer budget.** Closed, it costs nothing (no observers; the journal costs a mask test). Open, it adds
  ≤ 1 ms per frame on a 10k-node root (a target, to be measured in AU4). AccessKit is on only while the a11y panel is
  open; snapshots are taken on demand; its own roots are excluded from perf counters by origin.
- **Key binding:** cadatomic binds F12 to `dev.toggle_inspection`, its kernel panels (02-phase-a A2d). svelters
  registers a command `dev.ui_inspector`, and cadatomic chooses its key.

### 3.2 Panels

| Panel | Content | Built on |
|---|---|---|
| Pick overlay | hover outline with tag, anchor, role and size; click selects; margin, padding and content boxes; layout boundary and dirty bits (ARCH-SCENE §11 "F12 pass") | `ScenePass` (AllViewports) + `hit_test`; an overlay root interactable only in pick mode |
| Elements | scene tree per root and window; filters by kind, anchor, text and origin; `include_tooling`; virtual rows; "show implementation nodes" (shadow children) as Flutter's toggle | `virtual-list`/RowPool (LS1); WG5b tree-view when merged |
| Components | the Svelte component tree with props and `$state` per declaration (editable later through SV30 overrides); go to source | `svelte.tree`/`svelte.instance` (AU3) |
| Props, state, style | props (typed editors from the kind spec), state bits, computed style with matched rules, layers and source spans ("why is this red"), box model | `scene.node`, `style.computed`, `scene.layout` |
| Live edit | edit a declaration or rule; toggle a class; write to source (an HMR or H2 CSS-only update) or as an override (transient; reset restores) | CS engine override layer (`style.set`), `reload.apply` |
| Events and journal | delivered events, handler errors, the change journal filtered by origin (user, JS, Rust, agent X) | `events`, `journal` streams |
| Performance | frame times, scene time split, JS time and ops per root, layouts and boundaries, journal volume; a recording of N frames | `perf` stream (TR3, `SceneStats`) |
| Accessibility | the AccessKit projection beside the scene tree, with TK3 audit results inline (unnamed input, set size, duplicate ids across roots) | `a11y.tree`, TK3 |
| Console | logs, diagnostics, faults; `$0` is the selected ref; typed commands (`inspect`, `get`, `set`, `watch`) before any JS eval (inspector-demo.md §7) | `log` stream |
| Record and replay | the recorder (§2.6); open a `.svtrace.zip` and step through its snapshots | AU5 |

### 3.3 Relation to earlier plans

- ARCH-SCENE §11's "F12 inspector" (SG7) becomes the inspector's Rust overlay pass plus a text-dump fallback that needs
  no JS.
- inspector-demo.md's four views (component graph, insertion graph, native visual tree, object relations) map to
  Components, Elements and the `node.ext` extensions. Its GTK/Qt adapters stay maintenance-only under the egui-first
  focus.
- The protocol is backend-neutral. GTK and Qt get the host-level subset (style mirror queries, `get_prop`, the TK1
  semantic actions) when someone needs it.

## 4. Cross-platform real-input runs

| Mode | Injector | Coordinates | Needs | When |
|---|---|---|---|---|
| semantic | the dispatcher (§1.5) | none | nothing | agents, setup, most L1 |
| synthetic | egui `RawInput` in the next frame | the node's actionable point, in points | nothing | default for tests (L1–L2) |
| os, Linux | XTEST (`crates/svelters-conformance/src/xtest.rs`, x11rb) | `scene.layout` + window origin × `pixels_per_point` → root coordinates | a private Xvfb per worker (`tools/dev/headless`, `SVELTERS_HEADLESS_FORCE=1`); a Wayland injector is out of scope (XWayland as now) | L3 nightly, native pins, IME |
| os, Windows | SendInput; UIA for focus and tree cross-checks (WG3w) | the same, through the HWND's client origin | an interactive desktop session on the runner | L3 nightly |
| os, macOS | CGEvent; AX for the cross-check (WG3m) | the same, with the Retina scale | Accessibility (TCC) permission for the runner | L3 nightly on the macbook |

- One `OsInput` trait (`key`, `text`, `pointer_move`, `button`, `wheel`, `drag`) in `svelters-conformance`, with
  the three injectors as backends, so WG3's pin capture and e2e tests share them. The runner picks the mode per run
  (`input: 'os'`), never per test.
- In OS mode the clock is real (`clock: 'real'`); waits still go through `wait.for`.
- **The OS accessibility cross-check** (TK3's platform half, NA3): in L3 runs, compare the platform tree (AT-SPI,
  UIA, AX) with `a11y.tree` for roles, names and states per author id. This tests the AccessKit adapters, which
  kittest cannot.
- **Cost.** OS runs are serial per display and real-time; they run nightly and for conformance, not on every push.

## 5. Milestones

| Lane | Content | Size (lane-days) | Depends | Acceptance (examples) | cadatomic |
|---|---|---|---|---|---|
| **AU1** protocol core, in process | `svelters-automation`: the method schema; selector engines (§1.3) on SG3 + CS0 with test pseudo-classes and the kind→role table; reads (§1.4 minus `svelte.*`); semantic and synthetic acts with actionability; `wait.settled`/`wait.for`; the virtual clock in embed mode; seeds; `journal`/`events`/`log`/`perf` streams; `capture.screenshot` and `scene.snapshot` (SG7); sessions, grants, origins and tooling roots; `Driver::InProcess` + `#[svelters::test]`. Replaces TK2 and absorbs SG7 | 6–8 | SG3, SG4, CS0, CS5e (all merged) | gallery and `embed-egui` kittests rewritten with locators; the same selector resolves to the same `NodeId` in Rust and through the in-process JSON path (SL5); a 30 s wait in virtual time takes < 1 s wall | **first**: in-app agents and its L1/L2 tests |
| **AU2** IPC, clients, headless app mode | the IO thread and NDJSON JSON-RPC framing (cadatomic's), the reference server (`svelters run --automation`), `app.health`/`app.interrupt` off the UI thread; `--headless --clock --seed`; `Driver::Ipc`; `@svelters/test` on `@playwright/test` (`app`, `mount`, locators, `expect`, `toMatchSceneSnapshot`, `toHaveScreenshot`) | 5–7 | AU1 | §2.3's test passes in both modes with the same seed and the same YAML snapshot; a test that crashes the app fails alone and the next test passes; a JS infinite loop is broken by `app.interrupt` | **first**: mounts the methods in A8a's server; `KittestDrive`/`RpcDrive` wrap `Driver` |
| **AU3** reload-aware automation, Svelte tree | `Busy::RELOAD` live; `reload.apply {paths, probes, on_fail}` replying after settle; epochs and `node.stale`; `component=`/`source=` engines on compiler identity (anchors until SV16 lands); dev hooks on `$n.hmr` instances: `svelte.tree`, `svelte.instance` with H3 values; the protected tooling `Embed`; the `reload` stream | 5–7 | AU1, H1, H3 (merged); SV16/H4 identity for the full `component=` engine | §2.5 passes: a good edit keeps `$state`, a broken one reverts and the probe still clicks; two `Embed`s in one context, app runtime fault leaves the tooling `Embed` running | **first** for the self-modifying UI |
| **AU4** inspector v1, in process | §3.2 panels except Record and Replay, in the tooling `Embed`, own viewport window, dockable; the Rust pick and box-model passes; live CSS edit (override + to-source) | 8–10 | AU1, AU3; virtual-list (LS1, merged), WG5b tree-view (queued) or a plain list first | pick in two windows selects the node under the pointer; a CSS edit shows in the same frame and resets; inspector open on a 10k-node root costs ≤ 1 ms/frame (measured) | dev tool for its Svelte panels |
| **AU5** traces, recorder, detached inspector, TK1 steps | `.svtrace.zip` and replay; the recorder emitting TS, Rust, TK1 steps and cadatomic scenario NDJSON; the inspector bundle detached over IPC; TK1 locator, `count`, `snapshot` and `advance` steps | 5–6 | AU2, AU4 | a failing test leaves a trace whose replay shows the step that failed; a recorded session replays green in the other three formats | scenario recording (A8 decision 12) |
| **AU6** OS input runs | the `OsInput` trait over XTEST, SendInput and CGEvent; `input: 'os'`; the platform accessibility cross-check | 3–4 Linux, + 2–3 per platform | AU2; WG3w, WG3m | §2.2's docking test passes on Linux under XTEST in L3; the AT-SPI tree agrees with `a11y.tree` on the gallery | DK cross-window drags (W9) |

- **Order:** AU1 → (AU2 ∥ AU3) → AU4 → AU5; AU6 any time after AU2. Total about 32–42 lane-days.
- **cadatomic needs first:** AU1 (in-app agents and locators in its kittest layer); then AU2's mount into its A8a
  server and AU3 for the self-modifying loop. The inspector (AU4) comes after.
- **Related lanes:**
  - TK3 (the a11y audit) uses AU1's `a11y.*` reads and adds the scene↔AccessKit agreement check;
  - ZED-LESSONS #1 (`TestScheduler`) plugs into AU1's `--seed`;
  - CSS D8 and CLI decision 7 are answered by AU2: a generic protocol, and MCP through cadatomic's sidecar or a thin
    `svelters mcp` over the reference server.
- **Files:** AU1–AU3 add a new crate and touch `embed/settle.rs` (the RELOAD bit), the host clock
  in `core.rs`, `packages/native-runtime` (dev hooks), the compiler (dev hook emission), and `packages/test`
  (new). They conflict with little that is in flight; the host clock change goes through the coordinator (HOST
  owner).

## 6. Open questions

**Decided (user, 2026-10-22):**
- **Q1, shipped builds:** the protocol is built in and runs in process in production, but is **off by default in builds shipped
  to users**. Turning it on (it is what the inspector and in-app agents need) goes through a settings page with a warning
  (cadatomic's settings UI). The external socket stays behind a flag and a token on top of that. OS input, clock and eval
  are allowed only in test builds.
- **Q2, TypeScript runner: our own runner**, not `@playwright/test`. Playwright stays prior art for the API shape
  (locators, auto-waiting, web-first assertions, fixtures, traces).
- **Q3, inspector UI: Svelte in a protected root**, as recommended.


1. **Is the protocol in shipped builds?** cadatomic's pixel tier is a `compile_error!` in release builds (A8a-2). In-app
   agents and the self-modifying UI need the protocol in release, and the user wants e2e against the build as
   shipped.
   - **Recommend:** the in-process transports are always built. The external socket is in release builds but off
     unless the app is started with `--automation` and a token. `input.os`, `clock` and `eval` grants are
     test-build only. `svelte.*` dev hooks follow the editable layers. The coordinators settle this as a seam
     (svelters API, cadatomic policy).
2. **The TS runner:** `@playwright/test`'s runner with our own fixtures, locators, `expect` and trace, or vitest, or
   our own?
   - **Recommend `@playwright/test`.** Its fixtures, workers, retries, sharding, reporters and UI mode are
     browser-independent. Its trace viewer cannot read our traces, so replay is the inspector's (AU5).
3. **The inspector UI in Svelte or in Rust?** cadatomic §6.6 keeps "the IDE's editing surface" in Rust so that a Svelte
   edit cannot take away the tools to undo it.
   - **Recommend Svelte** (dogfood, and the user's lean toward an in-process, widget-built inspector) inside the
     protected tooling `Embed`, editable only under staged apply.
   - Keep the pick overlay, the recovery affordances and a no-JS text-dump fallback in Rust.

## Sources

Fetched this session:
- Playwright Clock API: <https://playwright.dev/docs/clock>
- Playwright aria snapshots: <https://playwright.dev/docs/aria-snapshots>
- Flutter DevTools inspector: <https://docs.flutter.dev/tools/devtools/inspector>

From prior knowledge, not re-checked this session (verify before relying on details):
- Playwright: locators, actionability, trace viewer, codegen, fixtures and parallelism
  (<https://playwright.dev/docs/locators>, `/actionability`, `/trace-viewer`, `/codegen`, `/test-fixtures`,
  `/test-parallel`)
- Testing Library query priority: <https://testing-library.com/docs/queries/about#priority>
- Cypress: <https://docs.cypress.io>
- WebdriverIO: <https://webdriver.io>
- Appium: <https://appium.io/docs/en/latest/>
- Flutter integration tests: <https://docs.flutter.dev/testing/integration-tests>
- Squish: <https://www.qt.io/product/quality-assurance/squish>
- QTest: <https://doc.qt.io/qt-6/qtest-overview.html>
- Microsoft UI Automation: <https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32>
- AccessKit: <https://github.com/AccessKit/accesskit>
- egui_kittest: <https://docs.rs/egui_kittest>
- Svelte inspector (vite-plugin-svelte): <https://github.com/sveltejs/vite-plugin-svelte/blob/main/docs/inspector.md>
- React DevTools standalone: <https://www.npmjs.com/package/react-devtools>
- Chrome DevTools Protocol: <https://chromedevtools.github.io/devtools-protocol/>
- Storybook tests: <https://storybook.js.org/docs/writing-tests>

From local sources:
- Zed `gpui::test`, through ZED-LESSONS.md §6;
- egui_inspection and egui_mcp, through cadatomic's `agent-driven-testing.md` (versions and verification state there).
