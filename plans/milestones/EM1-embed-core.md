# EM1: the EMBED core (SC1's svelters part, with RA1)

Design: `plans/research/ARCH-EMBED.md` (Decided D1–D12 and the seam items; §4 API, §5 frame, §6 threading
and failures, §8.1 ids, §10 EM1 row) and `plans/research/ARCH-SCENE.md` "Decided" (R16, R17, R20, R21). Starts
from EG0 (`plans/milestones/EG0-module-split.md`). Lane: EM1 (EMBED writer), branch `ci/em1`.

## What landed

### API cadatomic gets (`svelters_egui::embed`, `svelters_egui::Embed`)

```rust
let embed = Embed::new(&ctx, EmbedConfig::default())?;          // owns backend + host (Host::new_owned), attached
let entry = embed.load("ui/dist/embed.js")?;                     // svelteConfig({ embed }) bundle, namespace kept
let slot = embed.create_slot("panel.task", SlotOptions::fill().label("Tasks"));
let root = embed.mount(MountSpec::new(entry, "TaskPanel", slot, "panel.task").props(Props::new().with("count", 3)))?;

// App::logic                       App::ui (any pane, root viewport or immediate viewport)
embed.logic(ctx);                   embed.show_slot(ui, slot) -> SlotResponse { rect, content_size, state }

embed.set_root_props(root, Props::new().with("count", 4).delete("old"))?;   // queued, shallow merge (D10)
embed.unmount(root); embed.request_close(root); embed.restart_root(root);  // queued
embed.destroy_slot(slot); embed.set_visible(slot, false); embed.set_viewport(slot, vp);
embed.take_events();       // Mounted, Faulted{root, RootError{cause, error}}, CloseDecided, InfoChanged, Emitted, Error{root?}, RuntimeFaulted
embed.root_info(root);     // key, slot, state, title, dirty (written by JS), size, viewport, visible, owner
embed.settled();           // Settle { busy: Busy bits, next_timer }
embed.busy_probe();        // Send + Sync: barrier.add_busy_source({ let p = embed.busy_probe(); move || p.count() })
embed.anchor_rect(root, "heading"); embed.anchor_id(root, "heading");
embed.host(); embed.backend(); embed.stats();
```

