---
title: Events
description: How native signals reach Svelte handlers, what modifiers and return values do today, and what is planned for sync signals and listen flags.
---

Native signals reach your component as ordinary `on*` attributes. This page separates what works today from what is design only.

## What works today

```svelte
<button onclick={() => count++}>Increment</button>
<button onclicked={() => count++}>Increment</button>
<window onclose-request={(e) => { /* ... */ }}>
```

| Source | Renderer call | Host listen |
| --- | --- | --- |
| `onclick={f}` | `addEventListener(el, 'click', handler, {})` | `listen(h, clicked, on)` |
| `onclicked={f}` | `addEventListener(el, 'clicked', handler, {})` | `listen(h, clicked, on)` |
| `onclose-request={f}` | type `close-request` | `listen(h, close-request, on)` |
| `onclickcapture={f}` | type `click`, options `{capture: true}` | see [capture](#capture) |

- **Aliases.** `EVENT_TO_SIGNAL` in `packages/renderer/src/index.js` is `{ click: 'clicked', change: 'changed' }`. Any other name passes through unchanged.
- **No delegation.** The renderer receives each handler directly. Svelte's own delegation does not apply.
- **Raw arguments.** A handler gets the host's arguments spread: `changed` delivers the text (`onchanged={(text) => ...}`), `clicked` delivers none. Handler `this` is the node.
- **Queued delivery.** Events are queued by the backend and delivered to JS at the next checkpoint. Effects and tree operations that follow happen in microtasks, so their ops appear at the next checkpoint, not inside the dispatch.
- **Component callback props** (`onSave={...}`) are plain props and work.
- **Exceptions** in a handler propagate out of dispatch and stop the remaining handlers of that dispatch.

### Entry `changed`

Both backends deliver `[text]` for `changed`. Programmatic sets of `text` do not emit `changed` on Qt (the annotation uses `suppress_signals`, implemented with `QSignalBlocker`), and GTK emits it twice (delete plus insert), so tests accept repeats.

## Why `on:` does not compile

Under `experimental.customRenderer`, every `on:` directive is a compile error: `incompatible_with_custom_renderer: 'on:' is not compatible with 'customRenderer'`. It applies to `on:clicked|once` in both legacy and runes components. Use `onclicked`, not `on:clicked`.

In stock Svelte, mixing `on:click` and `onclick` in one component is also an error (`mixed_event_handler_syntaxes`).

## Modifiers

Svelte 5's own replacement for modifiers is a plain function wrapper written by the user. It needs no renderer support and only affects the JS side:

```svelte
<script>
  const once = (fn) => { let ran = false; return (...a) => { if (!ran) { ran = true; return fn(...a); } }; };
</script>
<button onclick={once(() => count++)}>Increment once</button>
```

### Capture

`onclickcapture` and `onkey-pressedcapture` work: Svelte strips a trailing `capture` from the event name and passes `{capture: true}` to the renderer. Note that the strip applies to any event name, so a native signal whose name ends in `capture` cannot be listened to without a rename.

:::caution[Planned: native capture phase]
Today `capture` reaches the renderer but does not select a native capture-phase connection. The renderer prototype maps `capture: true` to `flags.phase = 'capture'`, which needs the `listen` flags ABI change below. Until then, non-empty flags are not supported by the backends.
:::

## The event object (design, M4)

:::caution[Planned]
`call_sync` returns an error until M4. The synchronous event-object protocol below is the design from the signals research, not shipped behavior.
:::

Some native signals expect an answer during emission: veto a window close, mark a key press handled, accept a drop. The design reads the result from the event object after dispatch, as the DOM and QML do, rather than from the return value:

| Event field | Meaning | GTK result | Qt result |
| --- | --- | --- | --- |
| `defaultPrevented` (`e.preventDefault()`) | keep the default from happening | `TRUE` from a `stop_on_true` signal (`close-request`, `activate-link`, `key-pressed`) | event filter ignores the event (`close`) or returns `true` |
| `cancelBubble` (`e.stopPropagation()`) | stop propagation | same bool | `ev->accept(); return true` |
| `returnValue` | explicit value for value-returning signals (`DragAction`, tooltip bool) | the value | `setDropAction(...)` and so on |

An async handler works if it sets flags before its first `await`:

```svelte
<Window onclose-request={async (e) => {
  if (!dirty) return;
  e.preventDefault();                          // window stays open
  if (await confirmDiscard()) win.destroy();   // runs at a later checkpoint
}} />
```

This is the opposite of gjs, where a returned Promise is truthy and silently becomes `TRUE`.

Metadata marks each signal `delivery: queued` or `sync`. Only signals with a JS listener are connected, so a sync signal with no listener costs nothing. Other rules of the design:

- JS may read anything and write `$state` inside a sync handler. QuickJS jobs are not drained inside `call_sync`, and `flushSync()` there is a dev error.
- Host tree and property operations issued from a sync handler are appended to the checkpoint queue, not applied, except methods on borrowed argument objects.
- A sync-handler call is bounded: dev warns at 16 ms and interrupts at 1 s; release defaults to 250 ms. An exception or timeout returns the signal's `default_result`.
- Depth of nested sync emission is capped at 8.

The slots research lists open questions on this design (deferred ops versus direct application, delivering queued events before a sync handler, timeout budgets).

### Return values today

`dispatch` returns `{ results, event }`: each handler's return value in registration order, and `args[0]` when it is a non-null object. The host may ignore `results` until sync signals exist.

:::danger[Do not rely on return values]
The compiler passes inline arrows and plain identifiers straight through, but wraps every other expression (props, calls, member expressions, `stop(fn)`) in a function that does not `return`. A handler received as a prop cannot return a value to the host. Conversely, an inline arrow that happens to return a value (`() => (todo.done = true)`) does return it, so "truthy means handled" would misfire. The design keeps `accept_return: false` as the default.
:::

## `@svelters/events` (planned)

:::caution[Planned]
`@svelters/events` does not exist yet. These helpers are the recommendation from the modifier survey.
:::

| Helper | Does |
| --- | --- |
| `prevent(fn)` | calls `e.preventDefault()`, then `fn` |
| `stop(fn)` | calls `e.stopPropagation()`, then `fn` |
| `once(fn)` | runs `fn` once; a native disconnect is only an optimisation |
| `listen(type, fn, flags)` | for use with `{@attach}`, built on `svelte/events` `on()` |

`prevent`, `stop` and `once` only touch the event object or JS state, so they survive the lost-return-value wrappers and async handlers.

Intended usage with `{@attach}`:

```svelte
<window onclose-request={prevent(() => log.push('close vetoed'))}>
  <entry
    onkey-pressedcapture={(e) => log.push('capture ' + e.keyval)}
    onkey-pressed={stop((e) => log.push('bubble ' + e.keyval))}
    {@attach after('notify::text', () => log.push('after notify'))}
    {@attach listen('activate', once(() => log.push('activate once')), { sync: true })} />
</window>
```

`{@attach}` runs after the element is inserted, and `svelte/events` `on(el, type, fn, options)` forwards any options object to `renderer.addEventListener` unchanged and removes the listener on cleanup. That is why it is the channel for listen-time flags: the renderer never sees the user's function (only Svelte's `target_handler` wrapper), so tags set on the function are invisible at listen time. This already works with stock `svelte/events`; the helpers and the host support for the flags do not exist yet.

