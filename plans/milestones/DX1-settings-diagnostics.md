# DX1: settings binding, persisted UI state, standard paths, JS diagnostics

Spec: `plans/research/QT-SURVEY.md` §2.1, §2.2, §2.8 and §3 row 5 (QD1, QD2, QD7, QS13 core half).
Constraint (coordinator, from cadatomic): **svelters owns no persistence.** Settings and UI state bind to a
store the host supplies through a trait; the in-memory and simple-file defaults are for tests and for apps
that bring no store. No security or policy work.

## As built

| Piece | Where | What |
| --- | --- | --- |
| `SettingsBackend`, `UiStateBackend` | `svelters-host/src/dx/settings.rs` | The traits. Settings: `get`, `set` (rejects with `SettingsError::Locked { by }` or `Invalid`), `source` (`{ layer, locked }`), `subscribe` (callback on any thread, `Subscription` guard). UI state: `load(root)` / `save(root, map)`, the map keyed by anchor. |
| Defaults | same | `MemorySettings` (with `lock(key, value, layer)`), `FileSettings` (one JSON object, atomic rename, rolls memory back when the write fails), `MemoryUiState`, `FileUiState` (`<dir>/<escaped root>.json`). |
| `AppId`, `Paths` | `dx/paths.rs` | `AppId { qualifier, org, app }`; `Paths::for_app` (the `directories` crate; `state` falls back to `<data_local>/state` off Linux) and `Paths::under(root)` for tests. |
| `Dx`, `Host::set_dx` | `dx/mod.rs`, `lib.rs` | What the app supplies: `AppId`, `Paths`, both backends. `Dx::files(app)` builds the simple-file defaults (`settings.json` under config, UI state under `<state>/ui`); `Dx::in_memory`. |
| `svelters:settings` | `dx/settings.js` | `setting(key, fallback)` -> `{ value, source, locked }`, reactive; a write to a locked key throws an error with `code === 'settings/locked'` and the value stays. `uiState(defaults, anchor, { root })` -> a reactive, deeply proxied object, defaults filled under the stored state, saves debounced 250 ms; `flushUiState()`; `refresh(keys)` for changes made outside. |
| `svelters:paths` | `dx/paths.js` | `paths.{config,data,cache,state,runtime}` and `app.{qualifier,org,name}`, read-only. |
| `svelters:log` | `dx/log.js` | `logger(category)` -> `trace/debug/info/warn/error(message, fields?)`. |
| `__svelters_host.dx(op, a, b)` | `quickjs.rs`, `core.rs` (`dx_op`) | The one host op behind the three modules. Errors reach JS as `Error` with `code` (`settings/locked`, `settings/invalid`, `settings/unavailable` when the host has no `Dx`). |
| Console -> `tracing` | `core.rs` (`emit`) | Every `console.*` and `logger()` record is a `tracing` event with target `js` and a `category` field (empty for `console`), also kept in the console buffer and echoed as before. |
| Crash context | `svelters-host/src/crash.rs` | A per-thread ring of the last 32 host op names (every `Core::timed` call now names its op) and the last 8 reported JS errors with source-mapped stacks. `crash::context()` returns it; `crash::install_panic_hook()` appends it to panic reports. |
| Loader | `svelters-modules/src/compose.rs` | An added native set may now serve ids outside `svelters:app/*` (`svelters:settings`), gated by `NativeAccess` like other added sets. `ModuleHooks::default()` registers the DX1 modules in the added set, so `ModuleHooks::isolate()` (deny all) does not load them. |

Reactivity uses the runtime's tracker (`globalThis[Symbol.for('svelters.types.tracker')]`: `local` to subscribe,
`invalidate` to re-run) with a fixed tag as the handle, so `packages/native-runtime` is untouched. Without the
runtime, reads are plain.

## Tests

`crates/svelters-host/tests/dx.rs` (apps from source, like `modules.rs`): reads, writes and locked-key rejection with
its source; split position and column widths survive a restart through `FileUiState`; `paths`/`app`; `logger` and
`console.warn` reach `tracing` with target and category; no `Dx` -> `settings/unavailable`; crash context lists ops.

## Not done / open

- **Identity of UI state.** The key is `(root, anchor)`, both given by the caller (`root` defaults to `"app"`). The
  compiler gives components no identity JS can read, so a panel passes its own anchor; deriving `root` from the
  embed's current `RootKey` waits for the egui lanes.
- **External changes** are not pushed into JS: `SettingsBackend::subscribe` exists, but the host does not wire it to
  `refresh` (that needs a cross-thread wake into the JS loop; fold it into the R1 batch).
- **`#[derive(Settings)]`** and the generated `svelters:settings/<group>` modules (schemars typings) are not built.
- **Per-category `RUST_LOG`.** `tracing` targets are static, so the category is a field: filter with
  `js[{category=cad.task}]=debug`, not `js::cad.task=debug`.
- **JS stack at the panic point** is not captured (QuickJS is not safe to re-enter from a hook): the report carries the
  last ops and the last reported JS errors.
- **Policy.** `__svelters_host.dx` is reachable from any JS that reaches `__svelters_host`, like every other host op;
  gating it by extension is SEC1's.
