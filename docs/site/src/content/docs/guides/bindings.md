---
title: Bindings
description: Element bind:this and bind:<prop> on native widgets, where the binding facts come from, and how echoes of the app's own writes are suppressed.
---

Element `bind:this` and `bind:<prop>` work. The compiler resolves each binding against the toolkit's semantic spec at compile time: which property, which change signal, where the new value comes from and how echoes are suppressed. Component bindings (`$bindable`) work as in Svelte.

| Form | Status |
| --- | --- |
| element `bind:this` | works |
| element `bind:<prop>` with a binding plan in the spec | works |
| element `bind:<prop>` without one | compile error (`bind_no_plan`, `unknown_property` or `read_only_property`) |
| function bindings `bind:v={get, set}` | works, on elements and components |
| component `bind:prop`, `bind:this` | works |
| `bind:group`, `bind:clientWidth` and other DOM bindings | compile error: they are not native properties |

## `bind:<prop>`

```svelte
<script>
  let name = $state('Ann');
</script>

<window title="bind:text">
  <box orientation="vertical" spacing="6">
    <entry bind:text={name} />
    <button onclicked={() => (name = 'Bob')}>Set Bob</button>
    <label>Hello {name}</label>
  </box>
</window>
```

The binding name is the property's kebab-case name: `bind:text` on a GTK `entry` or a Qt `line-edit`, `bind:checked` on an `action` or `check-box`, `bind:value` on a Qt `slider`, `bind:current-index` on a Qt `tab-widget`. The element typings list every bindable property per tag (`BindableProps` in `@svelters/native-runtime/types`).

The compiler emits one runtime call per binding:

- A render effect writes the bound value to the property. A write equal to the last value the node holds is skipped.
- On the change signal, the new value is taken from the signal's arguments (or read with `getProp` when the signal carries none). It is recorded as the node's last value, so writing it back is not echoed, and it is assigned to your state only if it differs.

Writing the same property explicitly and through a binding (`<entry text="x" bind:text={t}>`) is a compile error (`bind_and_prop`). A one-way prop that has a binding in the spec (`text={draft}` on an entry, without `bind:`) still records native edits when its signal is delivered, so writing the edited value back is not echoed either.

Operations from the `bind-value` fixture, which is this example plus an "Upper" button and a change counter (abridged; `#3` is the entry, `#4` the "Set Bob" button, `#6` the greeting):

```text
create entry #3 ; set #3 text "Ann" ; listen #3 changed on
event #3 changed ["Zed"]  -> set #6 label "Hello Zed"     (no echo `set #3 text`)
event #4 clicked []       -> set #3 text "Bob" ; set #6 label "Hello Bob"
```

## Where binding facts come from

Bindings are annotations, not code. `bindings:` blocks in `metadata/overrides/<toolkit>/*.yaml` name the property, its direction (`two_way` or `read_only`), the change signal, the value source (`argN` or a read-back) and the echo strategy. They are checked against the toolkit metadata (the property exists and is writable, the signal exists, the argument has the property's type) and generated into:

- the semantic specs (`metadata/semantics/<toolkit>.json`, `kinds.<id>.bindings`), which the compiler reads;
- `packages/native-runtime/src/bindings.generated.js` and `packages/native-runtime/types/bindings.d.ts`.

The GTK spec has 237 bindings, many of them `notify::<prop>` on GTK properties; the Qt spec has 34, on the annotated elements. See [Metadata and annotations](/guides/annotation-workflow/).

## Echo suppression and notify signals

Setting a bound property from JS must not come back as a user change. Two mechanisms apply:

- **Compare.** The runtime skips a write equal to the node's last value and records natively reported values, as above.
- **Backend blocking** (`echo: host_suppressed` in the annotation, `backend_blocks` in the spec). The backend blocks its own handler while it applies a programmatic set, so the toolkit's notification does not reach JS. GTK blocks our handler id; Qt uses a per-handle flag, never `blockSignals(true)`, which would silence the app's other connections and accessibility observers. `echo_suppress[]` on a GTK property and `suppress_signals` on a Qt element property record these facts.

Where the change signal comes from:

| Toolkit | Notify source |
| --- | --- |
| GTK | `notify::<prop>` (`ParamSpec` with `EXPLICIT_NOTIFY`), or a user-edit signal such as `entry` `changed` or `check-button` `toggled` |
| Qt | the `NOTIFY` signal of the `Q_PROPERTY` (`text-changed`, `value-changed`, `current-index-changed`) |

Probed behaviour that drives the annotations:

```text
GTK Entry  set_text x, x, (blocked) y, y  -> notify::text ['x', 'x', 'y']   re-emits on identical set
GTK SpinButton set_value 5,5,500 (max 99) -> value-changed 5, 99            clamped value is emitted
GTK Switch set_active(True)               -> state-set True, then notify::active
Qt QLineEdit setText x, (blocked) y, y    -> textChanged ['x']               unchanged set does not emit
Qt QSpinBox  setValue 5,5,500 (max 99)    -> valueChanged [5, 99]
```

:::note[Not applied by the runtime]
The annotations also record `read_back` (read the property after a set because the toolkit may clamp or normalise it) and `compare_before_set` against the native value. The runtime compares only against the last value it wrote or saw; it does not read back after a set. A clamped value reaches JS only if the toolkit emits the change signal for it.
:::

## `bind:this`

```svelte
<script>
  let win = $state();
  let entry = $state();
</script>

<window title="bind:this" bind:this={win}>
  <entry bind:this={entry} text="seed" />
  <button onclicked={() => { console.log(entry.getProp('text')); win.call('present'); }}>Read</button>
</window>
```

The bound value is the runtime's node (`NativeElement` in the typings):

- `call(method, ...args)` calls a native method, `getProp(name)` reads a property. An object result is a node: the bound node for an object the app created, else one stable borrowed node per native object.
- `tag`, `handle` and `dead` describe the node.
- After the native widget is destroyed, `call` and `getProp` throw `StaleHandleError` with a `reason` (`destroyed` or the native death reason, such as `parent`). See [Ownership](/concepts/ownership/#abi-v2-and-implementation-state).

`bind:this` on a toplevel with `show="none"` is how an app opens a dialog itself: `about.call('present')`. `bind:this` on a component binds its exports, as in Svelte.

Sources: `docs/notes/native-compiler/runtime-contract.md` (`bind`, `track`), `docs/notes/native-compiler/semantic-spec.md`, `docs/notes/fork-bindings.md` (section 6, binding schema), `docs/notes/signals-and-slots-semantics.md` (section 1.6), `packages/native-runtime/src/index.js`, `packages/native-runtime/types/native.d.ts`, `fixtures/bind-value/`, `fixtures/bind-this/`, `fixtures/dialogs/`.
