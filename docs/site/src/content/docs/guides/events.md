---
title: Events
description: How native signals reach Svelte handlers, listen phases (capture, after), sync signals and the NativeEvent object, and what replaces DOM modifiers.
---

Native signals reach your component as ordinary `on*` attributes. The compiler checks every event name against the toolkit's semantic spec, so a misspelt or missing signal is a compile error (`unknown_event`) with the closest names.

## Handlers

```svelte
<button onclicked={() => count++}>Increment</button>
<entry onchanged={() => changes++} />
```

| Source | Runtime call | Host listen |
| --- | --- | --- |
| `onclicked={f}` | `$n.on(node, 'clicked', f)` | `listen(h, clicked, on)` |
| `onkey-pressedcapture={f}` | `$n.on(node, 'key-pressed', f, LISTEN_CAPTURE)` | `listen(h, key-pressed, on, capture)` |
| `{@attach listen('key-pressed', f, { after: true })}` | `$n.listen('key-pressed', f, LISTEN_AFTER)` | `listen(h, key-pressed, on, after)` |

- **Native names only.** The event name is the native signal name in kebab case (`onclicked`, `ontext-changed` on a Qt `line-edit`, `onclose-request` on a GTK window). There are no DOM-style aliases: `onclick` is `unknown_event`.
- **Raw arguments.** A handler gets the signal's arguments spread, as the spec lists them: `changed` on a GTK entry and `text-changed` on a Qt `line-edit` deliver the text (`onchanged={(text) => ...}`), `clicked` delivers none. Handler `this` is the runtime node.
- **One connection per signal and phase.** The host listens on the first handler for a (signal, phase) pair and stops when the last one goes. Handlers registered inside a block are removed with it.
- **Queued delivery.** Ordinary signals are queued by the backend and delivered to JS at the next checkpoint. State writes in a handler are flushed by Svelte's microtask batch, so the resulting ops appear at the next job drain, not inside the dispatch.
- **Component callback props** (`onSave={...}`) are plain props and work.
- **Legacy `on:clicked={handler}`** compiles to the same listener as `onclicked={handler}`. Modifiers (`on:clicked|once`), bare forwarding (`on:clicked`) and `on:` on components are compile errors.

### Entry text changes

GTK `entry` `changed` and Qt `line-edit` `text-changed` both deliver `[text]`. Programmatic sets of `text` do not emit `text-changed` on Qt (the annotation uses `suppress_signals`), and GTK emits `changed` twice for one replacement (delete plus insert). For two-way state, use [`bind:text`](/guides/bindings/) rather than a handler.

<a id="listen-flags-abi-proposal-not-applied"></a>

## Listen phases

Some toolkits deliver a signal in more than one phase. Each phase is a separate native connection, so it is chosen at listen time.

- **Capture.** `on<signal>capture={f}`, Svelte's capture suffix, listens in the capture phase. The compiler first looks up the full name, so a native signal whose name ends in `capture` still works.
- **After.** `{@attach listen(signal, f, { after: true })}` connects after the default handler.

`listen`, `LISTEN_AFTER` and `LISTEN_CAPTURE` come from `@svelters/native-runtime`. When the signal and options are static, the compiler resolves them at compile time. The spec lists the phases each signal accepts; anything else is `listen_phase_unsupported`:

| Toolkit | `after` | `capture` |
| --- | --- | --- |
| GTK | every signal (`G_CONNECT_AFTER`) | event controllers only (`GTK_PHASE_CAPTURE`) |
| Qt | QObject signals | the synthetic `key-press` only |

```svelte
<script>
  import { listen } from '@svelters/native-runtime';
  let log = $state('');
</script>

<entry>
  <event-controller-key
    onkey-pressed={() => (log += 'b')}
    onkey-pressedcapture={() => (log += 'c')}
    {@attach listen('key-pressed', () => (log += 'a'), { after: true })}
  />
</entry>
```

GTK delivers key presses, clicks, motion and focus through event controllers: `<event-controller-key>`, `<gesture-click>`, `<event-controller-motion>`, `<event-controller-scroll>`, `<event-controller-focus>`. They are attached to the enclosing widget whatever its child slots, so they can sit inside any element.

<a id="the-event-object-design-m4"></a>

## Sync signals and `NativeEvent`

Some native signals expect an answer during emission: veto a window close, mark a key press handled. The spec marks them `sync` (GTK: signals with a handler result or `sync_required`, such as `close-request` and `key-pressed`; Qt: the synthetic `close-request` and `key-press`). The host delivers them through `call_sync`, and their handlers get a `NativeEvent` first: `(event, ...args)`.

```svelte
<window title="Editor" onclose-request={(e) => {
  if (dirty) e.preventDefault();               // window stays open
}}>
  <entry>
    <event-controller-key onkey-pressed={(e, keyval) => {
      if (keyval === 97) e.preventDefault();   // key 'a' marked handled
    }} />
  </entry>
</window>
```

The result for the toolkit:

1. `e.result = v` wins (typed results, including objects that become out parameters).
2. Else `e.preventDefault()` or `e.stopPropagation()` give `true` (veto the close, key handled).
3. Else `null`: the toolkit's default.

`e.stopImmediatePropagation()` skips the remaining handlers. Handler return values are ignored, as Svelte ignores them, and the compiler warns about a sync handler that returns a boolean literal (`sync_handler_return`) or whose first parameter is named like the signal's first argument (`sync_handler_arguments`), both signs of a handler written for the old shape.

An async handler must set flags before its first `await`; a handler that returns a Promise draws a console warning. When JS is busy as the signal fires, the toolkit uses its default and the host delivers the event later with `e.deferred = true`; flags set then are ignored.

`NativeEvent` is exported from `@svelters/native-runtime` together with `StaleHandleError`.

## Modifiers

DOM modifiers do not compile. Svelte 5's own replacement is a plain function wrapper, which needs no runtime support:

```svelte
<script>
  const once = (fn) => { let ran = false; return (...a) => { if (!ran) { ran = true; return fn(...a); } }; };
  const prevent = (fn) => (e, ...a) => { e.preventDefault(); return fn(e, ...a); };
</script>
<button onclicked={once(() => count++)}>Increment once</button>
<window onclose-request={prevent(() => log.push('close vetoed'))} />
```

`prevent` and `stop` only make sense on sync signals, where the first argument is the `NativeEvent`. There is no shipped helper package for these.

## Replacing `svelte:window`

`svelte:window`, `svelte:document` and `svelte:body` are compile errors. Native replacements:

| Browser idiom | Native replacement |
| --- | --- |
| global shortcut | an `<action shortcut="Ctrl+Q" onactivate={...}>` in a menu or tool bar, or an `<event-controller-key>` on the window (GTK) |
| `beforeunload` / window close | `<window onclose-request={(e) => dirty && e.preventDefault()}>` (sync signal) |
| `resize` | `notify::default-width` (GTK) and similar property notifications; no portable `onresized` yet (**planned**) |
| theme | `setColorScheme` from `@svelters/native-runtime/style`; following the system setting is open |

Sources: `docs/notes/native-compiler/runtime-contract.md` ("Events"), `docs/notes/native-compiler/deviations.md`, `docs/notes/x2-sync-frame.md`, `docs/notes/x3-childattr-listen.md`, `docs/notes/x8-events-draw.md`, `docs/notes/signals-and-slots-semantics.md` (sections 1.3 to 1.6), `packages/native-runtime/src/index.js`, `fixtures/sync-handlers/`, `fixtures/listen-phases/`.