## Listen flags (ABI proposal, not applied)

Listen-time semantics must reach the backend, because they select a separate native connection. Dispatch-time semantics (prevent, stop) are results of one emission and live on the event object.

| Semantic | GTK | Qt | Needed at |
| --- | --- | --- | --- |
| capture phase | `gtk_event_controller_set_propagation_phase(GTK_PHASE_CAPTURE)` on a controller per listener | event filter on the target or an ancestor | listen time |
| after default handler | `G_CONNECT_AFTER` | call the base `event()` first in the subclass override | listen time |
| sync | closure marshalled into JS through `call_sync` | filter or virtual override calling JS | listen time (from metadata `delivery: sync`) |
| stop or prevent | `GDK_EVENT_STOP` or `TRUE` | `accept()` or filter result | dispatch time |
| once | `g_signal_handler_disconnect` after first emission | `Qt::SingleShotConnection` | either; JS-side `once` is enough |

Proposed ABI addition (**not applied**):

```text
listen(h, signal, on, flags?)
flags = { phase?: 'capture' | 'target' | 'bubble', after?: bool, sync?: bool }
```

- Two listens with different flags are separate native connections. `off` must carry the same flags.
- The host's dispatch back into JS must say which connection fired: `__svelters_dispatch(h, signal, args, {phase, after})`.
- `sync` is normally derived from metadata; the flag is only an override. `once` stays JS-side.
- Until a backend needs it, the renderer can reject non-empty flags with a clear error.

The current `listen` has no `flags` parameter; see [the ABI reference](/concepts/abi/#listen).

## Replacing `svelte:window`

`svelte:window`, `svelte:document` and `svelte:body` are compile errors. Each use case has a native replacement, none implemented yet (**planned**, "app-level API"):

| Browser idiom | Native replacement |
| --- | --- |
| global shortcut | `GtkShortcutController` or `QShortcut`; element form `<shortcut trigger="..." onactivated={quit} />` |
| `beforeunload` / window close | `<window onclose-request={(e) => dirty && e.preventDefault()}>` (sync signal) |
| `resize` | `notify::default-width` (GTK), `QEvent::Resize` filter (Qt), exposed as an `onresized` pseudo-signal |
| theme, activation | an app module, for example `@svelters/app` with `colorScheme`, `active` and `quit()` |

Sources: `docs/notes/svelte-feature-survey.md` (sections 3 and 3a), `docs/notes/signals-and-slots-semantics.md` (sections 1.3 to 1.6), `docs/notes/p1-js-lane.md`, `packages/renderer/src/index.js`.
