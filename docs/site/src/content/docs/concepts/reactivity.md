---
title: Reactivity of native properties
description: How reading a native property inside an effect subscribes to its changes, how the host batches them, and how to opt out.
---

A native property is state the toolkit owns. Reading it inside an effect (a template expression, `$derived`, `$effect`) behaves like reading `$state`: the effect runs again when the value changes.

```svelte
<script>
  import { TextBuffer } from 'svelters:gtk/gtk';
  const buffer = new TextBuffer({ text: 'Hello' });
</script>

<text-view buffer={buffer} />
<label label={`${buffer.text.length} characters`} />
```

The label follows what the user types into the view. No `$state` copy and no `on('changed', ...)` listener are needed.

## How it works

- **Getters subscribe, then read live.** A reactive getter registers the running effect for `(object, property)` and then reads the value from the native object. Outside an effect, or inside `untrack(...)`, it is a plain read. A missed notification can leave a value stale, but a read never returns a wrong value.
- **The toolkit announces changes.** On GTK this is `notify::<prop>`; on Qt it is the property's NOTIFY signal (QBindable, events and other signals are coming). The backend reports "this may have changed" to the host. It sends no value.
- **The host batches the changes.** Within one main-loop turn it collects the announced properties and drops duplicates. At the next checkpoint it reads each one natively and compares it with the last value it saw. Equal values are dropped. The rest go to JS in **one batch, before the queued events**, and Svelte then runs its effects once. Ten thousand changes in one turn cost one effect run, not ten thousand.
- **Your own writes.** A setter re-runs the readers at once. The host reads the value back, so the toolkit's echo of your write is not delivered a second time. If the toolkit adjusts the value (a slider clamps 150 to 100), the adjusted value is delivered once.
- **Large values.** Whole-text properties such as `TextBuffer.text` cost time in proportion to their size. The host does not read or compare them. It only invalidates them, and the getter reads them when an effect needs them. Prefer length-like properties in effects that run often.

## Which properties are reactive

Every property has an observability class per platform. The class comes from the metadata, and a generated test verifies it (`metadata/observe/<toolkit>.json`):

| Class | Getter | Notes |
|---|---|---|
| exact | reactive | every native change is announced |
| partial | reactive | some causes are not announced; the API pages say which |
| set_only | reactive on writes | only your own setters change it |
| constant | plain read | never changes after construction |
| none | plain read (snapshot) | changes are not announced; the typings say so in `@remarks` |

A property without a fact is unknown and its getter is a plain read. The API pages of the [non-GUI types](../../types/gtk/) mark each property *reactive*, *reactive on writes* or *not reactive*.

## Opting out

- `obj.peek('text')` reads a property without subscribing (a snapshot on purpose).
- Svelte's `untrack(() => obj.text)` does the same for any expression.

## For runtime and binding authors

`@svelters/native-runtime` exports the tracker the generated getters use:

- `observeProp(handle, prop, fn)` calls `fn(value)` once per delivered change and returns the unsubscribe function. `value` is `undefined` for whole-text properties.
- `trackProp(handle, prop)` subscribes the running effect.
- `invalidateProp(handle, prop)` re-runs the readers now.

Observation holds nothing alive. The tracker is keyed by handle number. The last effect that reads a property stops its native observation when it is destroyed. The host contract is in `plans/architecture/backend-abi.md` ("ABI v2.6"), and the design in `plans/research/REACTIVITY.md`.
