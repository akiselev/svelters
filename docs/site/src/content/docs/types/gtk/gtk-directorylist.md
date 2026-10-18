---
title: Gtk.DirectoryList
description: Gtk.DirectoryList from svelters:gtk/gtk: a shared object class generated from the GIR.
---

```js
import { DirectoryList } from 'svelters:gtk/gtk';
```

## Class

`Gtk.DirectoryList`, GType `GtkDirectoryList`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object` and implements `Gio.ListModel`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:DirectoryList:*`).

## Construct

```js
const x = new DirectoryList({ attributes: ..., file: ..., ioPriority: ... });
```

Construct-time properties (construct-only ones can only be given here): `attributes`, `file`, `ioPriority`, `monitored`.

## Statics

- `DirectoryList.new(attributes, file)`

## Properties

- `attributes` (str)
- `file` (object)
- `ioPriority` (int)
- `itemType` (gtype, read-only)
- `loading` (bool, read-only)
- `monitored` (bool)
- `nItems` (int, read-only)

## Methods

- `getAttributes()`
- `getFile()`
- `getIoPriority()`
- `getMonitored()`
- `isLoading()`
- `setAttributes(attributes)`
- `setFile(file)`
- `setIoPriority(ioPriority)`
- `setMonitored(monitored)`
- `getItemType()`, since 2.44
- `getNItems()`, since 2.44
- `getObject(position)`, since 2.44
- `itemsChanged(position, removed, added)`, since 2.44

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `items-changed`

## Not bound yet

2 members are not bound; reasons: gerror 2.