`EmbedConfig { id_salt, host: HostConfig, budgets: Budgets { warn 8 ms, interrupt 50 ms, mount 500 ms },
faults: FaultPolicy { handler_errors: Some((5, 10 s)) }, timer_horizon: 50 ms, prepass_plugin }`.
`EmbedConfig::app(host_cfg)` is the desktop/`TestApp` flavour (every entry under the host's `turn_budget`, as before).

Tests and other embedders: `svelters_egui::harness::EmbedHarness` (`new(size, cfg)`, `set_ui(|embed, ui| …)`,
`run`, `step`, `run_until_settled(max) -> Result<u32, NotSettled { chain: Vec<Busy> }>`).

### Host (`crates/svelters-host`)

- `Host::new_owned(backend, cfg)`: the host owns the boxed vtable (last field), no leak, no `unsafe` at call sites.
- `load_entry(path) -> u32` keeps the module namespace (`JsEngine::load_entry`, QuickJS `Module::namespace`).
- Root calls, each one entry into JS under the current turn budget, then a job drain (`core::root_call`):
  `mount_root(entry, export, root, target, props)` (adopts the native target handle first), `root_props`,
  `root_info`, `unmount_root`, `close_root -> bool`. Engine side: `JsEngine::{mount_root, root_call}` calling
  `globalThis.__svelters_embed.*`.
- Error origin: `JsError::handle` is set for errors of dispatched events (`deliver`, `call_sync`); embedders map
  it to a root through the scene.
- `busy() -> HostBusy` (events, changes, jobs, ops in flight, stores, frame, running, next timer).
- `set_turn_budget`/`turn_budget`: the budget is a `Cell` in the core read by every `arm()` (per-entry budgets).
- `take_root_messages() -> Vec<RootMessage>`: `Fault`/`Info`/`Emit` from the natives `__svelters_host.rootFault`,
  `rootInfo`, `rootEmit`.

### Runtime (`packages/native-runtime`)

- `src/embed.js` (`@svelters/native-runtime/embed`, also `svelters:embed` through the Vite alias): installs
  `__svelters_embed { mount, props, info, unmount, close, roots }`; per-root `$state` props and info;
  `root()` (context `ROOT_KEY`, else the host's current root) with `id key visible width height scale viewport
  focused title dirty emit onCloseRequest`; per-root rAF queues held while the root is hidden.
- `mount(…, { onerror })` (index.js, svelte.js): with `onerror` the root's branch is a real boundary
  (`boundary_scope`), so render and effect errors after the first render go to `onerror` → `rootFault`.
- Types: `types/embed.d.ts`, `types/embed-module.d.ts` (`declare module 'svelters:embed'`).
- Tests: `test/embed.test.mjs` (props merge and `$$delete`, info reactivity, close handlers, emit, boundary
  fault, first-render throw, per-root rAF held while hidden).

### Build (`packages/build`)

`svelteConfig(root, { embed: { TaskPanel: 'src/TaskPanel.svelte', … } })` (or `"svelters": { "embed": … }`)
generates `<root>/node_modules/.svelters/embed.js` (`import '@svelters/native-runtime/embed'; export { default as
TaskPanel } from …`) and builds `dist/embed.js`.

### Egui (`crates/svelters-egui`)

- `embed/{mod,roots,slots,settle}.rs`: the API above. Commands are queued and applied in `logic`
  (mount → `mount_root` under the mount budget; props; unmount; close; restart; dispose; destroy slot).
  `logic`: commands, root info sync (only changed fields, so no JS per frame), `logic_pass` (closes, checkpoint,
  rAF, remeasure under the interrupt budget), error/fault collection, commands again (fault disposals),
  repaint requests, probe. First pass only, once per pass.
- Faults: render (mount error or `rootFault`), handler (policy count; each exception is `EmbedEvent::Error`
  with its root), interrupt (an `Interrupted` error whose handle maps to a root), runtime (OOM: all roots),
  panic (`catch_unwind` around a slot's walk; dispose queued). A fault disposes the root's JS and frees anything
  left under the slot natively. Default fault UI: "`<root key>` failed: `<message>`" plus "Reload".
- `show_slot`: hidden → nothing; faulted → fault UI; otherwise a child `Ui` with `scope_id` = the slot's id,
  AccessKit group (label, author id = slot key), layout when the slot's size changed or its cache was cleared
  (`walk.rs::slot_ui`), paint, outbox flushed to the host queue (no JS), probe.
- Ids (`ids.rs`): slot `Id::unique(("svelters", salt, "slot", key))`; root base `Id::unique(("svelters", salt,
  root_key))`; anchored `base.with(("a", anchor))`; else `base.with(("h", handle))`; nodes outside roots keep
  `Id::unique(("svl", h))`. A subtree takes its root's ids when inserted (`ids::adopt`, called by the insert op);
  `anchor` changes re-id (`ids::reanchor`). Two embeds with one salt in a `Context` fail a debug assertion.
- `a11y.rs`: `decorate` (name + author id = anchor) used by `leaf.rs`; `slot_group`.
- Runners: `TestApp` (`harness`, `embed`, `backend`; `host()`), `run_app` and `svelters-egui-run` are thin users
  of `Embed` (`load_app` + `show_windows`); `EguiBackend::ui` (the `svl_backend.run` path and the recording-host
  rigs) is unchanged in behaviour (`logic_pass` + `show_windows`).

### Tests

`crates/svelters-egui/tests/embed.rs` on `fixtures/embed-egui/dist/embed.js` (Panel, Broken, Fragile, Spinner,
Ticker), the EM1 acceptance list:

| Acceptance | Test |
|---|---|
| two roots in one runtime in a host `Ui` (left and right); props reactive; unmount frees every handle (`live_callbacks`, arena len) | `two_roots_in_one_runtime_update_reactively_and_unmount_cleanly` |
| ids unchanged across a keyed reorder, a second root, a second embed with another salt; anchored ids stable across runs; author ids | `ids_are_stable_across_a_keyed_reorder_a_second_root_and_a_second_embed` |
| a throwing component faults only its root (first render and after mount), Reload, restart | `a_throwing_component_faults_only_its_root` |
| an infinite handler loop interrupted ≤ 100 ms (measured: `logic` 51 ms), only that root faults | `an_infinite_handler_loop_is_interrupted_within_100ms_and_faults_only_its_root` |
| `run_until_settled` ≤ 3 iterations; an injected rAF cycle reported with its chain | `run_until_settled_converges_and_reports_an_injected_cycle` |
| idle: 5 slots, settled, no repaint requested within 2 s, a forced frame runs no checkpoint | `idle_is_zero_frames_with_five_slots` |
| handler exceptions reported with their root, the fifth faults it | `handler_exceptions_are_reported_then_fault_the_root_by_policy` |

## Deviations from ARCH-EMBED, and why

- **`svelters:embed` is a runtime module, not a host built-in.** The Vite plugin aliases it (with
  `@svelters/native-runtime/embed`) to `src/embed.js`, so the bundle stays self-contained and there is one module
  instance. The root info lives in the Rust root table (`Root::{title, dirty}`, `SentInfo`) and is mirrored into
  a per-root `$state` object: host fields are sent by `info` only when they changed, title/dirty come back through
  `rootInfo`. That keeps D11(a)'s stated properties (reactive in JS, no per-frame JS, readable from Rust) without
  an E3 store type per root. A host built-in can replace the alias later without an API change.
- **No `Value::Undefined`** in `svelters_abi`: deletion in a patch is `Props::delete(key)` (sent as `$$delete`).
  `Props::from(record)` (exports E0 `IntoJs`) is not there yet; `Props::json` and `with` are.
- **The outbox is flushed at the end of each `show_slot`** (queue only, no JS) instead of at the start of the next
  `logic`: same order for JS, and `settled()` sees it (`Busy::OUTBOX` until the checkpoint).
- **Not in EM1** (all EM2 per §10, or later): `current.rs` (thread-local registry, `SlotPainter`, `with_current`,
  begin-pass plugin; `prepass_plugin` is ignored), `painter`, `move_root`, per-viewport repaint routing
  (`touched()` still requests a whole-app repaint), focus restore on `set_viewport` (it only stores the viewport
  for `root().viewport`), the hidden/drawn debug checks, `restart_runtime` (an OOM emits `RuntimeFaulted` and
  faults every root; no rebuild), the Mounting spinner, `SlotSizing::FitHeight` (only `Fill`), `focused` (always
  false), the duplicate-anchor diagnostic.
- **Per-root rAF tagging** uses the root's effects (the active effect's chain) and the host's calls into a root.
  A `requestAnimationFrame` from an event handler (no active effect) is untagged and runs on the global queue.
  EM2 can tag dispatches by handle (the host knows the handle's root).
- **Budgets**: `warn` only logs a `logic` longer than 4×warn in debug builds (no per-entry warning yet).
- **`TestApp`** holds `embed` instead of `host` (accessor `host()`); no test used the field.
- Added for the desktop path: `Embed::load_app`, `Embed::show_windows`, `EmbedConfig::app`,
  `runner::run_desktop`.

## Notes for SG1 (scene)

Minimal scene additions, for SG1 to reconcile into its columns:

- `Node.root: u32` (0 = no root; ARCH-SCENE §4.5 `core.root`), `Node.host_owned: bool` (a slot: JS sees it
  `SVL_LIFE_BORROWED`), `Common.anchor: String`.
- The insert op (`ops.rs`) calls `ids::adopt(scene, root_bases, parent, child)` after `tree::insert` (the "attach
  hook"); `set_prop` calls `ids::reanchor` for `anchor`. `EguiBackend::root_bases` maps a root to its id base. When
  SG1's `SceneMut` path takes inserts, call `ids::adopt` there (it walks the subtree once, only when the root
  changes).
- Layout: `walk.rs::slot_ui` relays out a slot when its available size changed or its root cache is empty; it
  ignores the global `layout_dirty` (windows still use it). SG1's per-root `RootDirty` can replace both.
- `frame.rs` is split into `logic_pass` (frames counter, closes, checkpoint, rAF, remeasure), `show_windows`,
  `after_pass` (now `pub(crate)`) and `bind_ctx`.
- `settled()` reads `DirtyFlags::remeasure`, a slot's `cache.is_empty()`, the backend's `frame_pending`,
  `checkpoint_pending`, `deadline` and the wake flag. R16 (defer queue, wanting providers) and R17 (`Ui` phase
  `SceneBusy`) attach here: `Busy::PROVIDER` is reserved and never set by EM1.
- R21 (`show_viewport_passes`) and R20 are not touched.

## Notes for RA1 (kinds, providers)

- `anchor` is a new prop at the end of `WIDGET_PROPS` (`P::Anchor`, `kinds/builtin.rs`, `scene/props.rs`), so
  existing member ids are unchanged; `metadata/semantics/egui.json` was regenerated (17 lines added).
- Provider widgets: use `Node.id` (salted, stable across moves) as their egui id, and call `a11y::decorate` (or
  the same author-id call) on their response so anchors reach AccessKit.
- `Widget::busy()` should set `Busy::PROVIDER` in `Embed::settled` (ARCH-EMBED R8).

## Notes for EM2

- Start from `embed/current.rs` (empty seam) and `Embed::set_viewport` (stores only). Repaint routing: replace
  `touched()`'s `request_repaint()` with the node's root → slot → viewport (`Node.root` is there now).
- Hidden roots already hold their rAF (runtime) and are not drawn; JS sees `visible`. Add the debug checks and
  the handler-rAF tagging noted above.
- `Embed` keeps the egui `Context` from `new`/`logic` in the backend's `Shared` (rebound every `logic`).

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-host`, `-p svelters-egui --all-targets`, `-p svelters-egui
  --no-default-features --lib`, `--workspace --all-targets`: clean (pre-existing warnings only).
- `SVELTERS_UPDATE=1 … nextest -p svelters-egui --test spec` regenerated `egui.json`.
- `cargo nextest run -p svelters-egui`: 23/23 (the whole crate, eframe_idle under WSLg included), then
  `--test embed` 7/7 after the handler-policy test was added; both before the merge of origin/master (FW1), with
  the egui fixtures and `embed-egui` built locally (`pnpm --filter './fixtures/*-egui' build`).
- `pnpm --filter @svelters/native-runtime test` (102 tests) before `test/embed.test.mjs`; `embed.test.mjs` 3/3.
- Not run locally: workspace tests, svelters-host tests, conformance, the JS package suite after the merge, the
  other backends. CI runs them.
