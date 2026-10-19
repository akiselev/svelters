---
title: Lists and snippets
description: Snippets, keyed each blocks and the List pattern, the virtualized ListView, grids through child attributes, and measured costs.
---

Svelte 5 replaced `<slot>` with snippets: named, parameterised template fragments that are ordinary values. Native lists use the same shape everywhere. A component takes `items` and a snippet and renders the snippet per item. Two list forms exist, and the source says which one it wants: `{#each}` builds every row eagerly, `<ListView>` is a virtualized native list.

## Snippets

- Content between a component's tags that is not a `{#snippet}` becomes the implicit `children` snippet: `let { children } = $props()` then `{@render children?.()}`.
- `{#snippet name(a, b)}...{/snippet}` written directly inside a component's tags becomes a prop called `name`.
- A snippet with parameters is the Svelte 4 "slot props": the child calls it once per item; the parent wrote the template and keeps its own scope (closures, state, handlers).
- Parameters are passed as getters, so updates are fine-grained. They must be plain identifiers: destructuring and defaults are compile errors.
- A snippet has no native parent of its own. Its widgets are created where it is rendered and torn down with the render site. When the snippet is declared in the same component, the compiler checks at each `{@render}` that its root elements fit the parent's slot.
- `<sv-slot name="x">{@render titlebar()}</sv-slot>` routes snippet content into a parent's named slot.

## The List pattern

`List` owns iteration and knows nothing about what a row looks like.

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

The code above is a pattern to copy into your app, not a shipped component. The fixtures `keyed-list`, `keyed-stress` and `if-toggle` in `fixtures/` exercise `{#each}` and `{#if}` directly.

### What a row costs

One row with a label and a button produces:

```text
create box #3
create label #4 ; set #4 label "1. task 0" ; insert #3 #4
create button #5 ; set #5 sensitive true ; listen #5 clicked on ; set #5 label "Done" ; insert #3 #5
insert #2 #3 before=end            <- the row box goes in last (post-order insert)
```

Host operations for the same list at two sizes, counted in the feature survey:

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

`{#each}` with a key keeps one native subtree per key. On an update the runtime disposes removed items, moves only the items outside a longest increasing subsequence of the old positions (`remove` plus `insert`, back to front), then builds new items front to back. Duplicate keys throw. Without a key, items are updated in place by index, and surplus items are removed from the end.

The `keyed-stress` fixture covers reverse, insert at middle, head and tail, remove, sort, rotate and filter, and agrees with the native order on both toolkits.

### Measured: 1000-row keyed list

`bench/keyed-bench` (1000 rows, each a box with a label and a button: 3000 widgets), debug `counter`, Xvfb, median of 3, milliseconds from the click to idle:

| Step | GTK | Qt |
| --- | --- | --- |
| Fill 1000 | 478 | 573 |
| Reverse | 57 | 701 |

The compiled `{#each}` is fast on reverse (no scan beyond the subsequence) and spends its fill time on per-item region and scope bookkeeping. Qt reorders are slow in the toolkit: a moved widget is detached from its `QBoxLayout`, hidden and re-shown.

Findings from the P5 measurements (`docs/notes/p5-integration.md`, taken on the since-retired renderer pipeline; the host and backends are unchanged):

1. QuickJS time dominates. Host and ABI cost is 0.1 to 1.5 percent of JS time (16 to 18 ms for 11,000 operations), so batching ABI calls is not justified by this measurement.
2. Node/V8 through the TestHost is roughly 3 to 5 times faster than QuickJS on this workload.

Guidance: plain `{#each}` plus a snippet is right for tens to a few hundred rows (forms, settings, toolbars, small lists). For larger or unbounded data use `<ListView>`.

## Virtualized lists: `<ListView>`

```svelte
<script>
  import { ListView } from '@svelters/lists';
  let rows = $state.raw([...]);
  let selected = $state(-1);
</script>

<ListView items={rows} key={(r) => r.id} bind:selected onactivate={(item, pos) => open(item)}>
  {#snippet row(item, index)}<label>{index}: {item.text}</label>{/snippet}
</ListView>
```

`{#each}` never virtualizes and `<ListView>` never builds rows eagerly. `@svelters/lists` re-exports `ListView` from `@svelters/native-runtime/lists`; the build resolves either import to the runtime's implementation. The native element underneath, `virtuallist`, is a backend kind: GTK `GtkListView`/`GtkGridView` over a `GListModel`, Qt a `QAbstractListModel` with pooled row widgets in a viewport. It cannot be written directly.

