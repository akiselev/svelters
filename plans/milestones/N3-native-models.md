# N3 - native models and reactivity (NON-GUI-TYPES §9, row N3)

`@svelters/lists` takes native list models: any GTK `GListModel` and any Qt `QAbstractItemModel` the app holds
as a handle. Reactive accessors (§9's second half) had already landed in R1; N3 confirms the TextBuffer
acceptance on master and adds the model-side reactive props it needs. User guide: `docs/site/src/content/docs/guides/native-models.md`.
Protocol text: `plans/architecture/backend-abi.md`, "Native models (N3)".

## The contract (one component, two toolkits)

```svelte
<ListView items={model} itemClass={FileInfo}>{#snippet row(item, index)}...{/snippet}</ListView>
```

- **Mode by value.** `items` (or `model`, which wins) that is an object with a native handle is a *native model*;
  anything else is iterated as an array (M5's keyed diff). Switching modes at runtime starts the list over.
- **The view reads the model natively.** GTK: `GtkListView`/`GtkGridView` over the model, wrapped in our
  `GtkSingleSelection`/`GtkNoSelection` unless the model is itself a `GtkSelectionModel` (then shown as is). Qt:
  `QListView::setModel(model)` with the M5 viewport pool. `n_items`/`get_item`/`rowCount`/`data` never reach JS.
  JS holds no copy, no keys and no ids; only visible rows are rendered (row slots recycled as in M5).
- **Change reports, not diffs.** The native list reports the model's changes to JS in one normalized form:
  `items-changed(pos, removed, added, epoch)` (GTK `items-changed`; Qt `rowsInserted`/`rowsRemoved`, `rowsMoved` as
  remove + insert, `modelReset`/`layoutChanged` as a whole-range replace), `data-changed(first, last, epoch)` (Qt
  `dataChanged`; GTK items announce their own changes), and `moved(row, pos, gen, epoch)` for a bound row whose
  item moved without a rebind (a GTK sort keeps rows on their items; a Qt layout change moves persistent indexes).
  JS uses them only to keep bound rows' `index` right and to re-read Qt rows whose data changed.
- **Ordering by epoch.** A sync `bind` runs at once while posted change events wait for the checkpoint, and a
  deferred `bind` can run after change events posted later. Every report carries the list's change counter; `bind`
  reports it too, and JS applies a change to a bound slot only when it is newer than the slot's. GTK connects its
  forwarding handler to the model before the view does, so a bind made inside an emission already counts it.
- **Row items.** GTK: `bind` carries the item's handle (SHARED), wrapped with identity (an object made in JS keeps its
  wrapper) and as `itemClass` when given. Qt: JS resolves the row at bind time (`QStandardItemModel` -> its
  `QStandardItem`, any other model -> `index(row, 0)`, a persistent `QModelIndex`); Qt binds run from the pool's
  relayout timer, never inside a model change. `item(model, pos, raw)` overrides both.
- **Reactive model props** through R1's tracker: `nItems` (all list models), `SingleSelection.selected`/`selectedItem`
  (already observed), `DirectoryList.loading` (new observe fact, below).

### Relation to the array path

| | array (M5) | native model (N3) |
| --- | --- | --- |
| source of truth | JS | the native model |
| per update | O(n) key reads + LIS diff -> minimal `splice`s | the toolkit's notification; no JS pass |
| native model | our id `GListModel` / `QAbstractListModel` | the app's model |
| keys | `key(item)` | identity / native position |
| sort/filter | JS (`$derived`) | native models (JS functions allowed per item/comparison) |

Use the array path for app data that lives in JS; use a native model for data that already lives natively
(files, stores shared with native code, selection models, big lists).

## Design decisions

1. **Native side holds the model, not JS.** Considered: JS listens to `items-changed` and mirrors it into the id model
   (no native change). Rejected: it keeps an O(n) id array in JS and a second native model, and every lazy-model
   batch (DirectoryList) round-trips through JS. The `model` prop costs one prop and three signals.
2. **Epochs instead of synchronous change delivery.** Change reports are posted (never `call_sync`: JS is usually the
   one mutating), so they can be overtaken by sync binds; a counter orders them without blocking.
3. **`moved` reports.** Without them a GTK sort (one `items-changed(0, n, n)`) leaves bound rows on their items with
   stale indices; the native side knows the new position (`GtkListItem:position`, Qt persistent indexes).
4. **Handles pinned by queued events (host).** A SHARED item handle in a deferred `bind` could be destroyed by the
   collection of its previous wrapper (FinalizationRegistry job) before the event was delivered. The host now counts
   handles in queued events' arguments and `destroy` leaves a pinned handle alive; the event makes a new wrapper.
   `call_sync` adopts handles in its arguments like `post_event` (NT1) did.
5. **GCompareDataFunc over list items.** `g_list_store_sort`, `g_list_store_insert_sorted`, `gtk_custom_sorter_new`
   and `gtk_custom_sorter_set_sort_func` were blocked (`callback: gpointer`). Their pointers are the model's
   GObjects: an allow list by C symbol (`OBJECT_POINTER_CALLBACKS`, in `gi.rs` and `types_facts.py`) binds them as
   objects.
6. **Shadowing functions.** `g_list_model_get_object` shadows `get_item`, so the typelib knows it only as
   `get_item`; the facts called `get_object` and every `getItem()`/`getObject()` failed at runtime. The record's
   native name is now the shadowed one; the typings gained the alias (`getItem`) the JS already had.

## Steps

1. ABI text (protocol extension, no vtable change) - done first.
2. GTK backend: `model` prop, forwarding, `bind` item + epoch, `moved`; host `call_sync` adoption + pins.
3. JS: `ListController` model mode; Node tests (`packages/native-runtime/test/lists-model.test.mjs`).
4. Real-host GTK test (`tests/gtk-types-models`, `crates/svelters-gtk/tests/lists_models.rs`).
5. Qt backend (`vlist.hpp.in`): `attach_model`, forwarding, `moved`; `tests/qt-types-lists` moved to model mode;
   Qt twin (`tests/qt-types-models`, `examples/counter-qt/tests/lists_models.rs`).
6. Typings (`packages/lists/src/index.d.ts`, checked by `packages/types/test/types/lists_usage.ts`), docs guide,
   curated model kinds (ledger), observe fact for `DirectoryList:loading`.

## Completion record

(filled in below)
