---
title: Lists and snippets
description: Snippets, keyed each blocks and the List pattern that work today, with measured costs, plus the planned virtualized list and column view.
---

Svelte 5 replaced `<slot>` with snippets: named, parameterised template fragments that are ordinary values. Native lists use the same shape everywhere. A component takes `items` and a snippet and renders the snippet per item.

## Snippets

- Content between a component's tags that is not a `{#snippet}` becomes the implicit `children` snippet: `let { children } = $props()` then `{@render children?.()}`.
- `{#snippet name(a, b)}...{/snippet}` written directly inside a component's tags becomes a prop called `name`.
- A snippet with parameters is the Svelte 4 "slot props": the child calls it once per item; the parent wrote the template and keeps its own scope (closures, state, handlers).
- Under the custom renderer each snippet is wrapped in `renderer_snippet`, which throws `snippet_renderer_mismatch` if it is rendered under a different renderer.
- `createRawSnippet` compiles but throws `invalid_snippet_in_custom_renderer` at runtime. Do not use it.

:::caution[Footgun]
A snippet declared directly inside a native element (not a component) compiles but renders nothing.
:::

## The List pattern (works today)

`List` owns iteration and knows nothing about what a row looks like. This was verified end to end on the production renderer in the survey.

```svelte
<!-- List.svelte -->
<script>
  let { items, row, empty, key = (x) => x.id } = $props();
</script>
<box orientation="vertical">
  {#each items as item, i (key(item))}
    {@render row(item, i)}
  {:else}
    {@render empty?.()}
  {/each}
</box>
```

The parent defines the row once:

```svelte
<List items={todos}>
  {#snippet row(todo, i)}
    <box>
      <label>{i + 1}. {todo.title}</label>
      <button sensitive={!todo.done} onclicked={() => (todo.done = true)}>Done</button>
    </box>
  {/snippet}
  {#snippet empty()}<label>Nothing to do</label>{/snippet}
</List>
```

`{@render row(item, i)}` passes getters, not values. When `todos[2].title` changes, only that row's label effect reruns: one `set` operation, no row recreation, no list diff.

The code above is the survey prototype (`List.svelte`, `ListApp.svelte`); copy it into your app. The fixtures `keyed-list`, `keyed-stress` and `if-toggle` in `fixtures/` exercise `{#each}` and `{#if}` directly.

### What a row costs

One row with a label and a button produces:

```text
create box #3
create label #4 ; set #4 label "1. task 0" ; insert #3 #4
create button #5 ; set #5 sensitive true ; listen #5 clicked on ; set #5 label "Done" ; insert #3 #5
insert #2 #3 before=end            <- the row box goes in last (post-order insert)
```

Host operations for the same list at two sizes:

| Step | 3 items | 1000 items |
| --- | --- | --- |
| mount | 36 ops | 10,006 ops (3,002 create, 3,002 set, 3,001 insert, 1,000 listen) |
| `todos[1].done = true` | 1 `set` | 1 op |
| rename one item | 1 `set` | 1 op |
| native `clicked` on a row | 1 `set` | 1 op |
| insert at index 1 | 12 ops | 1,009 ops (1,002 `set`: every later row's `{i + 1}` label changes) |
| remove at 0 | 7 ops | 1,004 ops (same reason) |
| reverse | 6 ops | 2,998 ops (999 moves plus 1,000 label sets) |
| clear (`{:else}`) | 15 ops | 4,003 ops |

Showing the index in a row makes every insert and remove touch every following row. That is inherent and the same in the DOM.

### Keyed lists and reordering

Keyed moves are `remove` plus `insert` of a parented node. The P5 work replaced the renderer's `indexOf`/`splice` child arrays with a doubly linked child list. Reverse of N rows through the bare renderer on the TestHost, before and after: 1000 rows 212 ms to 46 ms, 2000 rows 843 ms to 65 ms; 10,000 rows 790 ms (the survey had measured about 85 s before the fix, through Svelte).

The `keyed-stress` fixture covers reverse, insert at middle, head and tail, remove, sort, rotate and filter, and agrees with the native order on both toolkits. It fails on the DOM shim without the `before()`/`after()` index fix.

### Measured: 1000-row keyed list on QuickJS

`bench/keyed-bench` (1000 rows, each a box with a label and a button: 3000 widgets), release build, times in ms for one click:

| Step (ops) | | JS | host/ABI | toolkit calls | toolkit deferred |
| --- | --- | --- | --- | --- | --- |
| Fill 1000 (11000) | GTK | 1479 | 16 | 423 | about 0 |
| | Qt | 1907 | 18 | 226 | 2767 |
| Update every text (1000) | GTK | 320 | 2 | 23 | about 0 |
| | Qt | 334 | 2 | 12 | 108 |
| Reverse (1998) | GTK | 1741 | 4 | 144 | about 0 |
| | Qt | 1846 | 6 | 1393 | 5560 |
| Swap 2 rows (4) | GTK | 141 | 0 | 1 | about 0 |
| | Qt | 159 | 0 | 3 | 24 |
| Clear (4000) | GTK | 140 | 4 | 182 | about 0 |
| | Qt | 142 | 3 | 368 | 70 |

Findings from `docs/notes/p5-integration.md`:

1. QuickJS time dominates. Host and ABI cost is 0.1 to 1.5 percent of JS time, so batching ABI calls is not justified by this measurement.
2. Reverse is quadratic inside Svelte's own `reconcile` in the keyed `each`, not in svelters. It costs about 315 ms per 1000-row reverse on V8 and 1.8 s on QuickJS. Swap and update avoid that path (140 to 330 ms).
3. Qt reorders are slow in the toolkit: the `remove` descriptor reparents the widget to null after `QBoxLayout::removeWidget`, so every moved row is hidden and re-shown. A move within one layout should skip the detach (M5). GTK moves cost 144 ms for the same operation.
4. For comparison, Node/V8 through the TestHost: fill 450 ms, update 330 ms, reverse about 410 ms, swap about 120 ms, clear about 100 ms per step. QuickJS is roughly 3 to 5 times slower on this workload.

Guidance: plain `{#each}` plus a snippet is right for tens to a few hundred rows (forms, settings, toolbars, small lists). For larger or unbounded data use the virtualized list below once it exists.

## Virtualized lists and model-view (planned, M5)

:::caution[Planned]
Virtualized lists are milestone M5, after M4 (native semantics). What follows is the design validated by a prototype in the survey, not shipped code.
:::

Toolkits with virtualization ask for rows when they need them and recycle row widgets:

| Toolkit | Protocol | Recycling unit |
| --- | --- | --- |
| GTK4 `GtkListView`/`GtkGridView` | `GListModel` plus `GtkSignalListItemFactory`: `setup`, `bind`, `unbind`, `teardown` | `GtkListItem` and its child |
| Qt Widgets | `QAbstractItemModel` plus a delegate that paints rows. No widget-recycling list view exists. | none built in |
| WinUI 3 `ItemsRepeater` | `IElementFactory.GetElement`/`RecycleElement` | `UIElement` |
| AppKit `NSTableView` | `tableView(_:viewFor:row:)` and a reuse queue | `NSTableCellView` |
| egui | immediate mode: `ScrollArea::show_rows(range)` | none |

All of them reduce to four events on one list widget: setup(slot, container), bind(slot, position), unbind(slot), teardown(slot), plus a model-change notification going the other way.

### Design

Authoring is identical to the non-virtual list, so switching is a one-word change:

```svelte
<ListView items={todos}>
  {#snippet row(todo, i)}
    <box><ThemedLabel text={`${i + 1}. ${todo.title}`} />
         <check active={todo.done} ontoggled={() => (todo.done = !todo.done)} /></box>
  {/snippet}
</ListView>
```

Implementation: `mount()` a tiny `RowHost` component per native row slot into an adopted host widget, and recycle by rewriting a `$state` slot object on `bind`. Each slot is its own Svelte root: created on `setup`, re-pointed on `bind`, unmounted on `teardown`.

```svelte
<!-- RowHost.svelte: one per native row slot -->
<script>
  let { row, slot } = $props();              // slot = $state({ item, index })
</script>
{#if slot.item !== undefined}{@render row(slot.item, slot.index)}{/if}
```

What the prototype needed:

- Renderer: `adopt(h)`, a mount target that wraps a widget the host created. JS never creates or destroys the adopted widget.
- A `factory` delivery class: GTK expects the row to be filled when `setup`/`bind` return, so ops from these handlers apply immediately, with `flushSync()` allowed.
- A JS-to-host model notification: `n-items` plus `items-changed(pos, removed, added)`. The JS array is the model; the host keeps only a snapshot of keys and never calls JS per `get_item`.
- Context crosses the separate mount with `mount(..., { context: getAllContexts() })`.

Prototype numbers (simulated `GtkListView`, 1000 items, 21 visible rows):

| Step | Host ops |
| --- | --- |
| mount | 10 |
| `setup` plus `bind` of 21 slots | 252 (12 per row) |
| scroll by 5 | 5 (`set label` only) |
| edit a visible item | 1 |
| edit an offscreen item | 0 |
| shrink viewport, `teardown` 3 slots | 12 |

Cost scales with visible rows, not with list length.

Qt has no recycling widget view. The plan: text and icon rows use `QAbstractItemModel` roles with no per-row Svelte; widget rows use a host-side virtualizing scroll area that speaks the same four-event protocol.

## Column views (planned, M5)

A `GtkColumnView` column has its own factory, so each `Column` is a row pool over a shared model that comes from context. Prototyped in the survey:

```svelte
<ColumnView items={people}>
  <Column title="Name" expand>
    {#snippet cell(p)}<label>{p.name}</label>{/snippet}
  </Column>
  <Column title="Age">
    {#snippet cell(p)}<spin-button value={p.age} onvalue-changed={(v) => (p.age = v)} />{/snippet}
  </Column>
</ColumnView>
```

Editing `people[1].age` produces one `set` in the right cell. Sorting and filtering live in JS (a sorted array) or in native sort and filter models fed by keys.

## Static grids (planned, M4)

`<Grid><Cell row={0} col={1} colSpan={2}><Entry/></Cell></Grid>` needs a proxy element `sv-item` in the renderer and a `set_child_attr(parent, child, slot, attr, value)` ABI operation. Neither exists today.

Sources: `docs/notes/svelte-feature-survey.md` (sections 5 and 12), `docs/notes/p5-integration.md`, `docs/notes/signals-and-slots-semantics.md` (section 2.4), `fixtures/README.md`.