Props: `items` (array or iterable), `key(item, index)` (default: the index), the `row` snippet `(item, index)`, `layout` (`list` or `grid`), `selection` (`none` or `single`), `bind:selected` (position, -1 for none), `rowHeight` (Qt's uniform row height, default 32), `onactivate(item, pos)`. Any other prop is set on the native list unchanged (`vexpand`, `minHeight`, ...). `bind:this` gives `{ scrollTo(index), controller() }`.

Checked at compile time: a missing `row` snippet (`list_row_missing`), more than `(item, index)` parameters (`list_row_params`), content other than snippets (`list_children`), a row root that cannot go in the row outlet, a `box` (`list_row_toplevel` and the slot codes), a binding other than `selected` or `this` (`list_bind_invalid`), and a bad static `layout` or `selection` (`list_prop_invalid`).

### How it differs from `{#each}`

| | `{#each items as item (key)}` | `<ListView items key row>` |
| --- | --- | --- |
| Natives | one subtree per item | one native list; row content only for the row slots the toolkit creates (about the visible rows plus a margin) |
| Updates | keyed reconcile with minimal moves | the same subsequence diff over keys becomes `splice(pos, removed, ids)` calls; more than 16 splices collapse into one range replace. A replaced item object with the same key costs no native op: only bound rows update |
| What is reused | an item's natives live exactly as long as its key | a **row slot** is reused for different items as the view scrolls; its natives stay, `item` and `index` change |
| State in the row | per item | per **slot**: `$state` inside the row, `bind:this` targets and native widget state (focus, typed text, scroll) survive a rebind to another item. Keep per-item state in the item or a map by key |
| Context | the enclosing component's | the `<ListView>`'s, set on each row |
| Effects | inside the block's effect tree | each row is a separate root mounted into its slot; destroying the list destroys them |

The model (item count, item ids) is answered natively from a snapshot of ids and never calls JS. Row lifecycle events (`setup`, `bind`, `unbind`, `teardown`) are sync signals, so a row's content is in place before the toolkit paints it. A native selection writes `selected` without echoing it back; an app write of `selected` sets the native selection.

Timings (`bench-lists.mjs`, `lists-stress`, debug `counter`, Xvfb, median of 3, ms from the click to idle):

| Step | GTK | Qt |
| --- | --- | --- |
| Fill 1k | 84 | 13 |
| Fill 10k | 183 | 75 |
| Reverse 10k | 197 | 86 |
| Sort descending | 144 | 123 |
| Scroll to end | 22 | 11 |

A 10k `<ListView>` fills in about half the time of a 1k `{#each}`.

Sorting, filtering and sections are done in JS: sort or filter the array in a `$derived`, and the keyed diff sends minimal splices. `arrange(items, { filter, compare, group })` from `@svelters/lists/arrange` does the same for a `GtkListBox` or `GtkFlowBox` rendered with `{#each}`, returning section headers for `parent:header`.

## Grids and child attributes

Grid cells, spans, stretch factors and tab titles are child attributes: `parent:<name>={value}` on the child, checked against the slot of its actual parent.

```svelte
<grid>
  {#each order as id, i (id)}
    <label parent:column={i % 2} parent:row={Math.floor(i / 2)}>{id}</label>
  {/each}
</grid>
```

GTK has them on `grid`, `fixed`, `notebook`, `stack` and `overlay`; Qt on `grid-layout`, `box-layout`, `tab-widget` and `splitter`. See the `child-attrs` fixtures.

## Column views (planned)

:::caution[Planned]
A column view is not implemented. The feature survey prototyped one: each `Column` a row pool over a shared model that comes from context.
:::

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

Sources: `docs/notes/native-compiler/runtime-contract.md` ("Template blocks", "Virtualized lists"), `docs/notes/native-compiler/deviations.md`, `docs/notes/m5-lists.md`, `docs/notes/x3-childattr-listen.md`, `docs/notes/svelte-feature-survey.md` (sections 5 and 12), `docs/notes/p5-integration.md`, `packages/native-runtime/src/lists.js`, `packages/lists/`, `fixtures/README.md`.
