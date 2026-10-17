# M5 - virtualized lists and model-view (lane X5)

Closes A1 gap G6 (model-backed views) and, for the list kinds, the sort/filter part of G8. Working notes: `docs/notes/m5-lists.md`. Protocol text: `plans/architecture/backend-abi.md`, "Model-view protocol (M5)".

## Design

**The model lives in JS.** A JS array or store is the source of truth. The toolkit sees only a list of integer *row ids* (one per item, stable while the item's key is stable) and asks JS to bind a visible row to an id when it needs content.

### One portable widget: `virtuallist`

A backend-provided composite widget (an "extra" kind, like `gio-file`, not in the generated tables). It is an ordinary widget: it is inserted into the tree, takes the generic widget props of its toolkit base (`hexpand`, `css-classes`, ...), and speaks one protocol on both toolkits.

| member | kind | meaning |
|---|---|---|
| `splice(pos, removed, ids[])` | method | change the id list. GTK: `items-changed(pos, removed, added)`. Qt: `beginRemoveRows`/`beginInsertRows` |
| `count()` | method | native item count (tests, assertions) |
| `scrollTo(pos)` | method | scroll item `pos` into view |
| `setRow(row, widget)` | method | give row slot `row` its content widget (a handle) |
| `rowAt(pos)` | method | row slot currently bound to `pos`, or -1 |
| `selected` | prop | selected position, -1 for none |
| `selection` | prop | `none` or `single` |
| `layout` | prop | `list` or `grid` (GTK GridView; Qt IconMode) |
| `rowHeight` | prop | Qt: uniform row height in pixels (the pooled approach needs it); GTK ignores it |
| `setup(row)`, `bind(row, pos, id, gen)`, `unbind(row, gen)`, `teardown(row)` | signals | row slot lifecycle. `gen` increments on every bind of a slot, so a late (deferred) unbind of an older binding can be recognised and ignored |
| `activate(pos)`, `selection-changed(pos)` | signals | user actions |

A *row slot* is a recycled native container: a `GtkListItem` on GTK, a pooled widget placed over the viewport on Qt.

### JS side (`packages/lists`, plus two small renderer additions)

- `ListView.svelte`: `items`, `key`, `row` snippet `(item, index)`, `bind:selected`, `onactivate`, `layout`, `rowHeight`, `scrollTo()` through `bind:this`/exported function.
- Keyed diff (`diff.js`): old key list vs new key list gives removed ranges, a longest-increasing-subsequence based set of moves, and inserted ranges, each one `splice`. Never a reset. Identical key lists with replaced item objects produce no native op, only a row state update.
- Row pool: one mounted `RowHost` component per slot, mounted into an *outlet* (a realized element that is not in the window tree; renderer addition `outlet(tag)`). `bind` swaps the reactive item/index of the slot's component and calls `flushSync()` so the new content is in place when the toolkit's callback returns. `$bindable`, `bind:this` and other row-local state work because a row is an ordinary component instance that is reused.
- Row indices stay correct without native calls: JS applies each splice it issues to its table of bound slots.

### GTK

- Model: Rust `GListModel` subclass holding `Vec<u64>` of ids. `n_items` and `get_item` never call JS. Items are tiny `GObject`s created on demand that carry the id. `splice` emits one `items-changed`.
- Widget: `GtkScrolledWindow` > `GtkListView`/`GtkGridView` with `GtkSingleSelection`/`GtkNoSelection` and a `GtkSignalListItemFactory`. `setup` creates the slot, the others forward to JS through `call_sync` (bind, setup) with `post_event` fallback when JS is busy (unbind/teardown happen inside `splice` itself, so those usually go the deferred way and the `gen` guards the ordering).

### Sync callbacks: snapshot, not `call_sync` for the model

`n_items`/`get_item`/`rowCount`/`data` are answered from the native id snapshot, which JS updates inside the same host op that changes it. Reasons: (1) the toolkit calls them at any time, including while a host op is running (our own `splice` triggers `items-changed`, and GTK/Qt then query the model immediately), when `call_sync` would answer busy; (2) a layout asks for hundreds of items per frame, a JS round trip each would cost far more than the P5 numbers allow; (3) JS never needs to answer a question the snapshot cannot. Row *content* is the only thing that needs JS and it is requested lazily per visible row (bind), through `call_sync` so the content is correct before the first paint, falling back to a deferred event when busy.

### Qt

Three strategies were evaluated:

1. `setIndexWidget` per visible row: Qt owns the widget and deletes it on replacement, so no recycling, and every call walks the persistent editor machinery. Measured/documented as slow. Rejected for large lists, fine for small ones.
2. Delegate painting from `data()`: fastest, but rows are not Svelte components (no `bind:`, no events inside rows). Right for plain text/icon lists, offered through the model's `display`/`decoration` roles only if time permits.
3. **Viewport-pooled (chosen)**: a `QListView` with a native model of ids and uniform row height; a C++ helper keeps a pool of `N` row widgets as children of the viewport, `N = visible rows + 2`. Slot `s` shows position `p` with `s = p mod N`, so scrolling by one row rebinds exactly one slot. The helper positions slots from `visualRect(index)` on scroll, resize and model signals (coalesced through a zero timer). JS only hears `bind`/`unbind`.

### Change notifications

Always minimal ranges from the keyed diff. A move is remove + insert of the same id; a pure reverse is `n/2`-ish moves when LIS is short, which is the honest minimum for `GListModel` (no move signal) and Qt (`beginMoveRows` is used by the Qt backend when a remove range is immediately followed by an insert of the same ids: not implemented, listed under open issues if skipped).

### Sort and filter (G8)

Decision: **sort/filter in JS, push the result.** The app sorts or filters its array (`$derived`), and the keyed diff turns the new order into minimal `splice`s. Reasons: JS already owns the data, a native comparator would need a `call_sync` per comparison (n log n round trips, each of which must not re-enter JS), and the filtered view needs the same diff anyway. `GtkSortListModel`/`FilterListModel`/`QSortFilterProxyModel` with a JS predicate are therefore not exposed for `virtuallist`. `GtkListBox`/`GtkFlowBox` sort/filter/header funcs, which take plain widget children with no model, are covered separately (see the step list) if they fit.

## Steps

1. Plan, ABI text, notes (this file).
2. Renderer: `outlet`, `WNode.setProp`; log backend/TestHost accept the kind.
3. `packages/lists`: diff, controller, `ListView.svelte`; Node tests with the TestHost op log (minimal splices, pool recycling, scripted bind/unbind).
4. GTK backend: `virtuallist`; tests with the real host (10k, 100k, scroll, selection, handle accounting).
5. Qt backend: model + pool; tests.
6. Fixtures and benchmarks against P5.
7. Sort/filter docs; ListBox/FlowBox funcs.
8. A1-GAPS.md.

## Completion record

(filled in at the end)
