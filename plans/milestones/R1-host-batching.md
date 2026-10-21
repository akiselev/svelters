# R1: host batching, compare, read-back; reactive getters (plans/research/REACTIVITY.md §10 R1, decisions §13)

Lane: `crates/svelters-host`, `crates/svelters-backend-log`, `packages/testhost`, the GTK backend's observe
path and tables (`crates/svelters-gtk`, `tools/metadata/gtk/gen_backend.py`), `packages/native-runtime/src/observe.js`,
`@svelters/types` core, the GTK and Qt types generators. Branch `ci/r1`. Out of scope: compiler `bind:` lowering and
`$n.bind_*` (R2), the Qt backend's QBindable/event/signal mechanisms (R3), AppKit/WinUI `observe` (R4).

## What changed

```text
native change ─prop_changed(h,p)─▶ dirty set (ordered, dedup; wake on first insert)
                                        │ checkpoint
                                        ▼
                     get_prop (skipped: SVL_M_PROPORTIONAL) ─▶ compare with last ─▶ drop equal
                                        │
                     __svelters_changed([h, p, v, ...])  ◀── one batch, before queued events, no drain in between
                                        │
         native-runtime tracker (observe.js): update() of each subscriber, observeProp callbacks
                                        │
                              Svelte flushes once (job drain after the first event or at the end)
```

### Host (`crates/svelters-host/src/core.rs`, `core/ops.rs`)

Every rule of REACTIVITY.md §5.2; the contract is written down in `plans/architecture/backend-abi.md`
"ABI v2.6 additive / Host contract":

- `Observed { count, name, proportional, last, force }` per (h, prop id). The 0→1 transition calls the backend and reads a
  baseline `last` (unless proportional), so a GTK re-notify of an identical value is dropped from the start.
- Dirty set: `Vec` + `HashSet`, first-announcement order; `prop_changed` returns true on the first insert into an empty
  set, and `Callbacks::prop_changed` wakes the backend then (one wake per turn).
- `take_changes` reads each dirty pair still observed (`get_prop`, never JS), compares (`same_value`: handles by id,
  strings by content across UTF-8/UTF-16, lists/records/maps deeply, f64 by bits with NaN = NaN), keeps the differing
  ones and updates `last`. A value with a `BOXED` handle is always delivered and never kept; reads nobody sees are
  released.
- `deliver_changes` runs at the start of every checkpoint (live and scripted) and before `call_sync` runs JS.
- Set path (`after_set`): after a successful `set_prop` on an observed, non-proportional pair, read back once into
  `last`; if the read-back differs from the written value, the pair is forced dirty (delivered once, normalized).
- `destroy`/`on_destroyed` drop observations and dirty entries; reload clears the dirty set with the event queue.
- `JsEngine::dispatch_changed(&[ChangedProp])` replaces `dispatch_prop_changed`; R1a's `__svelters_prop_changed` is gone.

### ABI: one additive member flag

`SVL_M_PROPORTIONAL = 128` on prop descriptors (Rust `svelters_abi::SVL_M_PROPORTIONAL`, C header). The host must
know a prop is proportional to skip the read, and the API R2 codes against (`observeProp(h, prop, fn)`) carries no
cost hint, so the backend says it in the descriptor, from its facts. Default 0 (cheap): backends that do not set it
keep working. `observe`/`prop_changed` are unchanged.

### Log backend, TestHost, op log

- Scripts: `{"changed": [h, prop, value?]}`. The value becomes the native value; the log line is
  `changed #h prop <json>`.
- The TestHost implements the same compare (baseline, read-back after `setProp`, deep equality) and calls
  `__svelters_changed([h, prop, value])`.
- `tools/oplog-compare.mjs` parses the value.

### JS

- `packages/native-runtime/src/observe.js` (exported from `index.js`: `observeProp`, `trackProp`, `trackLocal`,
  `invalidateProp`). It is built on a port of Svelte's `createSubscriber` in the core adapter (`core/svelte.js`
  `subscriber`), because Node resolves `svelte/reactivity` to the server no-op without the `browser` condition.
- The table is keyed by the host object, then the handle number, then the canonical prop name (dashes and
  underscores removed, lowercased). It never holds a wrapper.
- `@svelters/types` core gains:
  - `installTracker`, `observed`, `observedLocal`, `observedSignal`, `invalidate`, `untracked`;
  - `NativeObject.peek(prop)`;
  - `setProp` now invalidates locally.
- Generated getters, by class:

  | Class | GTK (`gen_types.py`) | Qt (`gen_types_js.py`) |
  |---|---|---|
  | exact, partial | `observed` | `observed` |
  | set_only | `observedLocal` | `observedLocal` |
  | constant, none, no fact | `getProp` | `getProp` |

  Counts: GTK: 413 `observed`, 0 `observedLocal`. Qt: 60 `observed`, 28 `observedLocal`.

  Typings get `@remarks` for none and partial props, and for proportional ones. The docs pages of the curated
  types mark each property as reactive, reactive on writes or not reactive. There is a new concepts page,
  `docs/site/src/content/docs/concepts/reactivity.md`.
- Qt `QTextDocument.plainText` has no Q_PROPERTY: `toPlainText()` re-read after `contentsChanged` (`observedSignal`,
  table `SIGNAL_ACCESSORS` in `gen_types_js.py`). This is what makes the §7 example mirror-free on Qt before R3.

### GTK backend

(see "GTK lane" below)

## Acceptance

- NON-GUI-TYPES §7 without a `$state` mirror: `fixtures/gtk-types` (`buffer.text.length`, script step
  `{"changed": [1, "text", "Hello, world"]}`) and `fixtures/qt-types` (`doc.plainText.length`).

(remaining sections filled at completion)
