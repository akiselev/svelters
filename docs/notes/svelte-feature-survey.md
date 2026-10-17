# Svelte 5 features on native widgets: a survey

Date: 2026-10-17. Svelte is the vendored fork (`vendor/svelte/PIN`: 5.57.2, `custom-condition` @ `f3de3f2`, the #18042 stack). The renderer is `packages/renderer` (P1b). Background: `custom-renderer-trial.md`, `signals-and-slots-semantics.md` ("slots note" below), `p1-js-lane.md`, `gtk-v1-archaeology.md`.

## 0. How to read this

Every "compiles / error" claim below comes from compiling with the vendored fork and `experimental.customRenderer` set. Every op log comes from running a bundle under Node, either against the production renderer + `@svelters/testhost` (these logs show **host ops**: what GTK/Qt would receive) or against `tests/renderer-conformance/renderer.mjs` (these show **renderer calls**, including JS-only fragments, comments and text).

Experiments live in `$SURVEY` = `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/survey/`:

| File | What |
|---|---|
| `build.mjs` | esbuild + fork compiler; `prod` = `packages/renderer`, `ext` = `renderer-ext.mjs`, `log` = conformance logging renderer; handles `.svelte.js` modules |
| `compile-matrix.mjs` | ~60 features compiled under the renderer; output is the table in each section |
| `proxy.mjs` | `$state` proxies arriving at `setAttribute` (`out/proxy.txt`) |
| `list.mjs`, `list-log.mjs`, `list-time.mjs` | `<List>` with a row snippet, 3 and 1000 items (`out/list-*.txt`) |
| `renderer-ext.mjs` | `packages/renderer/src/index.js` + prototypes: `adopt(h)`, `callNative()`, listen flags |
| `virtual.mjs`, `virtual-run.mjs`, `apps/ListView.svelte`, `apps/RowHost.svelte` | virtualized list prototype (`out/virtual.txt`) |
| `table-run.mjs`, `apps/ColumnView.svelte`, `apps/Column.svelte` | column-view prototype (`out/table.txt`) |
| `life.mjs`, `misc.mjs`, `async.mjs`, `entry.mjs`, `mods.mjs` | lifecycle order; boundary/`svelte:element`/dynamic component/store; async; generated `Entry` wrapper; event modifiers |

Status words: **works** (works today on `packages/renderer`), **needs X**, **n/a** (meaningless for native UI), **forbidden** (a compile error under the renderer).

## 1. Reactivity

| Feature | Browser | Under the PR | Native | Status |
|---|---|---|---|---|
| `$state`, `$state.raw` | reactive variables; objects/arrays become deep proxies | unchanged | nothing native; drives `setProp` | works |
| `$derived`, `$derived.by` | computed values | unchanged | - | works |
| `$effect`, `$effect.pre`, `$effect.root`, `$effect.tracking` | side effects after/before DOM updates; manual roots | unchanged; `.pre` runs before the block's host ops, `$effect` after (`out/life.txt`) | effects run inside the JS job; host ops from the same flush are already issued | works |
| `$props`, `$bindable` | component inputs; two-way props | unchanged | the basis of every wrapper component | works |
| `$inspect`, `$inspect.trace` | dev logging | compiles; dev only, uses `console` | host must provide `console` in dev | works (dev) |
| `svelte/store` (`writable`, `$store`) | Svelte 4 stores | unchanged (needs the `custom-renderer` export condition) | - | works (`out/misc.txt`: `set label "store 2"`) |

**Proxies reach native props.** Measured with the logging renderer and `util.types.isProxy` (`out/proxy.txt`):

```text
<box model={obj} items={obj.items} raw={raw} snap={$state.snapshot(obj)} rows={obj.rows} />
[parent] model:PROXY items:PROXY raw:object snap:object rows:number
[child]  model:PROXY bound:PROXY          <- passed through a component prop, still the proxy
obj.rows++        -> set snap, set rows   <- model/items NOT re-set: same proxy identity
obj.items.push(3) -> set snap only
raw.rows = 99     -> nothing              <- $state.raw is not deep
raw = {...}       -> set raw
obj = {...}       -> set model, items, snap, rows (a new proxy)
```

Consequences:
1. A deeply reactive object or array passed as a prop **arrives as a `Proxy`**, also through components. A backend that serializes it (`JSON`, a structured copy into Rust) reads through the proxy, and reading inside a template effect subscribes to every field read. The production renderer currently throws for objects (`objects are not valid host prop values`), which is fine for P but not for model-like props.
2. **Deep mutations never reach `setAttribute`**, because the attribute effect only depends on the variable. A native prop that holds structured data (a combo box's string list, a chart series) must be passed as `$state.snapshot(x)` (re-set on any deep change, as a plain object) or `$state.raw` reassigned immutably, or handled by a model protocol (§5.2). Recommendation: the renderer calls `$state.snapshot` on object values before handing them to the host, and generated typings mark such props as "value, not live model".
3. Static boolean attributes arrive as `""` (`<spinner spinning />` → `set spinning ""`, `out/async.txt`), while `{expr}` booleans arrive as `true`/`false`. Metadata-driven coercion (`""` → `true` for `bool` props) is needed. This is the same family as `INT_PROPS` today.

## 2. Template

| Feature | Under the PR | Native translation | Status |
|---|---|---|---|
| Text `{expr}` | text nodes, JS-only | folded into `label` for `LABEL_KINDS` (`label`, `button`); text anywhere else is dropped | works for label/button; **needs metadata**: which kinds take text and into which prop (`GtkTextView` buffer, `QPlainTextEdit::plainText`), and an error or warning for stray text |
| Attributes / spread | `setAttribute(el, k, v)` with typed values; `{...rest}` uses `set_attributes` | `setProp` | works; **spread drops non-`on*` function props** (fork bug b) |
| `{#if}` / `{:else if}` / `{:else}` | 1 comment anchor per block, JS-only | remove + destroy the old branch, create + insert the new one | works |
| `{#each}` keyed / unkeyed / index / destructuring / `{:else}` | compiles; keyed moves are `insert` of a parented node | `remove` + `insert` (move), `set` for changed fields | works (`out/list-3.txt`); slow for big reorders, see §5.1 |
| `{#key}` | destroy and recreate the subtree | same | works |
| `{#await}` | pending/then/catch branches | same | works (trial) |
| `{@html}` | **forbidden** | n/a; a native "rich text" is a prop (`GtkLabel use-markup`, `QLabel` rich text) | n/a |
| `{@const}` | compiles | - | works |
| `{@debug}` | compiles; dev `console.log` + `debugger` | - | works (dev) |

Example of what a row costs (production renderer, `out/list-3.txt`). One `{#each}` row with a label and a button becomes:

```text
create box #3
create label #4 ; set #4 label "1. task 0" ; insert #3 #4
create button #5 ; set #5 sensitive true ; listen #5 clicked on ; set #5 label "Done" ; insert #3 #5
insert #2 #3 before=end            <- the row box goes in last (post-order insert)
```

## 3. Events

| Feature | Under the PR | Native | Status |
|---|---|---|---|
| `onclick={f}` / `onclicked={f}` / `onclose-request={f}` | `renderer.addEventListener(el, type, handler, {})`; no delegation; raw args | `listen(h, signal, true)`; `EVENT_TO_SIGNAL` maps `click`→`clicked`, `change`→`changed` | works |
| Modifiers (`\|once`, `\|capture`...) | `on:` is **forbidden**; `on<name>capture` passes `{capture: true}` | see §3a | needs flags, §3a |
| Component callback props (`onSave={...}`) | plain props | - | works |
| `createEventDispatcher` | compiles (deprecated) | dispatches to callback props | works, don't use |
| Native signal args / return values | handler gets the host args spread; return values come back in `dispatch().results`, **but only for inline arrows and identifiers** (§3a.1) | GTK sync signals, Qt event filters through the event object (slots note §1.5) | works; sync signals need P-later work |
| `svelte:window`, `svelte:document`, `svelte:body` | **forbidden** | replaced by native elements and an app module, below | needs app-level API |

**Replacing `svelte:window` / `svelte:document`.** Native apps have no single global event target, but every use case maps to something concrete:

| Browser idiom | Native replacement |
|---|---|
| `<svelte:window onkeydown>` global shortcut | GTK: `GtkShortcutController` with `scope=GTK_SHORTCUT_SCOPE_GLOBAL`/`MANAGED` on the window, or `gtk_application_set_accels_for_action`. Qt: `QShortcut` with `Qt::ApplicationShortcut`, or a `QAction` with a shortcut. Svelte: `<window><shortcut trigger="<Control>q" onactivated={quit} /></window>` (kind-routed child, slots note §2.4 point 3), or `<Action name="app.quit" accel="<Control>q" onactivate={quit} />` |
| `beforeunload` / window close | `<window onclose-request={(e) => dirty && e.preventDefault()}>` (sync signal, event-object result; Qt `QEvent::Close` filter) |
| `resize` | per-window `notify::default-width` (GTK), `QEvent::Resize` filter (Qt); see dimension bindings §4 |
| `online`, `visibilitychange`, theme changes | an app module, e.g. `import { app } from '@svelters/app'` exposing reactive `app.colorScheme`, `app.active`, `app.quit()`, `app.on('activate')`: `GtkApplication`/`AdwStyleManager`, `QGuiApplication` signals |

## 3a. Event modifiers and native signal semantics

Experiments: `$SURVEY/mod-compile.mjs`, `mod-compile2.mjs`, `ret.mjs` (compile checks), `mods.mjs` + `apps/Mods.svelte` + `events.mjs` + `renderer-ext.mjs` (runtime prototype, output in `out/mods.txt`). `$SURVEY` = `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/survey`.

### 3a.1 Current state (Svelte 5.57.2 and our fork, compiled)

**Legacy `on:event|modifier`.** Compiled without a renderer, each modifier becomes either a handler wrapper or an `addEventListener` option:

| Modifier | Compiled form (no renderer) | Kind |
|---|---|---|
| `preventDefault`, `stopPropagation`, `stopImmediatePropagation` | `$.event('click', el, $.preventDefault(fn))` etc. | handler wrapper; calls the DOM method on `args[0]` |
| `once`, `self`, `trusted` | `$.once(fn)`, `$.self(fn)`, `$.trusted(fn)` | handler wrapper |
| `capture` | `$.event('click', el, fn, true)` | option `capture` |
| `passive`, `nonpassive` | `$.event('click', el, fn, void 0, true/false)` | option `passive` |

- In runes mode `on:click` still compiles, with the warning `event_directive_deprecated`. So the premise that it "can't be mixed with runes" is not quite right. What is an error is mixing `on:click` and `onclick` **in one component**, even on different elements (`mixed_event_handler_syntaxes`).
- **Under `experimental.customRenderer`, every `on:` directive is a compile error**: `incompatible_with_custom_renderer: 'on:' is not compatible with 'customRenderer'`. This covers `on:clicked|once` too, in both legacy and runes components. **Option (a) is not available at all.**

**Svelte 5 attribute handlers under the renderer** (`renderer.addEventListener(target, type, handler, options)` as observed):

| Source | `type` | `options` |
|---|---|---|
| `onclicked={f}` | `clicked` | `{}` |
| `onclickcapture={f}` | `click` | `{capture: true}` |
| `onkey-pressedcapture={f}` | `key-pressed` | `{capture: true}` |
| `ontouchstart={f}` | `touchstart` | `{passive: true}` (Svelte's built-in passive list) |
| `on(node, 'notify::text', f, {after: true})` (from `svelte/events`, inside `{@attach}`) | `notify::text` | `{after: true}`, **passed verbatim** |

- So the fork **does pass `capture` to the renderer**. It strips a trailing `capture` from *any* event name. `onkeypressed_capture` becomes type `keypressed_` with capture on, and a native signal whose name ends in "capture" could not be listened to without a rename.
- `svelte/events` `on(el, type, fn, options)` goes through the same `create_event` → `renderer.addEventListener` path and forwards **any** options object unchanged. It also calls `removeEventListener` on cleanup. This is the only official channel for arbitrary per-listener metadata.
- **The renderer never sees the user's function.** It always receives Svelte's `target_handler` wrapper (`handler.name === 'target_handler'`). Properties tagged on the user function (`fn.__sv = {after: true}`) are invisible at listen time (verified: `handler.__sv=undefined`).
- **Return values are lost for many handler shapes** (new finding; it refines the trial's "return values propagated"). The compiler passes inline arrows and plain identifiers straight through, but wraps every other expression in `function (...$$args) { x?.apply(this, $$args); }` **without a `return`**:

  ```js
  $.event('inline', w, () => 1);                                       // returns 1
  $.event('ident',  w, k);                                             // returns k()
  $.event('prop',   w, function (...$$args) { $$props.h?.apply(this, $$args); });          // undefined
  $.event('call',   w, function (...$$args) { $.get(event_handler)?.apply(this, $$args); }); // undefined
  $.event('member', w, function (...$$args) { obj.f?.apply(this, $$args); });              // undefined
  ```

  A handler received as a **prop** (`onclose-request={onClose}` inside a generated wrapper component), a wrapped handler (`stop(fn)`) or a method (`obj.f`) cannot return a value to the host. Conversely, an inline arrow that happens to return something (`() => (todo.done = true)`, `() => log.push(x)`) **does** return it. `out/list-3.txt` shows `results:[true]`, which a naive "truthy return = handled" rule would turn into "stop the emission".
- **Svelte 5's documented replacement for modifiers is plain function wrappers** (`once(fn)`, `preventDefault(fn)` written by the user). They need no renderer support. Under our renderer they only affect the JS side.

### 3a.2 Native mapping: what each semantic needs, and when

| Semantic | GTK | Qt | Needed at |
|---|---|---|---|
| capture phase | `gtk_event_controller_set_propagation_phase(GTK_PHASE_CAPTURE)` on the controller we create for the listener | event filter installed on the target (it runs before `event()`); an ancestor filter for "capture from a parent" | **listen time** (separate native connection) |
| run after the default handler | `g_signal_connect_after` / `G_CONNECT_AFTER` | n/a for signals (queued anyway); for events, call the base `event()` first in the subclass override | **listen time** |
| sync (return-value signal) | closure marshalled synchronously into JS (`call_sync`) | event filter / virtual override calling JS synchronously | **listen time** (known from metadata, `delivery: sync`) |
| stop propagation / handled | return `GDK_EVENT_STOP` / `TRUE` | `ev->accept(); return true` | **dispatch time** (result of this call) |
| prevent default | `TRUE` from a `stop_on_true` signal; `g_signal_stop_emission_by_name` for run-last defaults | filter returns `true`, so the widget's own handler is skipped | **dispatch time** |
| once | disconnect after the first emission (`g_signal_handler_disconnect`) | `QObject::disconnect` / `Qt::SingleShotConnection` | either: JS-side `once(fn)` is enough; a native disconnect is an optimisation |

The split matters. **Listen-time** semantics must reach `host.listen`. **Dispatch-time** semantics are the result of one emission, and that is exactly what the slots note's event-object model already covers (§1.5: the host reads `defaultPrevented`, `cancelBubble`, `returnValue` and out-fields from the event object after dispatch).

### 3a.3 Options evaluated

| Option | Verdict | Why |
|---|---|---|
| (a) legacy `on:x\|mod` | **Impossible** | It is a compile error under `customRenderer`. Even without a renderer it is deprecated and can't be mixed with `onx` in one component. |
| (b) attribute-name suffixes parsed by the renderer | **Only for `capture`** | Svelte already parses `…capture` into `{capture: true}`, so `onkey-pressedcapture` works with no extra code. Other suffixes compile as part of the event name: `onclose-request.stop` → type `close-request.stop` (no warning), `onx--after` OK, `onx:stop` warns `attribute_illegal_colon`, while `\|` and `$` are `attribute_invalid_name`. The renderer could parse `.after`/`.sync`, but these names are stringly typed, invisible to TypeScript `on*` typings unless generated, and they collide with real signal details (`notify::text`). |
| (c) `@svelters/events` handler wrappers tagging the function | **Dispatch-time only** | Tags never reach `addEventListener` (the renderer sees `target_handler`), and wrapped handlers lose their return value (see above). They are fine for `prevent(fn)`, `stop(fn)` and `once(fn)`, which only touch the event object or JS state. |
| (d) `{@attach listen('key-pressed', fn, {phase: 'capture', after, sync})}` | **Works for listen-time flags** | It uses the official `svelte/events` `on()`, which forwards the options object verbatim, cleans up on detach, is typed, and is composable in wrapper components. It is more verbose, so it suits the rare cases (after, explicit capture on a parent, explicit sync). |

**Prototype** (`$SURVEY/renderer-ext.mjs`, a copy of `packages/renderer/src/index.js` with about 40 changed lines). `addEventListener` turns `options` into flags `{phase?: 'capture', after?: true, sync?: true, once?: true}` (`capture: true` → `phase: 'capture'`), counts listeners per **(signal, flags)**, and calls `host.listen(h, signal, on, flags)` with `flags` omitted when empty. `dispatch(h, signal, args, {phase, after})` runs only the listeners registered for that phase. Output of `apps/Mods.svelte`:

```svelte
<window onclose-request={prevent(() => log.push('close vetoed'))}>
  <entry
    onkey-pressedcapture={(e) => log.push('capture ' + e.keyval)}
    onkey-pressed={stop((e) => log.push('bubble ' + e.keyval))}
    {@attach after('notify::text', () => log.push('after notify'))}
    {@attach listen('activate', once(() => log.push('activate once')), { sync: true })} />
</window>
```

```text
create window #1
listen #1 close-request on
create entry #2
listen #2 key-pressed on flags={"phase":"capture"}
listen #2 key-pressed on
insert #1 #2 before=end slot=default
call #1 present []
listen #2 notify::text on flags={"after":true}        <- attachments run after insert
listen #2 activate on flags={"sync":true}
dispatch key-pressed {phase:capture} -> results=[1]    defaultPrevented=false cancelBubble=false
dispatch key-pressed (bubble)        -> results=[null] cancelBubble=true      <- stop(fn): flag set, return lost
dispatch close-request               -> defaultPrevented=true                  <- prevent(fn)
dispatch activate #1 / #2            -> 'activate once' logged once
```

**Proposed ABI addition (not applied):** `listen(h, signal, on, flags?)` with `flags = {phase?: 'capture'|'target'|'bubble', after?: bool, sync?: bool}`. Two listens with different flags are **separate native connections** (GTK: one controller per phase; Qt: filter vs override). `off` must carry the same flags. The host's dispatch back into JS must say which connection fired, i.e. `__svelters_dispatch(h, signal, args, {phase, after})`. `sync` is normally derived from metadata (`delivery: sync`), so the flag is only an override. `once` stays JS-side.

### 3a.4 Recommendation

1. **Capture:** use Svelte's own suffix, `onkey-pressedcapture={...}`. The renderer maps `options.capture` → `phase: 'capture'`. It costs nothing, matches DOM Svelte, and is typed if our generated `on*` typings include the `…capture` variants for input-controller signals.
2. **Stop / prevent / handled / values:** use the **event object**, as in signals-and-slots-semantics §1.5. Handlers call `e.preventDefault()` / `e.stopPropagation()` or set `e.returnValue`. Ship `prevent(fn)`, `stop(fn)` and `once(fn)` in `@svelters/events` as sugar: they only set those flags, so they survive the lost-return-value wrappers and async handlers. **Do not use return values as results by default.** The compiler drops them for props, calls and members, and inline arrows return accidental values. Keep `accept_return: false` as the default in the metadata, as the slots note already recommends.
3. **After / explicit sync / exotic phases:** use `{@attach listen(type, fn, {after: true})}` from `@svelters/events`, built on `svelte/events` `on()`.
4. **ABI:** add the optional `flags` to `listen`, plus a phase/after selector on dispatch. Until a backend needs it, the renderer can reject non-empty flags with a clear error.
5. **Upstream (optional):** ask the fork to `return` from the generated `function (...$$args) { x?.apply(this, $$args) }` wrapper when a custom renderer is active. That would make return values consistent, but the event-object model does not depend on it.

## 4. Bindings

Compiled under the renderer, every element binding is a compile error, `bind:` included: `bind:value`, `bind:checked` and `bind:group` (also on a real `<input>`), `bind:this`, `bind:clientWidth`. Component `bind:prop`, component `bind:this` and function bindings `bind:v={get, set}` compile and work.

### 4.1 Native inputs: generated wrapper components

Two-way binding is a **component** feature in Svelte 5 (`$bindable`), so each bindable native property gets a generated wrapper. Verified on the production renderer (`$SURVEY/apps/Entry.svelte`, `out/entry.txt`):

```svelte
<!-- generated: Entry.svelte (metadata: entry.text bindable, change signal `changed`) -->
<script lang="ts">
  import type { EntryProps } from './entry.gen';
  let { text = $bindable(''), onchanged, ...rest }: EntryProps = $props();
</script>
<entry {...rest} text={text} onchanged={(t) => { text = t; onchanged?.(t); }} />
```

```svelte
<!-- app -->
<Entry bind:text={name} placeholder="Name" onactivate={() => (submitted = name)} />
<label>Hello {name}</label>
```

```text
create entry #3 ; set #3 placeholder "Name" ; set #3 text "Ann" ; listen #3 activate on ; listen #3 changed on
event #3 changed ["Bob"]  -> set #4 label "Hello Bob"     (no echo `set #3 text`: the renderer mirrors entry.text)
JS name = "Cy"            -> set #3 text "Cy" ; set #4 label "Hello Cy"
```

- The wrapper forwards the user's `onchanged` explicitly, because a function in `...rest` is dropped by the spread bug unless its name starts with `on`. `on*` handlers inside a spread are attached as events, which is why `onactivate` worked.
- Echo suppression is hard-coded for `entry`/`changed` today. It has to become metadata-driven: per bindable prop, the change signal plus the slots note §1.6 rules (block our handler during sets, read back clamped values).
- `bind:checked` → `<CheckButton bind:active>` / `<Switch bind:active>` (GTK `notify::active`, Qt `toggled`). `bind:group` → a `RadioGroup`/`CheckGroup` wrapper component holding `$bindable` `value`/`values`, with items registering through context (GTK `GtkCheckButton:group`, Qt `QButtonGroup`). `bind:value` on a select → `<DropDown bind:selected>` (GTK `notify::selected`, Qt `currentIndexChanged`).
- egui has no widget state to echo, so the value round-trips every frame; WinUI `TwoWay` bindings and AppKit KVO fit the same wrapper.

### 4.2 `bind:this` → `{@attach}`

`<entry {@attach (node) => { ref = node; return () => (ref = null); }} />`. The attachment runs after the element is created and inserted (`out/life.txt`: `{@attach} node.h=3` after `insert`/`present`). What app code can do with `node` is the open question: P1 exposes no `getProp`/`call` to components. Prototype: `callNative(node, method, args)` in `renderer-ext.mjs`, used by the list prototype for `items-changed`. **Needs:** a typed handle API (`widget(node).grabFocus()`, generated per kind).

### 4.3 Component `bind:prop`

Works unchanged (the wrapper pattern above relies on it).

### 4.4 Dimension bindings

`bind:clientWidth` etc. are forbidden. Native sources:
- **GTK4 has no `size-allocate` signal any more.** Use `notify::default-width`/`height` on windows, `GtkDrawingArea::resize`, or a host-side hook: a tiny `GtkLayoutManager` or widget subclass that emits a `resized` pseudo-signal.
- **Qt:** a `QEvent::Resize` event filter.

Expose it as a pseudo-signal (`onresized={(w, h) => ...}`) and a generated wrapper `<Measure bind:width bind:height>`. Sizes are only known **after the toolkit's layout pass**, never in `onMount`. **Needs:** host pseudo-signal, low priority.

## 5. Composition

### 5.1 Snippets and `{@render}`: "slot props" in Svelte 5

Svelte 5 replaced `<slot>` with **snippets**: named, parameterised template fragments that are ordinary values. A component receives them as props and renders them with `{@render}`:

- Content between a component's tags that isn't a `{#snippet}` becomes the implicit `children` snippet: `let { children } = $props(); ... {@render children?.()}`.
- `{#snippet name(a, b)}...{/snippet}` written **directly inside a component's tags** becomes a prop called `name`.
- A snippet with parameters is the Svelte 4 "slot props" (`<slot item={x} let:item>`): the **child** calls it once per item with arguments, while the **parent** wrote the template and keeps its own scope (closures, state, handlers).

The one-definition-many-rows pattern, verified end to end on the production renderer (`$SURVEY/apps/List.svelte`, `ListApp.svelte`):

```svelte
<!-- List.svelte: owns iteration, knows nothing about what a row looks like -->
<script>
  let { items, row, empty, key = (x) => x.id } = $props();
</script>
<box orientation="vertical">
  {#each items as item, i (key(item))}
    {@render row(item, i)}
  {:else}
    {@render empty?.()}
  {/each}
</box>
```

```svelte
<!-- the parent: defines the row ONCE -->
<List items={todos}>
  {#snippet row(todo, i)}
    <box>
      <label>{i + 1}. {todo.title}</label>
      <button sensitive={!todo.done} onclick={() => (todo.done = true)}>Done</button>
    </box>
  {/snippet}
  {#snippet empty()}<label>Nothing to do</label>{/snippet}
</List>
```

How it works at runtime under the renderer:
- The compiler turns each `{#snippet}` into a function `(anchor, ...argGetters)`. Under a custom renderer it is wrapped in `renderer_snippet(renderer, fn)`, which throws `snippet_renderer_mismatch` if rendered under a different renderer.
- `{@render row(item, i)}` passes **getters**, not values. Inside the row, `todo.title` and `i` are reactive. When `todos[2].title` changes, only that row's label effect reruns: **one `set` op**, with no row recreation and no list diff.
- Each instantiation builds the template from scratch with `createElement`/`insert` (no cloning under the PR). Fragments, comments and anchors stay JS-only, and native widgets are created lazily when the row's fragment lands under the realized `<box>`.

Host ops (`out/list-3.txt`, `out/list-1000.txt`):

| Step | 3 items | 1000 items |
|---|---|---|
| mount | 36 ops | **10,006 ops**: 3,002 create, 3,002 set, 3,001 insert, 1,000 listen |
| `todos[1].done = true` | `set #8 sensitive false` | 1 op |
| rename item 2 | `set #10 label "3. renamed"` | 1 op |
| native `clicked` on row 0 | `set #5 sensitive false` | 1 op |
| insert at index 1 | 12 ops | 1,009 ops (3 create, 3 insert, **1,002 set**: every later row's `{i + 1}` label changes) |
| remove at 0 | 7 ops | 1,004 ops (same reason) |
| reverse | 6 ops (2 moves = remove + insert) | 2,998 ops (999 moves + 1,000 label sets) |
| clear (`{:else}`) | 15 ops | 4,003 ops |

Under the hood (logging renderer, `out/list-log.txt`), the 1000-row mount makes **33,018 renderer calls**: 5,003 fragments, 4,002 text nodes and 1,001 comments, all JS-only. Timing on Node with JIT (`list-time.mjs`): 1,000 rows mount in 0.76 s; 10,000 rows mount in 7.5 s, and **a reverse of 10,000 takes 86 s**. That is quadratic in `packages/renderer`: `nativeInsert` calls `flat(parent)` plus a linear scan, and `getNextSibling` uses `indexOf`. QuickJS will be several times slower again. Takeaways:
- Plain `{#each}` + snippet is right for **tens to a few hundred** rows (forms, settings, toolbars, small lists).
- Fix the renderer's O(n) sibling lookups (linked siblings and a per-parent native index). Work item #2.
- Showing the index (`{i + 1}`) in a row makes every insert/remove touch every following row. That is inherent and the same in the DOM.
- Above that size, use a virtualized native list (§5.2), where cost scales with visible rows.

### 5.2 Virtualized native lists: snippet per native row, on demand

Toolkits with virtualization ask for rows when they need them and recycle row widgets:

| Toolkit | Protocol | Recycling unit |
|---|---|---|
| GTK4 `GtkListView`/`GtkGridView` | `GListModel` (`n_items`, `get_item`, `items-changed`) + `GtkSignalListItemFactory`: `setup` → `bind` → `unbind` → `teardown`; `GtkListItem:position` changes on shifts | `GtkListItem` + its child widget |
| Qt Widgets | `QAbstractItemModel` + `QStyledItemDelegate` **paints** rows (no widgets). `setIndexWidget` is not virtualized (one persistent widget per index). Qt Widgets has **no** widget-recycling list view. | none built in |
| WinUI 3 `ItemsRepeater` | `IElementFactory.GetElement`/`RecycleElement`, `ElementPrepared`/`ElementClearing` | `UIElement` |
| AppKit `NSTableView` (view-based) | `tableView(_:viewFor:row:)` + `makeView(withIdentifier:)` reuse queue | `NSTableCellView` |
| egui | immediate mode: `ScrollArea::show_rows(range)` per frame | none (rows are cheap) |

All of them come down to the same four events on one list widget: **setup(slot, containerHandle)**, **bind(slot, position)**, **unbind(slot)**, **teardown(slot)**, plus "position changed" and a model-change notification going the other way. Options for instantiating a parameterised snippet per row:

| Option | Assessment |
|---|---|
| `createRawSnippet` | Compiles, but **throws at runtime** under a renderer (`invalid_snippet_in_custom_renderer`), and it is HTML-string based anyway. Dead end. |
| Portal: one component renders `{#each liveSlots as s (s.id)}<sv-portal target={s.h}>{@render row(s.item)}</sv-portal>{/each}` (the 2021 `listViewProxy` design) | Keeps rows in one Svelte tree (context and effects shared). But bind is a list diff of `liveSlots` plus a portal element kind in the renderer, and teardown order is coupled to the `{#each}`. Workable. |
| **`mount()` a tiny `RowHost` per native slot into an adopted host widget; recycle by rewriting a `$state` slot object** | Each slot is its own Svelte root, created on `setup`, re-pointed on `bind`, unmounted on `teardown`. It needs only two small renderer additions (`adopt(h)`, a native call). Context is carried with `mount(..., { context: getAllContexts() })`. **Prototyped; recommended.** |

**Prototype** (`$SURVEY/virtual.mjs`, `apps/RowHost.svelte`, `apps/ListView.svelte`, `virtual-run.mjs`, about 80 lines of JS/Svelte):

```svelte
<!-- RowHost.svelte: one per native row slot -->
<script>
  let { row, slot } = $props();              // slot = $state({ item, index })
</script>
{#if slot.item !== undefined}{@render row(slot.item, slot.index)}{/if}
```

```js
// virtual.mjs (core): setup/bind/teardown from the native factory
setup(slotId, h) {                               // host made a row container widget h
  const state = makeSlotState({ item: undefined, index: -1 });   // a $state object
  const app = mount(RowHost, { target: adopt(h), renderer: R, context,
                               props: { get row() { return getRow(); }, slot: state.proxy } });
  slots.set(slotId, { app, state });
},
bind(slotId, pos) { slots.get(slotId).state.set(getItems()[pos], pos); flushSync(); },
teardown(slotId)  { unmount(slots.get(slotId).app); slots.delete(slotId); },
```

```svelte
<!-- ListView.svelte (user-facing) -->
<listview {@attach (n) => (node = n)}
  onsetup={(slot, h) => pool.setup(slot, h)}   onbind={(slot, pos) => pool.bind(slot, pos)}
  onposition={(slot, pos) => pool.position(slot, pos)}
  onunbind={(slot) => pool.unbind(slot)}       onteardown={(slot) => pool.teardown(slot)} />
<!-- + an $effect that diffs item keys and calls callNative(node, 'items-changed', [pos, removed, added]) -->
```

App code looks exactly like the non-virtual `<List>`:

```svelte
<ListView items={todos}>        <!-- 1000 items -->
  {#snippet row(todo, i)}
    <box><ThemedLabel text={`${i + 1}. ${todo.title}`} />
         <check active={todo.done} ontoggled={() => (todo.done = !todo.done)} /></box>
  {/snippet}
</ListView>
```

Simulated GtkListView host, 1000 items, 21 visible rows (`out/virtual.txt`):

| Step | Host ops |
|---|---|
| mount | 10 (window, listview, 5 `listen`, `present`) |
| host: `setup` + `bind` 21 slots | 252 (12 per row: 4 create, 3 set, 3 insert, 1 listen, plus the host's own row container) |
| scroll by 5: slots 0–4 `unbind` → `bind` to rows 21–25 | **5** (`set label` only; `active` unchanged, so no op) |
| `todos[22].title = …` (visible) | 1 |
| `todos[500].title = …` (offscreen) | **0** |
| `toggled` on a row's check | 1 |
| insert at 0 | `call #2 items-changed [0,0,1]`, then the host sends `position` updates (20 label sets because the row shows `i + 1`) and recycles 1 slot |
| host shrinks the viewport: `teardown` 3 slots | 12 (3 remove + 9 destroy) |

Context crossed the separate mount: `ThemedLabel` read `getContext('theme')` → `set css-class "dim"`.

Design points the prototype surfaced:
1. **Renderer: `adopt(h)`.** A mount target wrapping a widget the host created. Children are realized and inserted into it, and the adopted widget itself is never created or destroyed by JS. In `renderer-ext.mjs` this is 12 lines plus `isAttached` treating adopted roots as attached (without that, a move inside a row would be swept as a destroy).
2. **Factory signals must apply their ops synchronously.** GTK emits `setup`/`bind` during the list's layout and expects the row to be filled when the handler returns, otherwise the row paints empty for a frame. The slots note's rule that "ops from a sync scope are queued to the checkpoint" needs an exception: a `delivery: factory` class whose handler may `flushSync()` and whose ops are applied immediately. GTK tolerates widget changes during bind (that is what `GtkBuilderListItemFactory` does).
3. **Model notifications go JS → host.** The JS array is the model. The host needs `n-items` plus `items-changed(pos, removed, added)` (GTK) / `beginInsertRows…` (Qt). The prototype diffs keys by common prefix and suffix. Item identity for GTK can be the key, so the host keeps a native snapshot of keys only (slots note category E: never call JS per `get_item`).
4. **Per-row isolation is good:** a throwing row can get its own boundary, unmounts are explicit, and rows don't diff against each other.
5. **Qt:** there is no native recycling view. Either (a) for text/icon rows, use `QAbstractItemModel` roles plus a delegate with no snippet per row (`<ListView items={x} text={(t) => t.title} icon={...}>`), or (b) for widget rows, a **host-side virtualizing container** (a `QAbstractScrollArea` subclass that lays out a pool of row widgets and emits the same setup/bind/unbind/teardown). (b) gives one JS protocol for GTK, Qt, WinUI `ItemsRepeater` and AppKit `NSTableView`. For egui, the host calls bind for the visible range each frame and keeps the row roots alive while visible.

### 5.3 Grids and tables

**Static grids** (`GtkGrid`, `QGridLayout`): per-child placement is attached props. Svelte surface: `<Grid><Cell row={0} col={1} colSpan={2}><Entry/></Cell></Grid>`, where `Cell` renders `<sv-item slot="cells" row=… column=…>`. **Needs** `sv-item` in the renderer and the `set_child_attr(parent, child, slot, attr, value)` op (slots note §2.4 point 7 and §2.5.7). In GTK a cell move is a `GtkGridLayoutChild` prop set; in Qt it is `removeWidget` + `addWidget`. Not implemented today.

**Data tables** (`GtkColumnView`, `QTableView`): a column is a native object with **its own factory** (`GtkColumnViewColumn:factory`, Qt `setItemDelegateForColumn`). So each `<Column>` is a row pool (§5.2) over a shared model, with the model coming from context. Prototyped (`$SURVEY/apps/ColumnView.svelte`, `Column.svelte`, `out/table.txt`):

```svelte
<ColumnView items={people}>
  <Column title="Name" expand>
    {#snippet cell(p)}<label>{p.name}</label>{/snippet}
  </Column>
  <Column title="Age">
    {#snippet cell(p)}<spinbutton value={p.age} onvalue-changed={(v) => (p.age = v)} />{/snippet}
  </Column>
</ColumnView>
```

```text
create columnview #2 ; set #2 n-items 3
create column #3 ; set #3 title "Name" ; set #3 expand true ; listen #3 setup/bind/teardown ; insert #2 #3
create column #4 ; set #4 title "Age"  ; ...                                                 ; insert #2 #4
host: setup+bind 3 rows x 2 columns -> 6 cell roots (create label/spinbutton, set, insert into host cell-box)
people[1].age++ -> set #12 value 43          (one op, in the right cell)
```

The column element is just a native child of `columnview` (GTK `append_column`; Qt header section plus delegate), and `ColumnView` publishes `items` through `setContext`. Sorting and filtering belong to the native model (`GtkSortListModel`) or to JS producing a sorted array. Header and footer snippets are `sv-slot`s on the column.

### 5.4 Named native slots

`sv-slot` is implemented in `packages/renderer`: `<sv-slot name="titlebar"><headerbar/></sv-slot>` routes its native descendants to slot `titlebar` of the nearest native ancestor, and the proxy never reaches the host. `data-slot="start"` on a native child works too. (`slot="…"` is rejected by the compiler on non-hyphenated parents.) The idiomatic surface is generated wrapper components with snippet props:

```svelte
<!-- generated Window.svelte -->
<script>let { titlebar, children, ...rest } = $props();</script>
<window {...rest}>
  {#if titlebar}<sv-slot name="titlebar">{@render titlebar()}</sv-slot>{/if}
  {@render children?.()}
</window>

<!-- app -->
<Window title="Notes">
  {#snippet titlebar()}<HeaderBar>{#snippet start()}<Button icon="list-add" />{/snippet}</HeaderBar>{/snippet}
  <Paned>{#snippet startChild()}<Sidebar />{/snippet}{#snippet endChild()}<Editor />{/snippet}</Paned>
</Window>
```

| Slot | GTK | Qt |
|---|---|---|
| window titlebar | `gtk_window_set_titlebar` | n/a (native frame), or `QMainWindow::setMenuWidget` |
| header start/end | `gtk_header_bar_pack_start/end` | `QToolBar` areas |
| paned start/end | `gtk_paned_set_start_child/end_child` | `QSplitter::insertWidget(0/1)` |

Status: routing **works**. Replace-default and restore semantics (slots note §2.4 point 4) and the generated wrappers are **needed**. A snippet declared directly inside a *native element* (not a component) compiles but renders nothing. That is a footgun worth a lint.

### 5.5 Context and boundaries

- `setContext`/`getContext`/`getAllContexts` (and `createContext`) **work**. Context flows into snippets rendered by children (it is the *rendering* component's context, i.e. lexical in the parent tree). A separately `mount`ed root (row pools) starts with **no** context unless `mount(…, { context: getAllContexts() })` passes it; the prototype does this. Uses: a list or table passing selection state and the model to rows and columns (§5.3), radio groups, form validation.
- `<svelte:boundary {failed}>` **works** with the snippet as an attribute (`out/misc.txt`: the child throws → `remove` the old label, `create label "error: boom"`). The **inline** `{#snippet failed()}` form crashes the compiler when `experimental.async` is off (fork bug a, `svelte-fork-upstream-bugs.md`). With `experimental.async: true`, inline `failed`/`pending` compile.

### 5.6 Dynamic components and `svelte:element`

- `<Comp />` with `Comp` in `$state` **works**: the swap is create new, insert before the next sibling, remove + destroy the old (`out/misc.txt`). `<svelte:component>` still compiles (deprecated).
- `<svelte:element this={kind}>` **compiles and works**, as a dynamic *native kind*: `this="switch"` → `create switch`, props replayed, `insert before=#4`, then `remove`/`destroy` of the old button. Useful for generated wrappers ("button or link button") and inspectors. Widget state (focus, scroll) is lost on a kind change, as expected.
- `<svelte:self>` compiles where allowed (recursive trees).

## 6. Lifecycle

Order on the production renderer (`out/life.txt`, ops applied directly by the host):

```text
A: script runs ; A: $effect.pre
create window #1 ; create box #2 ; create label #3 ; set #3 label "A" ; insert ... ; call #1 present []
mount() returned
A: {@attach} node.h=3 ; A: $effect ; A: onMount ; A: after tick()
-- show B: state change -> B: script ; B: $effect.pre ; create label #4 ; insert #2 #4 ; B: {@attach} ; B: $effect ; B: onMount ; flushSync() returned
-- hide B: remove #2 #4 ; B: attach cleanup ; B: onMount cleanup ; B: onDestroy ; destroy #4 (microtask sweep)
```

- **When `onMount`, `$effect` or `{@attach}` runs, the element's native widget exists and is inserted into a presented window.** Lazy creation happens on insert, and all ops of the flush come before effects. Whether GTK has *realized/mapped* it depends on the host: GTK4 maps children added to a mapped parent synchronously, but **allocation (size) only happens at the next frame**, so sizes read in `onMount` are 0. Qt is the same: `show()` is deferred to the event loop for geometry.
- If a host batches ops until the checkpoint instead of applying them directly, `onMount` would run *before* the native ops land. Keep the P contract (ops applied as issued) or document that native calls from `onMount` are queued.
- `onDestroy` and cleanups run after the native `remove`, before `destroy` (sweep at the end of the job). The widget is detached but alive, which is safe for reading final state.
- `tick()` resolves after the flush. `flushSync()` applies state changes and native ops synchronously. It is needed in factory handlers (§5.2) and in tests, and is a dev error inside other sync signal handlers (slots note §1.5).
- `untrack` works unchanged.

## 7. Styling (short)

- `class`, `class:x`, `class={{…}}`, `style`, `style:x` compile and arrive as flattened strings (`class="active"`). The renderer keeps them JS-only today. GTK: map `class` → `gtk_widget_set_css_classes` (cheap and natural). Qt: dynamic property `class` plus style repolish. `style:` would need a CSS-subset parser per toolkit; skip it and use typed props instead.
- `<style>`: `css: 'injected'` is **forbidden**. The default (external) mode compiles: the CSS comes out as `r.css.code` (`.x.svelte-1lj1c1s{color:red}`) and the scoping class is added to elements' `class`. A build step could feed that into a per-app `GtkCssProvider` (GTK CSS supports class selectors, so Svelte's hash-scoping works if `class` is mapped to CSS classes). Qt stylesheet `.Name` selectors match C++ class names, not CSS classes, so scoping would need rewriting the selectors into dynamic-property selectors (`[svelteScope="xyz"]`) plus a per-widget property. That is plausible but heavy. Status: **n/a for now**. `class` → CSS classes is the one cheap win.

## 8. Motion (short)

- `transition:`, `in:`, `out:` and `animate:` are **forbidden** (Rich: transitions "in their current form" won't carry over; attachments may get transition hooks later). `svelte/motion` (`Tween`, `Spring`) compiles but schedules through `requestAnimationFrame`, which the host must provide from the frame clock (PLAN rule 6).
- Native replacements:
  - **Enter/leave:** `GtkRevealer` (`reveal-child` prop + transition type) / `GtkStack` transitions, as wrapper components (`<Revealer show={x}>`).
  - **List add/remove:** `GtkListView` has none built in, and `AdwAnimation` can animate props.
  - **Qt:** `QPropertyAnimation` on `maximumHeight`/`windowOpacity`, `QGraphicsOpacityEffect`.
  - **Generic:** an `{@attach animate(prop, to, ms)}` that calls a host animation op, or `Tween` driving a prop at frame rate (costs one `set` per frame).
- Exit animations need "delay destroy until done", which the renderer can do in its sweep. It is not needed early.

## 9. Other

| Feature | Status |
|---|---|
| `svelte:head`, `svelte:body` | forbidden, n/a |
| `{@html}` | forbidden, n/a |
| HMR | **works** (trial §5): `compile({hmr})` + `import.meta.hot` routed to a host registry; state resets |
| SSR, hydration | n/a; the server build throws `custom_renderer_unavailable_on_server` |
| `experimental.async` (`await` in template/script, `pending` snippet) | **works** (`out/async.txt`): the pending snippet renders a native spinner, then it is swapped for the content; later updates keep old content until resolved. Without the flag, `await` in the template is a compile error. Worth enabling: it also avoids the boundary-inline bug |
| `{@attach}` / `use:` | **works**; runs after insert; the main way to reach the native handle, and to listen with flags (§3a) |
| `svelte:options`, `svelte:fragment`, legacy `<slot>` | compile (legacy mode); don't use `<slot>` in new code |

## 10. Summary table

| Feature | Status | What we need to build |
|---|---|---|
| `$state` / `$derived` / `$effect` / `$props` / `$bindable` / stores / `$inspect` | works | snapshot object props before `setProp`; `console` in dev |
| Deep `$state` passed as a native prop | proxy arrives; deep changes not re-sent | renderer `$state.snapshot` for object props; model protocol for big data |
| Text interpolation | works for `label`/`button` | metadata: text-bearing kinds + target prop; warn on stray text |
| Attributes / spread | works | metadata coercion (`""`→`true`, ints, enums); fix or avoid the spread-drops-functions bug |
| `{#if}` `{#each}` `{#key}` `{#await}` `{@const}` `{@debug}` | works | O(1) sibling lookups in the renderer (reverse of 10k = 86 s) |
| `{@html}`, `svelte:head/body` | forbidden | n/a |
| `on<signal>` handlers, callback props | works | metadata `delivery: sync` + host `call_sync` (P-later) |
| Modifiers | `on:` forbidden; `…capture` works | `listen` flags (phase/after/sync), `@svelters/events` (`prevent`/`stop`/`once`/`listen`) |
| Return values from handlers | only inline arrows/identifiers | event-object results (slots note §1.5); never rely on returns |
| `svelte:window` / `svelte:document` | forbidden | `<shortcut>`/`Action` elements, `onclose-request`, `@svelters/app` module |
| `bind:value/checked/group` on elements | forbidden | generated `$bindable` wrappers + metadata echo rules |
| `bind:this` | forbidden | `{@attach}` + typed native handle API (`callNative` prototype) |
| Component `bind:prop` | works | - |
| Dimension bindings | forbidden | `resized` pseudo-signal + `<Measure>` wrapper |
| Snippets / `{@render}` / `children` / snippet params | works | generated wrappers use them for slots |
| Virtualized lists | prototype (`$SURVEY/virtual.mjs`) | `adopt(h)`, factory signal class, `items-changed` op, `ListView` wrapper; Qt host-side virtualizing container |
| Grids (`GtkGrid`/`QGridLayout`) | not implemented | `sv-item` + `set_child_attr` |
| Tables (`GtkColumnView`/`QTableView`) | prototype (`Column` = row pool per column) | same as lists + `column` kind |
| Named native slots | `sv-slot` works | generated wrappers with snippet props; replace/restore semantics |
| Context | works | pass `getAllContexts()` into row mounts |
| `svelte:boundary` | works with `{failed}` attr; inline crashes (async off) | file/fix fork bug a, or enable `experimental.async` |
| Dynamic components, `svelte:element` | works (`svelte:element` = dynamic native kind) | - |
| Lifecycle (`onMount`, `onDestroy`, `tick`, `flushSync`, `untrack`) | works; widget exists and is inserted at `onMount` | keep "ops applied as issued" in the host |
| `class:` / `style:` / `<style>` | compile (`css:'injected'` forbidden); JS-only today | `class` → `gtk_widget_set_css_classes`; scoped CSS later |
| Transitions / animations | forbidden | `Revealer`/`Stack` wrappers; host animation op later; `requestAnimationFrame` from the frame clock for `svelte/motion` |
| HMR | works | host registry (as in the trial) |
| SSR / hydration | n/a | - |
| `experimental.async` | works | decide to enable it (also sidesteps the boundary bug) |
| `{@attach}` / `use:` | works | - |

## 11. Next 10 work items (prioritized)

1. **Generated wrapper components + bindable metadata.** Per kind: `$bindable` props with their change signal, echo rules from slots note §1.6 (replacing the hard-coded `entry`/`changed` mirror), forwarding of explicit callbacks, and typed `on*` props. This unblocks forms.
2. **Renderer complexity fix.** Linked siblings or index maps for `getNextSibling`/`nativeInsert`/`flat`. The 10k reverse takes 86 s today and 1k takes 1.3 s on Node JIT. Add a 10k keyed-reorder benchmark to `tests/renderer-conformance`.
3. **Prop value coercion from metadata.** `""` → `true` for bool props, ints/enums/flags, and `$state.snapshot` (or rejection) for objects so proxies never reach the host.
4. **Event semantics.** Event-object results for sync signals (`preventDefault`/`stopPropagation`/`returnValue`), `@svelters/events` (`prevent`, `stop`, `once`, `listen`), and the optional `flags` argument on `listen` + phase/after on dispatch (§3a). Capture comes through `on<name>capture`.
5. **Row pool for virtualized lists.** `adopt(h)` in the renderer, a `factory` delivery class (sync, ops applied immediately), `items-changed` from JS to host, `ListView`/`GridView` wrappers on `GtkSignalListItemFactory`. Port `$SURVEY/virtual.mjs` into `packages/`.
6. **`sv-item` + `set_child_attr`** for grids, notebook pages and stack pages, with `<Grid>/<Cell>`, `<Notebook>/<Page>`, `<Stack>/<StackPage>` wrappers.
7. **Native handle API for `{@attach}`.** Typed method calls and `getProp` on a node (`focus`, `scrollTo`, `selectRegion`), generated per kind, replacing `bind:this` uses.
8. **App-level API.** `<shortcut>`/`Action` elements with accelerators, `onclose-request` on `<window>` as a sync veto, multiple top-level windows, and `@svelters/app` (quit, activate, color scheme).
9. **`ColumnView`/`Column`** on the row pool (GTK), plus the Qt plan: a model-role `ListView`/`TableView` for text rows and a host-side virtualizing container for widget rows.
10. **Fork hygiene.** File bugs a (boundary inline snippet) and b (spread drops functions). Consider asking for `return` in the generated `function (...$$args) { x?.apply(this, $$args) }` handler wrapper. Decide on `experimental.async`.

## 12. Recommendation: lists, grids, tables

1. **Authoring model: one shape everywhere.** A component takes `items` and a parameterised snippet (`row`, or `cell` per column) and renders it per item. App code is identical for `<List>` (plain `{#each}`) and `<ListView>` (virtualized), so switching is a one-word change.
2. **Small or medium lists (up to a few hundred rows; anything that isn't a scroller of data):** `<List>` = `<box>{#each items as item, i (key)}{@render row(item, i)}{/each}</box>`. It works today, and updates are fine-grained (1 op per changed field). Fix the renderer's linear sibling lookups first.
3. **Large or unbounded lists:** `<ListView>` on the **native virtualized widget with a JS row pool**. One `mount()` of a tiny `RowHost` per native row slot into an adopted host container, recycling by rewriting a `$state` slot object on `bind`, `unmount` on `teardown`, and context forwarded with `getAllContexts()`. The JS array stays the model; the host gets `n-items` + `items-changed` and keeps only a key snapshot. Cost scales with visible rows: 252 ops for 21 rows, 5 ops to scroll 5 rows, 0 ops for offscreen edits. Do not use `createRawSnippet` (it throws under the renderer). The portal design from 2021 also works, but needs a portal element and couples teardown order to an `{#each}`.
4. **Qt:** text/icon rows use `QAbstractItemModel` roles with no per-row Svelte. Widget rows use a host-side virtualizing scroll area that speaks the same setup/bind/unbind/teardown protocol, which WinUI `ItemsRepeater` and AppKit `NSTableView` also map to directly.
5. **Grids:** `<Grid>` + `<Cell row col rowSpan colSpan>` via `sv-item` and `set_child_attr`. Do not encode placement in `data-slot` strings.
6. **Tables:** `<ColumnView items>` + `<Column title>{#snippet cell(row)}…{/snippet}</Column>`. Each column is a native column object with its own row pool (GTK's per-column factory), and the model is shared through context. Sorting and filtering live in JS (sorted array) or in native sort/filter models fed by keys.
