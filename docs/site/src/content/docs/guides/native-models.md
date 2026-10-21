---
title: Native models in lists
description: Showing a GTK GListModel or a Qt QAbstractItemModel in <ListView> without copying it into JS, how rows find their items, selection, lazy models, and when to use an array instead.
---

`<ListView>` takes either a JS array or a native list model. With an array, JS owns the data and a keyed diff turns each update into minimal native splices (see [Lists and snippets](/guides/lists-and-snippets/)). With a native model, the toolkit's own view reads the model directly. JS keeps no copy of the list, no keys and no ids. It renders only the rows that are visible.

```svelte
<script>
  import { ListView } from '@svelters/lists';
  import { DirectoryList } from 'svelters:gtk/gtk';
  import { File, FileInfo } from 'svelters:gtk/gio';

  const dir = new DirectoryList({ file: File.newForPath('/usr/share'), attributes: 'standard::display-name' });
</script>

<label label={`${dir.nItems} files${dir.loading ? ', loading' : ''}`} />
<ListView items={dir} itemClass={FileInfo} vexpand>
  {#snippet row(info, index)}
    <label label={`${index}: ${info.getDisplayName()}`} />
  {/snippet}
</ListView>
```

The Qt version is the same component:

```svelte
<script>
  import { ListView } from '@svelters/lists';
  import { QStringListModel } from 'svelters:qt/core';
  const names = new QStringListModel();
  names.setStringList(['alpha', 'beta']);
</script>

<ListView items={names}>
  {#snippet row(index)}<label>{names.data(index)}</label>{/snippet}
</ListView>
```

## Which models

- **GTK:** any object that implements `GListModel`. That includes `Gio.ListStore`, `Gtk.StringList`, `Gtk.DirectoryList`, the filter, sort, flatten, map and slice models, the selection models, and models that APIs return.
- **Qt:** any `QAbstractItemModel`: `QStringListModel`, `QStandardItemModel`, `QSortFilterProxyModel` and the others. The list shows the root rows of column 0.

`items` (or the `model` prop, which wins when both are set) decides the mode. An object with a native handle is shown as a model. Anything else is iterated as an array. Switching between the two modes at runtime is allowed: the list starts over.

## What a row receives

The row snippet gets `(item, index)`, as in array mode.

- **GTK:** the item is the object the model holds at that position, as a wrapper. An object that already has a wrapper keeps it (identity: the `StringObject` you appended is the one you get). For items made natively, such as the `GFileInfo`s of a `DirectoryList`, pass `itemClass={FileInfo}` so rows get that class's methods. Without it, the row gets a plain `NativeObject` and `FileInfo.from(item)` casts it.
- **Qt:** a `QStandardItemModel`'s rows are its `QStandardItem`s. Any other model's rows are `QModelIndex` values, which you read through the model (`model.data(index)`).
- `item={(model, position, raw) => ...}` overrides both rules.

Rows stay current without mirrors:

- Reactive getters on items re-render a row when the native property changes (GTK `notify::`, see [Reactivity of native properties](/concepts/reactivity/)).
- A Qt `dataChanged` over a bound row re-renders it, so `it.text()` and `model.data(index)` show the new data.
- `index` follows inserts, removals and moves, including a sort that keeps a row on its item.

## Changing the model

Mutate the model natively: `store.insert(2, item)`, `store.splice(1, 2, [a, b, c])`, `store.sort(compare)` on GTK; `insertRows`, `removeRows`, `setData`, `sort` on Qt. Each mutation is the toolkit's own change notification (one `items-changed` on GTK; one row signal or layout change on Qt), and the view updates only what it shows. JS does no diff.

Filtering and sorting can be native too: a `Gtk.FilterListModel` over a `Gtk.SortListModel` with `Gtk.CustomFilter.new(fn)` and `Gtk.CustomSorter.new(fn)`, or a `QSortFilterProxyModel`. A JS predicate or comparator is called once per item or per comparison while GTK sorts, which is fine for thousands of items and not for hundreds of thousands. For those, sort in JS and use an array, or use a sorter that needs no JS.

## Counts, loading and selection

These are ordinary reactive getters: `store.nItems`, `dir.nItems`, `dir.loading` and `selection.selected` re-render whatever reads them. No `items-changed` listener is needed.

- **Selection, GTK:** a model that is itself a selection model (`Gtk.SingleSelection`, `Gtk.MultiSelection`, `Gtk.NoSelection`) is shown as is, and its own `selected` / `isSelected()` are the selection. Any other model gets the list's single selection, and `bind:selected` works as in array mode.
- **Selection, Qt:** the view's own selection; `bind:selected` works in both modes.

A lazy model such as `DirectoryList` grows while it loads. The view shows the first rows as soon as they exist, and `nItems` follows.

## Arrays or native models

| | JS array | native model |
| --- | --- | --- |
| Source of truth | JS (`$state.raw`) | the native model |
| Update cost | a keyed diff over all items (O(n) key reads), then minimal splices | the toolkit's own notification, no JS pass |
| Row item | your JS object | the native object (GTK) or row index / item (Qt) |
| Sort and filter | in JS (`$derived`), then the diff | native models, or JS functions per item |
| Good for | app data that lives in JS | data that already lives natively: files, settings, stores shared with native code, big lists |

Measured on the test machine (debug build, shared machine, so read them as ratios; `crates/svelters-gtk/tests/lists_models.rs`, `examples/counter-qt/tests/lists_models.rs`): a 100k-string model shows its first rows in about 1.0 s on GTK and 0.27 s on Qt, against 1.4 s and 2.1 s for a 100k-item array. Scrolling costs only the rebinds of the rows that enter the view in both modes. The view keeps the same bounded pool of rows in both modes: about a screen of rows plus a margin on Qt, and the slots GTK creates for its first layout.

## Protocol

The native side of model mode is an additive extension of the `virtual-list` protocol: a `model` property, two arguments appended to `bind` (the change counter and, on GTK, the item), and the `items-changed`, `data-changed` and `moved` signals. See `plans/architecture/backend-abi.md`, "Native models (N3)".
