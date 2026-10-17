# Fork bindings: element `bind:this` and a renderer binding hook (phase A)

Status: phase A done (fork compiler and runtime). Phase B (our renderer, binding metadata, generated `.d.ts`) waits for P5.
Fork: `/projects/svelte-fork`, branch `svelters/renderer`, base `f3de3f2` (stack head of #18042/#18405/#18461/#18511). Local commit(s): see `vendor/svelte/PIN`. Nothing was pushed.

## 1. Why the PR forbade element `bind:`

- Commit `9167c8df4` "fix: disallow `bind` and `transitions`" (paoloricciuti, 2026-03-31) added `e.incompatible_with_custom_renderer(node, '`bind:`')` at the top of the element branch of `BindDirective.js` in the analyzer, plus the test samples `bind-directive` and `bind-component`.
- The PR body (#18042, "Limits") gives the one reason: "`bind:` on regular elements is forbidden, since svelte register known DOM events to keep the variables in sync." That is a statement about `bind:value`/`checked`/`group`/media/size bindings, which are all `addEventListener` + DOM property reads (`bind_property`, `bind_value`, `ResizeObserver`...).
- It was not specific to `bind:this`. `bind_this` (`internal/client/dom/elements/bindings/this.js`) takes the node as an opaque value and only uses effects, `component_context` and the `DESTROYING` flag; there is no DOM access. The compiler path (`build_bind_this`) is also DOM-free. So `bind:this` was swept in by the blanket check.
- No review comment, issue comment or later commit on #18042 discusses `bind` (`gh pr view --comments`, `gh api pulls/18042/comments`, searched for "bind"; empty).
- The side effect we also hit: under the check, unknown names such as `bind:text` would have reached `bind_invalid_name` (the DOM `binding_properties` table), so merely deleting the error is not enough; the DOM validation has to be skipped under a renderer.

## 2. Design

### 2.1 `bind:this`

Reuses `$.bind_this` unchanged. The only compiler change is to stop rejecting it. It cooperates with:

- `{#if}` teardown: `bind_this` nulls the variable when the effect is destroyed (test `bind-this`).
- keyed and unkeyed `{#each}`: moves keep following the key; removal nulls the entry (test `bind-this-each`).
- `<svelte:element bind:this>` including a tag change (test `bind-this-svelte-element`).
- HMR: with `compileOptions.hmr` the component compiles and binds (test `bind-this-hmr`). HMR swaps components by destroying the old effect tree, which runs the same `bind_this` teardown; I did not simulate a hot update in the test harness (no `import.meta.hot`), so the swap itself is covered only by that shared teardown path.
- The value is the renderer's own element node. Per `ownership-and-lifetimes.md` it can outlive the native object; staleness is the renderer's concern (reads on a dead node throw `StaleHandleError`), Svelte only nulls it on teardown.

### 2.2 Binding hook

Options considered:

| | A. `renderer.bind(el, name, get, set) -> {update, destroy}` (chosen) | B. compiler routes to `setAttribute` + `listen` with `renderer.bindings[name]` |
| --- | --- | --- |
| Compiler diff | one `$.bind_renderer(node, name, get, set)` call | needs the change event name at compile time, but the renderer is only known at runtime (`customRenderer` is a module path), so it would need a runtime lookup anyway |
| Echo suppression | renderer owns it (block our handler id, a flag, or compare) and `update` is always synchronous and never nested in `set` | compiler/runtime cannot know the native strategy |
| Normalization / clamped read-back | `set(v)` from `update` is allowed; the runtime also re-checks `get()` after a native `set` (function bindings that reject or transform the value) | needs a second, separate read-back mechanism |
| Per-kind errors | renderer throws from `bind` with the element kind | would need another hook |
| Read-only | renderer just never returns `update` | needs a flag |

Chosen API (in `Renderer`, optional):

```ts
bind?(element: TElement, name: string, get: () => any, set: (value: any) => void): RendererBinding | void;
type RendererBinding = { update?(value: any): void; destroy?(): void };
```

Runtime (`bind_renderer`, `internal/client/dom/elements/bindings/renderer.js`, 55 lines):

1. Calls `current_renderer.bind(el, name, get', set')`. No `bind` method: runtime error `bind_unsupported_by_renderer` (client error; message names the binding). A renderer without the hook therefore behaves as before for `bind:`, only the error moved from compile time to runtime because the renderer module is not known to the compiler.
2. `get'` is an untracked read of the state value (for adopting the native value at init, as `bind_value` does with `untrack(get) == null`).
3. `set'(v)` runs the user setter without reactive context (the same as the DOM listeners, so it is safe from a native signal that fires inside an effect) and then, after `tick()`, compares `get()` to `v`; if they differ (a function binding setter that rejected or normalized, as in DOM `bind_value`'s "respect any validation in accessors"), it calls `update(get())` so the native side converges.
4. If the binding has `update`, a render effect calls `update(get())` initially and on every state change, with the call untracked. It is always synchronous inside the effect, never nested in `set'`, so a renderer that blocks its own handler during `update` is race-free.
5. If the binding has `destroy`, it runs in a teardown (component or branch destroy). A pending read-back after destroy is dropped.

How the requirements land:

- (a) echo suppression: renderer-side by design. Svelte only guarantees (4) and that `set` with an equal value is a no-op (signals are equality-checked).
- (b) clamped read-back: `update(v)` may call `set(clamped)`; the runtime read-back covers setter-side normalization. Tested with `bind-value-clamp` and `bind-value-function`.
- (c) unsupported prop: the renderer throws from `bind` with the kind, e.g. `bind:value is not supported on <label>` (`bind-unsupported-kind`).
- (d) function bindings: compile unchanged, the compiler already hands `get`/`set` over (`bind-value-function`).
- (e) dimensions: free; the renderer never returns `update` and only calls `set` (`bind-readonly`).

Compiler:

- Analyzer (`2-analyze/visitors/BindDirective.js`): when `customRenderer` is a renderer module, the DOM `binding_properties` validation is skipped. Still errors (`incompatible_with_custom_renderer`): `bind:group` (needs DOM binding groups) and bindings on `<svelte:window|document|body>` (those elements are already rejected). `bind:files`, media, size, contenteditable bindings are accepted: the hook covers them, the renderer decides. A component with `customRenderer={null}` keeps all DOM validation.
- Transform (`3-transform/client/visitors/BindDirective.js`): any `bind:<name>` except `this` becomes `$.bind_renderer(node, 'name', get, set)`. `set` is omitted when it equals `get`, as for the DOM bindings (a bindable prop is both).
- `RegularElement.js`: the `input`/`textarea` special cases (`$.remove_input_defaults`, `$.remove_textarea_child`) are now skipped under a renderer so that a native kind that happens to be called `input` is not treated as an HTML input.
- Messages: `bind_unsupported_by_renderer` in `messages/client-errors/errors.md` (generated files regenerated with `node scripts/process-messages`).

### 2.3 Not covered, on purpose

- Hydration: custom renderer components are not hydrated, nothing to do.
- SSR: `bind:` on elements is not special under SSR for a native tag; unchanged. (The server build throws `custom_renderer_unavailable_on_server` anyway.)
- `bind:group`: needs a design; our plan is a wrapper component (see survey note §4.1).
- Reading the element's current value on demand (`el.value`) stays a `bind:this` + native call concern.

## 3. Fork changes

See the PIN for SHAs. Files under `packages/svelte`:

| File | Change |
| --- | --- |
| `src/compiler/phases/2-analyze/visitors/BindDirective.js` | renderer branch replaces the blanket error |
| `src/compiler/phases/3-transform/client/visitors/BindDirective.js` | `$.bind_renderer` call |
| `src/compiler/phases/3-transform/client/visitors/RegularElement.js` | skip `input`/`textarea` DOM init under a renderer |
| `src/internal/client/dom/elements/bindings/renderer.js` | new runtime |
| `src/internal/client/index.js` | export |
| `src/internal/client/custom-renderer/types.d.ts` | `bind?` and `RendererBinding` |
| `messages/client-errors/errors.md`, generated `errors.js`, `documentation/docs/98-reference/.generated/client-errors.md` | new client error |
| `tests/custom-renderers/renderer.ts`, `shared.ts` | test renderer `bind`, `native_change` helper |
| `tests/custom-renderers/samples/bind-*` and `bind.test.ts` | tests |
| `.changeset/bright-nodes-bind.md` | changeset |

## 4. Tests

Results are in the final report and in `vendor/svelte/PIN`. Suites run: `tests/custom-renderers` (the PR's four files plus mine), then the whole `svelte` vitest project.

## 5. Upstream proposal (file-ready text for a comment on #18042; not posted)

> **Element `bind:this` and a `bind` hook for custom renderers**
>
> `bind:` on regular elements is currently a compile error under `customRenderer`, with the stated reason that Svelte's element bindings rely on DOM events. That holds for `bind:value`, `bind:checked` etc., but not for `bind:this`: `bind_this` only needs the node and the effect lifecycle, so it works as is (keyed `{#each}`, `{#if}` teardown, `<svelte:element>`).
>
> For the other bindings I'd suggest one optional renderer method instead of a closed list of names:
>
> ```ts
> bind?(element, name, get, set): { update?(value): void; destroy?(): void } | void
> ```
>
> The compiler emits `$.bind_renderer(node, 'name', get, set)` for every `bind:<name>` except `this` (and `group`, which stays an error). The renderer decides what the name means for its element: subscribe to the native change and call `set(value)`, apply state changes in `update(value)`, release in `destroy()`, and throw if the element does not support that binding. `update` runs from a render effect, never from inside `set`, so echo suppression stays a renderer detail (compare, or block its own handler). After a native `set`, the runtime re-reads `get()` and calls `update` if the setter did not take the value (the same "respect accessors" behaviour as `bind:value` on `<input>`). Read-only bindings (sizes, focus) just omit `update`. Function bindings need nothing extra.
>
> A renderer without `bind` gets a runtime error for any element `bind:` other than `this`; `customRenderer={null}` components keep all DOM validation. The diff is ~110 lines of source (analyzer: skip DOM validation under a renderer, transform: one call, one runtime file, types, one error message) plus tests with the object renderer from `tests/custom-renderers/renderer.ts`.
>
> Open questions: whether `bind:group` deserves a renderer-facing form, and whether the analyzer should let a renderer module declare its supported names (it cannot today, the module is not loaded at compile time).

## 6. Binding metadata requirements for phase B

### 6.1 What the renderer needs per `bind:<name>` and kind

For each bindable `name` on an element kind, `bind(el, name, get, set)` must know:

1. **Native property** it reads and writes (and the accessor pair, for dynamic invocation).
2. **Change signal(s)** and which of them mean user edit versus any change.
3. **Value type and normalization**: neutral type, nullability, numeric range, coercion on read (string to number), and what the native side does on write (clamp, validate, reject).
4. **Echo-suppression strategy**: block our handler id (GTK `g_signal_handler_block`), a per-handle suppress flag checked by our connector (Qt, never `blockSignals`), compare-before-set, or none needed (user-intent signal).
5. **Programmatic set emits sync signals?** Which ones (GTK `Switch::state-set`, Qt `valueChanged`), so the guard covers them and tests can assert them.
6. **Read-back**: after a write, read the property; if it differs, call `set(native)`.
7. **Lifetime** (`ownership-and-lifetimes.md`): what happens when the handle dies natively.
8. **Direction**: `two_way` or `read_only`.

### 6.2 Schema extension (`svelters-element-kind/v0`, optional field `bindings`)

Allowed under the change rules in `plans/architecture/widget-metadata.md` ("Lanes may add optional fields"). Every fact carries `origin`, like the rest of the schema.

```yaml
bindings:
  - name: text                   # the `bind:<name>` identifier exposed to Svelte
    direction: two_way           # two_way | read_only
    property: text               # a `properties[].name` of this kind (or `via: {getter, signal}` for non-properties)
    target: self                 # self | { borrowed: lineEdit }  (sub-widget the signal lives on; borrowed => OWNER death)
    type: { ref: string, nullable: false }
    change:                      # native -> state
      signals:
        - { signal: "notify::text", role: any_change }       # any_change | user_edit
      preferred: "notify::text"  # the one the binding listens to (user_edit wins when available)
      normalize_read: null       # e.g. to_number for text-valued numeric kinds
    write:                       # state -> native
      accessor: set_text         # from properties[].accessors
      normalize_write: null      # e.g. null -> ""
      natively_normalizes: false # true: the native side clamps/validates, so read back
      range_from: null           # properties[].range, or {adjustment: ...}
    echo:
      strategy: compare_and_block  # block_handler | flag | compare | compare_and_block | none (user_edit signal)
      compare_before_set: true     # GtkEntry re-notifies on identical text and moves the cursor
      read_back: false
      sync_signals_on_set: []      # native signals a programmatic set emits synchronously, also guarded
    lifetime:
      on_native_death: end_binding # end_binding | retain_state; never call set() after death
      on_stale_update: warn_noop   # update() on a dead handle: dev warning, no-op (teardown-shaped op)
      on_destroy: idempotent_disconnect
    unsupported_for: null          # reason string for the renderer-time error, "bind:text is not supported on <kind>"
    origin: manual                 # gir | runtime | metaobject | manual | probe
    verified_by: [echo_probe, clamp_probe]   # generated tests per fact (annotation loop)
```

Defaults a generator can derive from the existing schema, so the hand-written part stays small: `property`, `type`, `accessors`, `range` from `properties[]`; the candidate signals from `properties[].notify`; `sync_signals_on_set` from `signals[].emitted_by_setters` (`signals-and-slots-semantics.md` line ~913). Hand-annotated: `preferred`, `echo.strategy`, `natively_normalizes` confirmation, `lifetime`, `direction`.

Strategy rule: if a user-intent signal exists (`textEdited`) use it, `strategy: none`. Otherwise `compare_and_block`: compare before the set and block **our** handler id (GTK) or set a per-handle suppress flag (Qt). Qt `QSignalBlocker`/`blockSignals(true)` is not allowed, it would silence other observers (semantic graph, app connections).

### 6.3 Examples

Facts marked *probed* come from `signals-and-slots-semantics.md` §1.6 (`gtk_sync_probe.py`, `qt_sync_probe.py`); *docs* are from the toolkit docs and need a generated verification test before they are trusted.

```yaml
# ---------- GTK4 ----------
- id: "svelters-api:v1:type:gtk:Gtk:Entry"
  bindings:
    - name: text
      direction: two_way
      property: text
      type: { ref: string, nullable: false }
      change:
        signals: [{ signal: "notify::text", role: any_change }, { signal: changed, role: any_change }]   # Gtk.Editable
        preferred: "notify::text"
      write: { accessor: set_text, natively_normalizes: false }
      echo:
        strategy: compare_and_block         # probed: set_text re-emits notify::text on identical text and moves the cursor
        compare_before_set: true
        read_back: false
        sync_signals_on_set: [changed]
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual                          # provenance cites the probe (svmeta ORIGINS has no `probe`)

- id: "svelters-api:v1:type:gtk:Gtk:CheckButton"
  bindings:
    - name: checked
      direction: two_way
      property: active
      type: { ref: bool, nullable: false }
      change: { signals: [{ signal: "notify::active", role: any_change }, { signal: toggled, role: any_change }], preferred: "notify::active" }
      write: { accessor: set_active, natively_normalizes: false }
      echo:
        strategy: block_handler             # docs: set_active emits toggled and notify::active only when the value changes
        compare_before_set: true
        read_back: false
        sync_signals_on_set: [toggled]
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual                          # provenance cites the toolkit docs
      verified_by: [echo_probe]
    # A group of CheckButtons (radio) is not bind:checked on each; it is the RadioGroup wrapper (bind:group stays unsupported).

- id: "svelters-api:v1:type:gtk:Gtk:SpinButton"
  bindings:
    - name: value
      direction: two_way
      property: value
      type: { ref: f64, nullable: false }
      change: { signals: [{ signal: value-changed, role: any_change }, { signal: "notify::value", role: any_change }], preferred: value-changed }
      write:
        accessor: set_value
        natively_normalizes: true           # probed: set_value(500) with max 99 -> value-changed 99
        range_from: { adjustment: [lower, upper, step_increment] }   # value is held in the GtkAdjustment
      echo:
        strategy: compare_and_block
        compare_before_set: true
        read_back: true                     # probed: clamped value is emitted; still read back, never rely on the echo
        sync_signals_on_set: [value-changed]
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual                          # provenance cites the probe (svmeta ORIGINS has no `probe`)
    - name: width
      direction: read_only                  # GTK4 has no size-allocate; needs a host pseudo-signal (survey §4.4)
      property: { via: { getter: get_width, signal: "host:resized" } }
      type: { ref: i32, nullable: false }
      change: { signals: [{ signal: "host:resized", role: any_change }], preferred: "host:resized" }
      write: null
      echo: { strategy: none }
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual

# ---------- Qt ----------
- id: "svelters-api:v1:type:qt:QLineEdit"
  bindings:
    - name: text
      direction: two_way
      property: text
      type: { ref: string, nullable: false }
      change:
        signals: [{ signal: "textEdited(QString)", role: user_edit }, { signal: "textChanged(QString)", role: any_change }]
        preferred: "textEdited(QString)"    # probed: textEdited never fires for setText. The P4 mapping (js `changed`) uses textChanged + suppress_signals; switching to textEdited would need no suppression
      write: { accessor: setText, natively_normalizes: false }   # a validator/inputMask can still reject: set read_back true when present
      echo:
        strategy: none                      # with textEdited; the implemented P4 annotation says host_suppressed (textChanged)
        compare_before_set: true            # probed: unchanged setText does not emit textChanged
        read_back: true                     # validator/inputMask/maxLength may alter the text
        sync_signals_on_set: ["textChanged(QString)"]   # only matters if textChanged is bound instead
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }   # Qt: destroyed() drops our connection
      origin: manual                          # provenance cites the probe (svmeta ORIGINS has no `probe`)

- id: "svelters-api:v1:type:qt:QCheckBox"
  bindings:
    - name: checked
      direction: two_way
      property: checked
      type: { ref: bool, nullable: false }
      change: { signals: [{ signal: "toggled(bool)", role: any_change }, { signal: "clicked(bool)", role: user_edit }], preferred: "clicked(bool)" }
      write: { accessor: setChecked, natively_normalizes: false }   # tristate: checkState/stateChanged is a separate binding
      echo:
        strategy: none                      # docs: setChecked emits toggled, not clicked
        compare_before_set: true
        read_back: false
        sync_signals_on_set: ["toggled(bool)", "stateChanged(int)"]
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual                          # provenance cites the toolkit docs
      verified_by: [echo_probe]

- id: "svelters-api:v1:type:qt:QSpinBox"
  bindings:
    - name: value
      direction: two_way
      property: value
      type: { ref: i32, nullable: false }
      change: { signals: [{ signal: "valueChanged(int)", role: any_change }], preferred: "valueChanged(int)" }   # Qt6: only the int overload; "textChanged" is the string form
      write: { accessor: setValue, natively_normalizes: true, range_from: { properties: [minimum, maximum] } }
      echo:
        strategy: flag                      # per-handle suppress flag; never QSignalBlocker
        compare_before_set: true
        read_back: true                     # probed: setValue(500) with max 99 -> valueChanged [5, 99]
        sync_signals_on_set: ["valueChanged(int)"]
      lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
      origin: manual                          # provenance cites the probe (svmeta ORIGINS has no `probe`)
```

### 6.4 Rules the renderer applies (from §1.6 of the signals note, mapped onto the hook)

- `bind()` connects the preferred signal through the same connector that carries `listen`, with the handler id stored on the binding record; `update` and `destroy` use that record.
- `update(v)`: if `compare_before_set` and the native value equals `v`, return. Else set the suppress flag (or block the handler id), call the accessor inside the existing "applying ops" guard so `sync_signals_on_set` are suppressed too, clear the flag, then if `read_back` and the native value differs from `v`, call `set(native)` once. Do not rely on the echo.
- The native handler: read the property, apply `normalize_read`, call `set(value)`. `set` is safe from any signal context (the runtime wraps it with no reactive context). Handlers run at a host checkpoint like every other event, so sync-scope rules from the signals note apply.
- Missing support: `bind()` throws `bind:<name> is not supported on <kind>` (kind = the metadata kind name) when the kind has no `bindings` entry for `name`. This is the (c) error.
- Lifetime: when the handle dies natively (`on_destroyed(h, reason)`), mark the binding record dead and drop the subscription bookkeeping; never call `set` afterwards; `update` on a dead node is a dev warning and a no-op (it can legitimately run during teardown); `destroy` is idempotent and must not touch the dead native object (GTK: no `g_signal_handler_disconnect` on a finalized widget; Qt: the connection died with `destroyed`). A re-realized node (the closed-window reopen case) re-runs `bind` from the node's recorded binding list, which is why the renderer keeps `{name, get, set}` per node. Borrowed targets (`target: { borrowed: lineEdit }`, GtkEntry's inner `GtkText`) die with their owner (reason `OWNER`) and must be re-fetched, not cached.
- Read-only (`direction: read_only`): `bind` returns `{ destroy }` only; the first value is pushed by calling `set` once after the first layout pass (sizes are not known in `onMount`, survey §4.4), not at `bind` time.

## 7. What phase B must do

1. Merge the new tarball (done in this branch) into the renderer package after P5 merges; do not rebase the renderer lane onto an unmerged PIN.
2. Implement `bind(el, name, get, set)` in `packages/renderer`: look up the kind's `bindings` entry (§6.2), connect through the `listen` machinery, implement `update` per §6.4, throw the per-kind error, track a record per node for destroy, stale handles and re-realization.
3. Add `bindings` to the schema section in `plans/architecture/widget-metadata.md` as an optional field; extend `tools/metadata/common/svmeta.py` validation; add overrides for GTK Entry/CheckButton/SpinButton/Switch/Scale and Qt QLineEdit/QCheckBox/QSpinBox/QSlider first.
4. Generated verification tests per binding fact (annotation loop): echo (no extra native write, no handler call from our own set), clamped read-back, sync signals suppressed, native death mid-binding, user-edit signal not emitted by a programmatic set.
5. Generate `.d.ts` so `bind:text`/`bind:value`/`bind:checked` type-check per kind, and `bind:this` gives the typed handle class.
6. Conformance: add `bind:this` and one `bind:value` (op-log fixtures) to `tests/renderer-conformance` and the host-level fixtures; check dev, prod, values2 and hmr bundles. The shim backend (`SVELTERS_BACKEND=shim`) has no `bind` hook, so it either gets one in `@svelters/dom` or those fixtures are renderer-only.
7. Keep `bind:group` on wrapper components (survey §4.1) and revisit after the first release; dimension bindings need the host `resized` pseudo-signal.
8. Decide what `update` does in the egui backend (value round-trips every frame, no change signal): `bind` there would poll in the frame callback; out of scope until M7.

## 8. Phase B: what was built

Done in the `fork-bindings` branch after P5 merged (`eca1f3a`), on the new tarball.

- **`bind:this`** on a native element gives the renderer's node. New on the node: `call(method, ...args)`, `getProp(name)`, `handle`, `dead`, and `StaleHandleError` (exported). A node that was never realized throws a plain `Error` ("no native widget yet"); a destroyed one throws `StaleHandleError` with `reason` `"destroyed"`; a re-inserted dead node is realized again and works. The native-death reasons of ABI v2 (`__svelters_destroyed(h, reason)`) are not delivered to JS yet (M2a), so `dead` only ever becomes `"destroyed"` today; setting `el.dead = reason` there is the whole hook-up.
- **`bind` hook** in `packages/renderer/src/index.js`, driven by `src/bindings.generated.js`. Only `entry.text` exists (both toolkits). The renderer registers the change signal through its normal listener bookkeeping (one native `listen` however many handlers), compares before set against `el.props` (also updated by natively made edits), implements `readBack` (tested with a fake kind, no real kind needs it yet) and drops its handler on `destroy`. Updates for a not-yet-realized or dead node touch no host op and are replayed on realize.
- **Echo** is done natively: the GTK backend blocks `changed` and `notify::text` and the Qt backend uses the `suppress_signals` flag during a programmatic `setProp`. `bind-value` shows it end to end on both toolkits: the count of native `changed` signals that reach JS is the same with and without the app writing to the entry.
- **Metadata**: `bindings:` in `metadata/overrides/gtk/Entry.yaml` (merged into `metadata/gtk/Entry.json` by `ingest.py` as an additive key, no generated Rust changed) and in the `entry` element of `metadata/overrides/qt/QLineEdit.yaml` (ignored by `gen_cpp.py`). `tools/metadata/common/gen_bindings.py` (separate file, `--check` for CI, tests in `test_gen_bindings.py`) validates each binding against the facts it cites (property exists and is writable, the change signal exists, `host_suppressed` is backed by `echo_suppress` or `suppress_signals`, both toolkits agree) and emits `bindings.generated.js` and `types/bindings.d.ts`. The existing generators were not edited.
- **Types**: `BindableProps` feeds `bind:<name>` attributes on the intrinsic elements (a wrongly typed `bind:text` fails `svelte-check`); `NativeElement` types the `bind:this` value. `packages/renderer/check/svelte.config.js` gives svelte-check the same `customRenderer` option the build uses.
- **Window title**: no binding. Nothing edits it natively (the user cannot type into a title; the window manager does not write it back), so there is nothing to read back. `title` stays a one-way prop.
- **Fixtures**: `bind-this` (reads the native text typed into an entry, calls `present` on the window, probes a stale node) and `bind-value`, with integration tests on GTK and Qt. Both are renderer-only (`SVELTERS_BACKEND=shim` skips them).
- Not done: read-only bindings (`bind:width`): they need the host `resized` pseudo-signal; the generator and renderer already accept `direction: read_only`, there is just no kind annotated.
