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

Branch `worktree-agent-a8dce564007788244` (not merged). Notes: `docs/notes/m5-lists.md`.

- **Commits:** 6321590 lists package + renderer outlets + virtual-list fixture; 28a8e17 GTK `virtuallist` + protocol text; d6defe9 lists-stress fixture and range-replace collapse; fb33e06 GTK end-to-end test; abd7024 Qt `virtuallist` + Qt end-to-end test.
- **Delivered:** `virtuallist` on GTK (`crates/svelters-gtk/src/backend/vlist.rs`: `GListModel` of ids, `GtkSignalListItemFactory`, ScrolledWindow > ListView/GridView) and Qt (`tools/metadata/qt/templates/vlist.hpp.in`: `QAbstractListModel` of ids + viewport-pooled row widgets over a `QListView`); `@svelters/lists` (`ListView`, keyed diff, row pool); renderer `outlet()`/`releaseOutlet()`/`WNode.setProp`.
- **Svelte API:** `<ListView items key row layout selection bind:selected rowHeight minHeight onactivate>` with `{#snippet row(item, index)}`, `bind:this` for `scrollTo(i)`. Rows are ordinary component instances that are reused, so `bind:` and `bind:this` inside rows work. Use `$state.raw` for big arrays.
- **Tests:** `packages/lists` diff tests 9; fixtures `virtual-list` (scripted bind/unbind, 120 ops) and `lists-stress` (125 ops) on the TestHost; GTK `tests/vlist.rs` (native protocol, 10k) and `tests/lists_e2e.rs` (Svelte list, 10k and 100k, 35 steps); Qt `tests/lists_e2e.rs` (same 35 steps). Asserted: exact notifications (`append` is one `items-changed(10000,0,1)`, `removeMiddle` `(5,3,0)`, a move is remove + insert, rename of same keys raises none, a reverse or sort at most 4, a scattered filter collapses to one), pool size O(visible) for 10k/100k items, scroll to end/middle on 100k, selection and activation both ways, and handle accounting (hide list: `start - 1` handles; show again: exactly `start`; 3 fill/scroll/clear cycles do not grow the table).
- **Benchmarks** (debug test profile, QuickJS at -O2, shared machine at load ~35, so read them as ratios; click to condition, includes 15 ms polling; `lists_e2e -- --nocapture`):

| step | GTK ms | Qt ms |
|---|---|---|
| fill 10k (first) | 1359 | 175 |
| fill 100k | 3229 | 2002 |
| append / prepend / removeFirst (10k) | 271 / 306 / 307 | 105 / 102 / 86 |
| moveFirst | 818 | 278 |
| reverse / sortDesc (10k) | 1581 / 1049 | 198 / 341 |
| odd filter | 1819 | 203 |
| scroll to end / start (10k) | 268 / 166 | 16 / 16 |
| scroll to end / mid (100k) | 186 / 169 | 16 / 16 |
| clear | 166 | 112 |

  For comparison P5's 1000-row `{#each}` list was 1.5-1.9 s fill, 0.32 s update and 1.7-1.8 s reverse in JS alone, plus toolkit time. A 10k `ListView` fills in about the time of that 1k list and a 100k one is a few seconds; what remains is Svelte reading every item for the key (O(n) per update). Scrolling costs only the rebinds of the rows that enter. GTK numbers are higher than Qt because they include GTK layout of the whole `GtkListView` under cairo.
- **Gaps:** G6 and the model-view part of G8 recorded in `metadata/A1-GAPS.md`.
- **Open issues:**
  1. `update()` is O(n) per change (keys of all items are read). An append to a 100k list costs one pass in JS. A `push`-aware fast path (compare lengths and the last key) would make it O(1).
  2. Qt `beginMoveRows` is not used; a move is remove + insert.
  3. ColumnView/TableView/TreeView and combo/dropdown models are not covered; `virtuallist` is a single-column list/grid.
  4. Qt rows need a uniform `rowHeight`. Qt grid cells are square (`rowHeight`).
  5. A row widget keeps its stale content while unbound; hidden on Qt, off-screen on GTK.
  6. When the whole `virtuallist` dies natively, JS learns of the row outlets only through `ListController.detach` (component destroy); the outlets are not in the native tree so the death cascade does not reach them.
  7. GTK may create an oversized first pool (205 slots for a 300px view) because the first layout runs before the window has its size and while JS is still busy; GTK trims it later. A 32px placeholder per slot (`rowHeight`) limits it.
  8. `GtkListBox`/`GtkFlowBox` sort/filter/header funcs (G8) not done.
  9. Merge note: root `Cargo.toml` gains `[profile.dev.package.rquickjs-sys] opt-level = 2`; `packages/testhost/bin/svelters-test.mjs` forbids quoted `node:` specifiers instead of any `node:` text (Svelte's JSDoc contains it).
