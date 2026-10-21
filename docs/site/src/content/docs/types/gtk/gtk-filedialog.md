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

Getters marked *reactive* re-run the effects (and template expressions) that read them when the property changes natively; GTK announces the change (`notify`) and the host delivers it once per main-loop turn. *Reactive on writes* re-runs only after your own writes. The other getters are snapshots. To read a reactive getter without subscribing, use `obj.peek('prop')` or Svelte's `untrack(() => obj.prop)`. See [Reactivity](../../../concepts/reactivity/).

- `acceptLabel` (str, reactive)
- `defaultFilter` (object, reactive)
- `filters` (object, reactive)
- `initialFile` (object, reactive)
- `initialFolder` (object, reactive)
- `initialName` (str, reactive)
- `modal` (bool, reactive)
- `title` (str, reactive)

## Methods

- `getAcceptLabel()`, since 4.10
- `getDefaultFilter()`, since 4.10
- `getFilters()`, since 4.10
- `getInitialFile()`, since 4.10
- `getInitialFolder()`, since 4.10
- `getInitialName()`, since 4.10
- `getModal()`, since 4.10
- `getTitle()`, since 4.10
- `setAcceptLabel(acceptLabel)`, since 4.10
- `setDefaultFilter(filter)`, since 4.10
- `setFilters(filters)`, since 4.10
- `setInitialFile(file)`, since 4.10
- `setInitialFolder(folder)`, since 4.10
- `setInitialName(name)`, since 4.10
- `setModal(modal)`, since 4.10
- `setTitle(title)`, since 4.10

## Async methods

Each folds a native `_async`/`_finish` pair into one call that returns a Promise. It resolves with the finish result (an object of the out parameters where there are any) and rejects with a `NativeError`. The native `GCancellable` is not exposed: pass `{ signal }` (an `AbortSignal`) as the last argument to cancel, which rejects with code `cancelled`. See [Async calls and callbacks](../async/).

- `open(parent, { signal }?)` → `Promise<object>` (`open` + `open_finish`), since 4.10
- `openMultiple(parent, { signal }?)` → `Promise<object>` (`open_multiple` + `open_multiple_finish`), since 4.10
- `openMultipleTextFiles(parent, { signal }?)` → `Promise<{ ret, encoding }>` (`open_multiple_text_files` + `open_multiple_text_files_finish`), since 4.18
- `openTextFile(parent, { signal }?)` → `Promise<{ ret, encoding }>` (`open_text_file` + `open_text_file_finish`), since 4.18
- `save(parent, { signal }?)` → `Promise<object>` (`save` + `save_finish`), since 4.10
- `saveTextFile(parent, { signal }?)` → `Promise<{ ret, encoding, lineEnding }>` (`save_text_file` + `save_text_file_finish`), since 4.18
- `selectFolder(parent, { signal }?)` → `Promise<object>` (`select_folder` + `select_folder_finish`), since 4.10
- `selectMultipleFolders(parent, { signal }?)` → `Promise<object>` (`select_multiple_folders` + `select_multiple_folders_finish`), since 4.10

