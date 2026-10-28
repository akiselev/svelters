# SV-B2: cadatomic requests SV83–SV86, SV-OBJ4 (done; in CI)

Lane `ci/sv-b2` (2026-10-27/28), from svelters master `e01ce9b`. The requests are rows SV83, SV84, SV85,
SV86 and SV-OBJ4 of cadatomic's request table. Each item has a failing test first, then the fix. APIs are
additive and defaults are unchanged (the one exception, SV86's write past the end, is below).

| Item | Commit | What | Test |
| --- | --- | --- | --- |
| SV84 (P1) | `9c3b0ea` | An each block the host interrupted mid-update rebuilds its items | `crates/svelters-skelter/tests/keyed_each.rs` (6 tests) |
| SV84 follow-up | see the log | `{#if}`, `{#key}`, `{#await}` (and snippet/component) blocks: the same repair (coordinator, 2026-10-27) | `crates/svelters-skelter/tests/interrupted_blocks.rs` (one test per kind) |
| SV86 | `34eeb33` | A scroll-area write past the end stays at the end as content grows | `skelter-widgets` `views::scroll::tests::sv86_*` |
| SV-OBJ4 | `2c22709` | `A11yCx::service<T>()`, `Document::request_a11y(node)` | `skelter-core` `tests::a11y::sv_obj4_*` |
| SV85 | `0028cfa` | `svelters::exports::on_destroy(f)`: a native export ties a disposer to its calling component | `crates/svelters-skelter/tests/scoped.rs` |
| SV83 | `26e30b6` | `virtual-list` `auto-row-height`: rows sized from the type scale | `skelter-widgets` `views::virtual_list::sv83::*` |

## SV84: keyed `{#each}` leaving rows in the native tree

**Plain interleavings do not leak.** `keyed_each.rs` drives a keyed nested `{#each}` (groups of rows,
`{#if}` inside rows) through all → few → all → few, with the updates falling across `logic` passes and
document commits in these orders: a pass per update, two updates per pass, an update between `logic` and
the commit, several passes per commit, and a fixed pseudo-random schedule. After every commit it checks
two things: the document under the slot, and the accessibility tree folded from the commits (through
AccessKit's own consumer). All of these passed before the fix.

**Root cause: the turn-budget interrupt.** The embed interrupts any JS entry that runs over
`Budgets::interrupt` (50 ms by default). On a loaded machine, a debug build re-rendering ~40 rows can go
over that. The case that leaks is an interrupt in the flush that runs after an event handler (a job, not a
call into a root). That interrupt names no node, so `collect` reports `EmbedEvent::Error { root: None }`
and faults nothing.

QuickJS skips every `finally` on an interrupt. FX6's `recover` resets Svelte's scheduler, but the each
block stays half reconciled: its `items` list and the regions it built disagree. The block is also left
`CLEAN`, so later updates reconcile from the stale list and the rows built in between are never removed.
`sv84_an_interrupted_update_leaves_no_rows_behind` reproduces this; it failed before the fix with half
the list missing.

**Fix** (`packages/native-runtime`):
- Each blocks register in `updating` while their update runs.
- `recover` calls `abortUpdates()`. It marks every block still registered as `aborted` and makes its
  effect dirty (`invalidate` in `core/svelte.js`), so FX6's traversal schedules it.
- The block's next run first scraps every region it has (`scrap`): the effect, or the branch being built
  when the abort came (`open` records it as `b.building`). The natives are removed and destroyed, and the
  collection is rendered again from nothing.
- After an abort, the items lose their local state. That only happens after an interrupt.

**`if`/`key`/`await` blocks: fixed in this lane too** (coordinator, 2026-10-27). They switch branches
through `Branches.ensure`, which now registers its state in `updating`. On `recover`, a switch the host
interrupted is marked dirty, and `Branches.repair()` on the block's next run scraps every branch region and
resets the block's guard (`shown`, `has`, `first`), so the current branch is built again from nothing.
Snippet blocks and dynamic components (`swapping`) share it.

`interrupted_blocks.rs` has one test per kind. In each, a click switches to a branch whose three labels
each take 60 ms, under a 150 ms interrupt budget. Before the fix, each test failed: the old branch stayed
beside a half-built new one, or the block stayed stuck on the old branch.

Side finding, not chased: in that harness an `{#await p}` over `p = $derived(new Promise(...))` (the
promise read only by the block) did not re-run on its source's change unless an `$effect` also read the
source. The test sets `p` as state from the click instead. Worth a look by whoever owns `await_block`.

## The interrupt-in-flush finding (for every host)

The 50 ms `Budgets::interrupt` can fire inside the flush that follows an event handler. That flush runs as
a job, not as a call into a root, so the interrupt names no node:
- `Embed::collect` reports `EmbedEvent::Error { root: None, error.kind == ErrorKind::Interrupted }` and
  faults nothing;
- the root keeps running, with whatever that flush had half done.

FX6's `recover` resets Svelte's scheduler. Until this lane, though, each/if/key/await blocks stopped
mid-update kept their half-updated bookkeeping. That is the root of a visible class of bugs: stale or
duplicated rows, a branch that never switches. This lane's fixes repair those blocks.

A host should still log these events: they mean a debug build on a loaded machine went over budget, which
is worth seeing. A host may also raise `Budgets::interrupt` in debug builds.

## SV86: scroll-area pinned to the end

There were two bugs:
1. `scroll-y` was clamped when JS wrote it, against the content of that moment. A write in the same pass
   that adds the content was therefore clamped to the old content (or to 0 before the first layout).
2. `ScrollArea` refused every JS write once the user had scrolled. Its own stale check compares
   `native_rev > js_rev`, and nothing calls `js_saw` on skelter. The document already refuses stale JS
   writes by revision (`PropError::Superseded`), so the area's check was redundant there.

**skelter-core (new, nothing pinned by default):**
- `Document::pin_scroll_end(node, x, y)` and `scroll_pinned(node)`. A pinned axis follows the end of the
  content after every layout (`follow_scroll_pins` in `frame`, before the commit is built).
- `set_scroll_offset` unpins an axis that it moves away from the end: a wheel, a drag, a fling, a
  `scrollTo` short of the end.
- `PrepareCx::pin_scroll_end(x, y)`.

**skelter-widgets:**
- The scroll-area keeps the raw value JS wrote (`requested`). Its prepare clamps it to the content laid out
  after the write, and a write past that end pins the axis (`take_scroll_pin`, a new shim hook).
- `scroll_geometry` takes core's offset (unless an animation or a JS write is on its way), so the area
  sees a pin following its content and compositor flings.
- `write_checked` (a new shim hook, called by the host before a prop write) marks the write as current.

**Behaviour change:** a `scroll-y` write past the end now means "stay at the end". Before, it was clamped
to stale content. This is what the request asks for; no other default changes.

## SV-OBJ4

- `A11yCx::service<T>() -> Option<&T>`, as `StyleCx::service` (the services are not taken during
  `a11y_node`).
- `Document::request_a11y(node)` marks the node `Dirty::A11Y`; a removed or stale node is ignored.
- The test checks that the re-send carries only that node, and that `request_a11y` on a removed node sends
  nothing.

## SV85: component-scope hook for native exports

- **Rust:** `svelters::exports::on_destroy(f) -> bool`, plus `CallCx::on_destroy` and
  `CallCx::keep_until_destroyed(v)`. It stores `f` in the host's `AppObjects` (`add_scoped`), and returns
  false outside a call from JS.
- **Host (QuickJS):** after `call`, `callStatic` or `construct` returns, `bind_scoped` hands the new ids to
  the runtime's `globalThis.__svelters_scope_bind(ids)`. A new op `releaseScoped(id)` runs and forgets a
  disposer.
- **Runtime (`core/svelte.js`):** `__svelters_scope_bind` registers a teardown render effect under the
  active effect. That is the component's script effect, an `$effect` (released when it re-runs), or a
  block branch.
- **Outside any effect** (module top level, an event handler), or with no runtime, the disposer stays with
  the host and is dropped unrun with it.
- The test covers keyed items leaving, an `$effect` re-running, and the root unmounting.
- `svelters-skelter` gets `svelters` as a dev-dependency (test only; `Cargo.lock` gains the edge).

## SV83: rows from the type scale

`virtual-list` rows are exactly the estimate: `measured()` is only called by unit tests on the document
path, so rows are never measured. The change:

- New prop `auto-row-height` (bool, **off by default**). When on, the row estimate is
  `Theme::row_height_for(font_size)`: the density's `row-h` × max(1, computed `font-size` / 13).
  `BASE_FONT_SIZE` is 13, the size the density tables are for. A written `row-height` is then the least
  row height.
- The widgets' shim `StyleCx` gains `font_size` and `theme`, filled from core's `StyleCx` by the host.
- The list notes the type scale in `layout_style` (restyle) and applies it in `virtual_children` (before
  the realise and the layout of the same frame).
- The skelter surface (`tier1.rs`) lists the prop, and `metadata/semantics/skelter.json` was regenerated
  (`SVELTERS_UPDATE=1`).
- Callers write `auto-row-height` and drop their numbers, or keep them as minimums.

**Question for the coordinator:** should `auto-row-height` become the default for `virtual-list` (and
`tree-view`)?
- **Recommendation:** yes, in a later pin. Lists without `row-height` would go from 32 px to the density
  row (22 px), which is a visible default change, so it is not made here.
- `tree-view` does not have the prop yet: it forwards only the list's first six props. Adding it there is
  the same few lines.

## Built and run

- **Run:** the tests above, filtered; `skelter-widgets` `scroll`/`virtual_list`/`tree` and `skelter-core`
  `scroll` unit tests; `native-runtime` `blocks`/`transitions`/`runtime` node tests;
  `check-svelte-internals`; `svelters-skelter` `spec` and `vlist`.
- **Checked:** `cargo check --all-targets` on `skelter-core`, `skelter-widgets`, `skelter`, `skelter-test`,
  `skelter-gallery`, `skelter-gallery-app`, `svelters-skelter`, `svelters-host`, `svelters`,
  `skelter-egui-island` and `svelters-automation`.
- **Not run locally:**
  - the full workspace (GTK and Qt system libraries are missing in the container);
  - `svelte-check`;
  - conformance;
  - the egui/GTK/Qt suites.

  CI runs all of these.
