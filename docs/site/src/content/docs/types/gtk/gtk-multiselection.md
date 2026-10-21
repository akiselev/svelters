---
title: Gtk.MultiSelection
description: "Gtk.MultiSelection from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { MultiSelection } from 'svelters:gtk/gtk';
```

## Class

`Gtk.MultiSelection`, GType `GtkMultiSelection`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object` and implements `Gio.ListModel`, `Gtk.SectionModel`, `Gtk.SelectionModel`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:MultiSelection:*`).

## Construct

```js
const x = new MultiSelection({ model: ... });
```

Construct-time properties (construct-only ones can only be given here): `model`.

## Statics

- `MultiSelection.new(model)`

## Properties

Getters marked *reactive* re-run the effects (and template expressions) that read them when the property changes natively; GTK announces the change (`notify`) and the host delivers it once per main-loop turn. *Reactive on writes* re-runs only after your own writes. The other getters are snapshots. To read a reactive getter without subscribing, use `obj.peek('prop')` or Svelte's `untrack(() => obj.prop)`. See [Reactivity](../../../concepts/reactivity/).

- `itemType` (gtype, read-only)
- `model` (object, reactive)
- `nItems` (int, read-only, reactive)

## Methods

- `getModel()`
- `setModel(model)`
- `getItemType()`, since 2.44
- `getNItems()`, since 2.44
- `getItem(position)`, since 2.44
- `itemsChanged(position, removed, added)`, since 2.44
- `getSection(position)`, since 4.12
- `sectionsChanged(position, nItems)`, since 4.12
- `getSelection()`
- `getSelectionInRange(position, nItems)`
- `isSelected(position)`
- `selectAll()`
- `selectItem(position, unselectRest)`
- `selectRange(position, nItems, unselectRest)`
- `selectionChanged(position, nItems)`
- `setSelection(selected, mask)`
- `unselectAll()`
- `unselectItem(position)`
- `unselectRange(position, nItems)`

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `items-changed`
- `sections-changed`
- `selection-changed`

