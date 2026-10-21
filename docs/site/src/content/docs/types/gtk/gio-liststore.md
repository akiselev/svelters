---
title: Gio.ListStore
description: "Gio.ListStore from svelters:gtk/gio, a shared object class generated from the GIR."
---

```js
import { ListStore } from 'svelters:gtk/gio';
```

## Class

`Gio.ListStore`, GType `GListStore`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object` and implements `Gio.ListModel`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gio:ListStore:*`).

## Construct

```js
const x = new ListStore({ itemType: ... });
```

Construct-time properties (construct-only ones can only be given here): `itemType` (construct-only).

## Statics

- `ListStore.new(itemType)`, since 2.44

## Properties

Getters marked *reactive* re-run the effects (and template expressions) that read them when the property changes natively; GTK announces the change (`notify`) and the host delivers it once per main-loop turn. *Reactive on writes* re-runs only after your own writes. The other getters are snapshots. To read a reactive getter without subscribing, use `obj.peek('prop')` or Svelte's `untrack(() => obj.prop)`. See [Reactivity](../../../concepts/reactivity/).

- `itemType` (gtype, construct-only)
- `nItems` (int, read-only, reactive)

## Methods

- `append(item)`, since 2.44
- `find(item)`, since 2.64
- `insert(position, item)`, since 2.44
- `insertSorted(item, compareFunc)`, since 2.44; `compareFunc` is a callback (call scope)
- `remove(position)`, since 2.44
- `removeAll()`, since 2.44
- `sort(compareFunc)`, since 2.46; `compareFunc` is a callback (call scope)
- `splice(position, nRemovals, additions)`, since 2.44
- `getItemType()`, since 2.44
- `getNItems()`, since 2.44
- `getItem(position)`, since 2.44
- `itemsChanged(position, removed, added)`, since 2.44

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `items-changed`

## Not bound yet

2 members are not bound; reasons: callback 2.

