---
title: Gtk.FileDialog
description: "Gtk.FileDialog from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { FileDialog } from 'svelters:gtk/gtk';
```

## Class

`Gtk.FileDialog`, GType `GtkFileDialog`, since 4.10.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:FileDialog:*`).

## Construct

```js
const x = new FileDialog({ acceptLabel: ..., defaultFilter: ..., filters: ... });
```

Construct-time properties (construct-only ones can only be given here): `acceptLabel`, `defaultFilter`, `filters`, `initialFile`, `initialFolder`, `initialName`, `modal`, `title`.

## Statics

- `FileDialog.new()`, since 4.10

## Properties

- `acceptLabel` (str)
- `defaultFilter` (object)
- `filters` (object)
- `initialFile` (object)
- `initialFolder` (object)
- `initialName` (str)
- `modal` (bool)
- `title` (str)

## Methods

- `getAcceptLabel()`, since 4.10
- `getDefaultFilter()`, since 4.10
- `getFilters()`, since 4.10
- `getInitialFile()`, since 4.10
- `getInitialFolder()`, since 4.10
- `getInitialName()`, since 4.10
- `getModal()`, since 4.10
- `getTitle()`, since 4.10
- `open(parent)`, since 4.10
- `openMultiple(parent)`, since 4.10
- `openMultipleTextFiles(parent)`, since 4.18
- `openTextFile(parent)`, since 4.18
- `save(parent)`, since 4.10
- `saveTextFile(parent)`, since 4.18
- `selectFolder(parent)`, since 4.10
- `selectMultipleFolders(parent)`, since 4.10
- `setAcceptLabel(acceptLabel)`, since 4.10
- `setDefaultFilter(filter)`, since 4.10
- `setFilters(filters)`, since 4.10
- `setInitialFile(file)`, since 4.10
- `setInitialFolder(folder)`, since 4.10
- `setInitialName(name)`, since 4.10
- `setModal(modal)`, since 4.10
- `setTitle(title)`, since 4.10

