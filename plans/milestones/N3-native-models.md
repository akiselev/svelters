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

Branch `worktree-agent-afa611f6f60aa462b`, pushed as `ci/n3` (not merged).

### Commits

| commit | what |
| --- | --- |
| 0cb1bb59 | ABI text; GTK model mode (`model` prop, forwarding, `bind` item + epoch, `moved`); `ListController` model mode + Node tests; host: `call_sync` adopts handles, queued events pin their handles; GCompareDataFunc over list items; shadowing fix (`get_item`); `DirectoryList:loading` observe fact; `tests/gtk-types-models` + `lists_models.rs` |
| 2841e85c | Qt model mode (`vlist.hpp.in` `attach_model`), Qt test hooks follow the shown model, `tests/qt-types-lists` in model mode, Qt twin `tests/qt-types-models` + `examples/counter-qt/tests/lists_models.rs` |
| 816b9457 | `@svelters/lists` typings, aliases in the GTK typings, six curated model kinds |
| dff8ccdc | ledger refresh (gtk), guide "Native models in lists", this file |
| 3a59147b | coverage/semantics regenerated for the observe fact; `@svelters/types` test uses `get_item` |
| 854840a0 | test harness `has_prop` asks the running class (GTK 4.14 has no `FilterListModel:watch-items`) |

### Acceptance

- **10k-item `Gtk.DirectoryList` scrolls on the real host** (`svelters-gtk::lists_models`): the test generates 10,000
  files in a temp directory and types the path into the app. It waits for `n=10000 ... loading=false` in a label
  (reactive `nItems` and `loading`, no mirror). Scrolling to the end, the middle and the start keeps 205 row slots.
  That is the same count as the array path; it is M5 open item 7, GTK's first-layout pool.
- **`Gio.ListStore` mutated from JS updates rows minimally.** Each JS mutation raises exactly one native
  `items-changed` (asserted): fill `(0,0,10)`, insert `(2,0,1)`, remove `(0,1,0)`, splice `(1,2,3)`, `sort`
  `(0,11,11)`. Bound rows keep their item and follow it: after the insert, the row that showed `2 S2` shows `3 S2`;
  after the sort, rows show their new index through `moved`.
- **A filter/sort chain works.** `FilterListModel(SortListModel(store, CustomSorter.new(fn)), CustomFilter.new(fn))`:
  initial rows, an append to the base store, and a refilter (`filter.changed('different')`). Reactive item getters:
  a native `Adjustment.set_value` on an item re-renders its row. An app-owned `SingleSelection` is shown as is and
  its `selected` is reactive.
- **Qt uses the same contract and passes.** `counter-qt::host_types native_models_feed_lists` now runs in model mode,
  with two `<ListView items={model}>` and no JS copy of the rows. `counter-qt::lists_models` covers a
  `QStringListModel` mutated from JS: reset, `I 2 1;C 2 1;`, `R 0 1;`, `C 0 1;`, and `layout;` for a sort, each
  asserted. It also covers a `QSortFilterProxyModel` chain, activation and selection, and 10k/100k models with
  12 pooled rows.
- **The TextBuffer no-mirror example passes on master (R1).** `fixtures/gtk-types` and `fixtures/qt-types` (op logs),
  plus `svelters-gtk::host_types the_gtk_types_fixture_runs_on_the_real_host` and
  `counter-qt::host_types qt_types_fixture`: a native edit shows up as `19 characters` with no `$state`.

### Gates (at 854840a0 unless noted)

- `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast`: **2352 passed, 0 failed,
  7 skipped**. The same count at 3a59147b, before the last edit.
- `python3 tools/metadata/ledger.py refresh --toolkit gtk` and `--toolkit qt` (at 816b9457): gtk 2709 facts,
  2734 tests, 0 manual and 0 inferred not passing, 5 parked (the same 5 as before). qt 1252 facts, 2361 tests,
  0 / 0, 0 parked. `ledger.py check`: gtk 580 manual verified (+30: six curated kinds × 5 groups), inferred 2129
  (+1: `DirectoryList:loading`); qt 634 / 631; appkit 79 / 491; winui 60 / 480 (8 parked).
- `pnpm -r build` (at 3a59147b; no JS changes after it): ok.
- `pnpm test`, including `test:meta`, at 3a59147b: ok. Pytest 140 passed / 1 skipped. Native-runtime Node tests 88/88
  (3 new model-mode tests). `@svelters/types` 19/19 plus strict `tsc`, which now includes `lists_usage.ts`.
  `ledger.py check` reran after 854840a0: ok.
- `python3 tools/docs/build_ir.py` then `pnpm docs:test`: unittest 11 OK (1 skipped), site tests 17/17.

