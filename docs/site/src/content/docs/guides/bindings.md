---
title: Bindings
description: Two-way binding on native widgets today (component wrappers), echo suppression, and the element bind support coming through the Svelte fork.
---

:::caution[In progress: F1]
Element `bind:this` and `bind:<prop>` are coming through our Svelte fork. Milestone F1 (`STATUS.md`, state IN PROGRESS) has two phases: phase A adds `bind:this` and a binding hook in the fork; phase B adds the renderer side and binding metadata after P5. Until then, every element `bind:` is a compile error, and this page describes the interim pattern.
:::

## What is a compile error today

Under `experimental.customRenderer`, every element binding fails to compile: `bind:value`, `bind:checked`, `bind:group` (also on a real `<input>`), `bind:this` and `bind:clientWidth`. Stock Svelte also rejects `bind:value` on anything other than `input`, `textarea` and `select` (`bind_invalid_target`), and unknown names such as `bind:text` on a custom element (`bind_invalid_name`).

What works unchanged:

| Form | Status |
| --- | --- |
| component `bind:prop` | works |
| component `bind:this` | works |
| function bindings `bind:v={get, set}` | works |
| element `bind:this`, `bind:<prop>` | forbidden today; **in progress (F1)** |
| dimension bindings (`bind:clientWidth`) | forbidden; **planned** as a `resized` pseudo-signal plus a `Measure` wrapper |

## Two-way binding through component wrappers

Two-way binding is a component feature in Svelte 5 (`$bindable`), so each bindable native property gets a wrapper component. The survey verified this pattern on the production renderer. The wrapper is generated from metadata (`entry.text` is bindable and its change signal is `changed`):

```svelte
<!-- generated: Entry.svelte -->
<script lang="ts">
  import type { EntryProps } from './entry.gen';
  let { text = $bindable(''), onchanged, ...rest }: EntryProps = $props();
</script>
<entry {...rest} text={text} onchanged={(t) => { text = t; onchanged?.(t); }} />
```

```svelte
<!-- app -->
<Entry bind:text={name} placeholder-text="Name" onactivate={() => (submitted = name)} />
<label>Hello {name}</label>
```

Resulting operations:

```text
create entry #3 ; set #3 placeholder-text "Name" ; set #3 text "Ann" ; listen #3 activate on ; listen #3 changed on
event #3 changed ["Bob"]  -> set #4 label "Hello Bob"     (no echo `set #3 text`)
JS name = "Cy"            -> set #3 text "Cy" ; set #4 label "Hello Cy"
```

:::note[State of the wrappers]
The wrapper above is the survey prototype. Generating wrapper components from metadata is the first item in the survey's work list and is not shipped. The generator produces `.d.ts` element declarations today (`packages/renderer/types/{gtk,qt}.d.ts`), not wrapper components.
:::

Notes on the pattern:

- The wrapper forwards the user's `onchanged` explicitly, because a function in `...rest` is dropped by the spread bug unless its name starts with `on` (see [known fork bugs](/guides/svelte-support/#known-fork-bugs)). `on*` handlers inside a spread are attached as events.
- Other planned wrapper forms: `bind:checked` becomes `CheckButton bind:active` or `Switch bind:active` (GTK `notify::active`, Qt `toggled`); `bind:group` becomes a `RadioGroup`/`CheckGroup` wrapper with `$bindable` `value`/`values` and items registering through context; `bind:value` on a select becomes `DropDown bind:selected`.
- egui has no widget state to echo, so the value round-trips every frame. WinUI `TwoWay` bindings and AppKit KVO fit the same wrapper.

## Echo suppression and notify signals

Setting a bound property from JS must not come back as a user change. Today, the entry mirror is hard-coded for `entry` with `changed`. The renderer mirrors `entry.text`, so a native `changed` event does not produce a redundant `set text`. The survey's recommendation is to make this metadata-driven (**planned**).

Where the change signal comes from:

| Toolkit | Notify source |
| --- | --- |
| GTK | `notify::<prop>` (`ParamSpec` with `EXPLICIT_NOTIFY`) |
| Qt | the `NOTIFY` signal of the `Q_PROPERTY` (`textChanged`, `valueChanged`, `currentIndexChanged`) |

with a curated preference for user-edit signals where they exist (Qt `textEdited` over `textChanged`, `QAbstractSlider::sliderMoved`).

Probed behavior that drives the rules:

```text
GTK Entry  set_text x, x, (blocked) y, y  -> notify::text ['x', 'x', 'y']   re-emits on identical set
GTK SpinButton set_value 5,5,500 (max 99) -> value-changed 5, 99            clamped value is emitted
GTK Switch set_active(True)               -> state-set True, then notify::active
Qt QLineEdit setText x, (blocked) y, y    -> textChanged ['x']               unchanged set does not emit
Qt QSpinBox  setValue 5,5,500 (max 99)    -> valueChanged [5, 99]
own writable props on Gtk widget classes: 581, with EXPLICIT_NOTIFY: 498
```

Design rules (from the signals research; **planned**, M4 two-way binding):

1. When the renderer sets a bound property, bracket the native set by blocking **our** handler (`g_signal_handler_block` with our handler id on GTK; a per-handle suppress flag on Qt). Never `blockSignals(true)` on Qt: it silences the app's other connections and accessibility observers.
2. After the set, read the property back. If it differs from what JS wrote (clamping, normalisation), post one synthetic change event so `$bindable` converges to the native value.
3. Compare before setting in the renderer. `GtkEntry` re-notifies on identical text and also moves the cursor.
4. Programmatic sets can emit sync signals (`Switch::state-set`). Block the sync handler too while applying a host op.
5. Prefer the user-intent signal where metadata has one.
6. GTK `freeze_notify`/`thaw_notify` helps batching but not suppression, since notifications are delivered at thaw.

In the annotation files, `echo_suppress[]` on a property and `suppress_signals` on a Qt element property record these facts. Qt `QLineEdit` programmatic `text` sets do not emit `changed` for that reason.

## `bind:this` today: `{@attach}`

`{@attach}` runs after the element is created and inserted, so it can hold the node:

```svelte
<entry {@attach (node) => { ref = node; return () => (ref = null); }} />
```

What app code can do with the node is limited: the current renderer exposes no `getProp` or `call` to components. The survey prototyped `callNative(node, method, args)`; a typed per-kind handle API (`widget(node).grabFocus()`) is **planned**. Once F1 lands, element `bind:this` is the intended syntax.

After the native widget is destroyed, a held node is marked dead; reads and calls on it throw `StaleHandleError(reason)`. This is part of the ownership design, see [Ownership](/concepts/ownership/#abi-v2-and-implementation-state).

Sources: `docs/notes/svelte-feature-survey.md` (section 4), `docs/notes/signals-and-slots-semantics.md` (section 1.6), `docs/notes/p5-integration.md`, `STATUS.md`, `PLAN.md`.
