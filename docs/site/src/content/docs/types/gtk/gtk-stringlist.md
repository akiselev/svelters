---
title: Gtk.StringList
description: "Gtk.StringList from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { StringList } from 'svelters:gtk/gtk';
```

## Class

`Gtk.StringList`, GType `GtkStringList`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object` and implements `Gio.ListModel`, `Gtk.Buildable`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:StringList:*`).

## Construct

```js
const x = new StringList({ strings: ... });
```

Construct-time properties (construct-only ones can only be given here): `strings` (construct-only).

## Statics

- `StringList.new(strings)`

## Properties

- `itemType` (gtype, read-only)
- `nItems` (int, read-only)
- `strings` (strv, construct-only)

## Methods

- `append(string)`
- `find(string)`, since 4.18
- `getString(position)`
- `remove(position)`
- `splice(position, nRemovals, additions)`
- `take(string)`
- `getItemType()`, since 2.44
- `getNItems()`, since 2.44
- `getObject(position)`, since 2.44
- `itemsChanged(position, removed, added)`, since 2.44
- `getBuildableId()`

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `items-changed`