**CI on `ci/n3` @ 854840a0:** js 39585932504, linux-gtk 39585932540 (GTK 4.24: 2163/2163; GTK 4.14: 1920/1920 with
`counter::integration bind_this_gtk` flaky once, passed on retry), linux-qt 39585932631 (96/96), macos-appkit
39585932533, windows-winui 39585932625. All five succeeded. At 3a59147b, linux-gtk failed on GTK 4.14
(`types_verify gtk_filterlistmodel_props`: `watch-items` needs 4.20); 854840a0 fixes that.

### Performance (debug test profile, shared machine; ms from the click to the condition, 15 ms polling)

Same run, same fixture, native model vs JS array (GTK: `lists_models.rs`; Qt: `lists_models.rs`):

| step | GTK native | GTK array | Qt native | Qt array |
| --- | --- | --- | --- | --- |
| 10k: first rows shown | 975 (StringList) | 741 | 44 | 346 |
| 100k: first rows shown | 963 | 1362 | 266 | 2052 |
| 10k scroll end / mid / start | 423 / 353 / 248 | 124 / 101 / 23 | 20 / 23 / 24 | 16 / 16 / 19 |
| 100k scroll end / mid / start | 292 / 281 / 266 | 124 / 101 / 84 | 17 / 16 / 16 | 19 / 16 / 16 |
| DirectoryList 10k: loaded (`n=10000`, `loading=false`) | 648 | | | |
| DirectoryList 10k scroll end / mid / start | 274 / 235 / 257 | | | |
| row slots while scrolling | 205 | 205-206 | 12 | 12 |

- A native model's cost does not grow with its size: 10k and 100k fill in the same time, while the array path
  pays an O(n) keyed diff (100k: 1.4 s GTK, 2.1 s Qt).
- GTK scrolling in model mode costs more per rebound row than the array path (about 2-3x here). Each bind adopts
  the item's handle, creates or looks up its wrapper, reads `item.string` natively, and GTK's position notifies
  post `moved` events. The cost is per visible row, not per item. Qt scrolling is cheap in both modes.

### ABI

No `svl_backend`/`svl_host` change. The `virtual-list` widget protocol gained, additively, a `model` prop, two
trailing `bind` arguments (`epoch`, `item`), and the `items-changed`, `data-changed` and `moved` signals. Id mode is
unchanged: the id-mode op logs did not change, and model mode listens to the new signals only when it is entered.
The host gained two behaviours: `call_sync` adopts handles in its arguments, and handles in queued events are
pinned against `destroy`. Text: `backend-abi.md`, "Native models (N3)".

### Shared files touched outside the lane

- `packages/types/test/core.test.mjs` (fake host answers `get_item`).
- `packages/types/test/types/{tsconfig.json,lists_usage.ts}`.
- `tools/metadata/gtk/{types_facts.py,gen_types.py,test_types.py}`, `metadata/overrides/gtk/types/gtk-types.yaml`,
  `metadata/rules/gtk.yaml`. This also regenerated the GTK typings, facts and docs cards, coverage, and semantics.
- `crates/svelters-host/src/core.rs` and `core/ops.rs`.
- `crates/svelters-gtk/src/{gi.rs,testing.rs}`.
- `tools/metadata/qt/templates/backend.cpp.in` (test hooks).
- Not touched: `packages/types/src/core.js`, `types_pkg.py`, `vite.js`.

### Open items

1. **Callbacks before `run` (GTK):** the GTK backend learns the host pointer only in `run` (or `attach_host`), so a JS
   callback invoked while the bundle mounts (a `CustomFilter` evaluated at construction) gets "no host" and returns
   the zero value. The tests call `attach_host` first, as `host_types`/`common` already did. A real app that builds a
   filter model at startup would see an empty model until the next change. This belongs with the host start-order
   change (STATUS "Next" 2).
2. GTK model-mode scrolling is about 2-3x the array path per rebound row: wrapper and handle per bind, and `moved`
   events for every position notify. Possible fixes: report `moved` only for bound rows whose position really changed,
   and cache wrappers per item.
3. `itemClass` is needed for items made natively: there is no runtime class registry, by design, to keep tree
   shaking. A registry filled by imported classes would remove it.
4. Qt's `rowCount()` has no change signal; `tests/qt-types-lists` keeps a count-only `$state` updated from the row
   signals. A reactive `rowCount` (`observedSignal`) would remove it.
5. Qt trees (child rows) and columns beyond 0 are not shown; GTK `TreeListModel` works as a flat `GListModel`
   (expanders not rendered).
6. `Gio.ListStore.find_with_equal_func(_full)` stay blocked (`GEqualFunc` has no user-data pointer to tell the item
   pointers apart; the allow list is for `GCompareDataFunc`).
