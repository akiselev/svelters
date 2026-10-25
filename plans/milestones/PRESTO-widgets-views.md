# PRESTO-widgets-views: presto-widgets `views` (scrolling, layers, menus, popups, lists, trees)

Lane WIDGETS-VIEWS of presto batch 2 (`plans/cloud/presto-b2/LANE-widgets-views.md`). Branch
`ci/presto-widgets-views`, from `ci/presto-base`, with `ci/presto-base` merged up to 622aef24 (presto-anim,
presto-compositor skeleton, presto-core, presto-text, the WIDGETS-BASIC substrate ac7cc8c4). Session `session_01UQTt4dFy9hz4VEv6q2vmpg`.

## What landed (`crates/presto-widgets/src/views/`)

| File | What |
|---|---|
| `shim.rs` | the stand-in still needed: `fast` (a per-thread queue of `FastCommit`s, for a kind's fast-commit path; the app runner drains `fast::take()`). Scroll physics are `presto_anim::scroll` (curves, flings, velocity, page step, per-OS defaults, the latch's gesture and burst timing); `scroll.rs` keeps the chain choice (innermost that can move, `contain`, removed node, zero-delta start, cancel) |
| `common.rs` | `ms`/`secs` (policy times as spans), `platform`, `emit` by signal name |
| `mod.rs` | `register_all`: the 19 view kinds through WIDGETS-BASIC's `host::register` |
| `placement.rs` | popup placement: side/align, gap, flip, shift, fit, anchor width (svelters-egui `interact/anchor.rs`) |
| `scroll.rs` | `Scroller` (offset, bars, overlay fade, thumb drag, track page/jump with repeat, keys, a11y, reveal), `ScrollLatch` (**latching per gesture**: a phased touchpad gesture and its OS momentum stay on one scroller and never chain mid-gesture; plain wheels latch per 500 ms burst; `overscroll-behavior: contain`), `ScrollArea` kind (smooth wheel on Windows/Linux, 1:1 touchpad, own fling on Linux, OS momentum elsewhere; **passive fast commits** `FastCommit::ScrollOffset` only while every listener is passive; stale JS offset writes rejected), `Scrollbar` kind |
| `panel.rs`, `split.rs` | `Panel` with an inner-edge sash; `SplitView` (native sash drag after the drag threshold, Escape cancels, keys, a11y, live or on-release commit), `Sash` |
| `virtual_list.rs` | `RowHeights` (uniform, then Fenwick prefix sums), `KeySelect` (none/single/multi/toggle/range, typeahead), the M5 row protocol as `RowRequest` (setup/bind/unbind/teardown), the RowPool plan (reuse free slots, bind only new rows, pool limit), splice with scroll anchoring, `VirtualList` kind |
| `tree.rs` | `TreeView` over the list: model with lazy children, flatten, expand/collapse splices, Right/Left/`*`, disclosure presses, drag-expand delay, TreeItem levels |
| `overlay.rs` | the per-window overlay stack (architecture §14): bands (adorner < window < popup/menu/modal < toast < tooltip < drag image), click-through, hit routing with modal blocking, light-dismiss chains, Escape one level, focus restore, `LayerDelta`s |
| `popup.rs`, `tooltip.rs`, `toast.rs`, `modal.rs`, `inner_window.rs` | popup/popover kind; tooltip timer (delay, follow-on, suppression); toast host (queue, max visible, timeouts with hover pause, actions, thread-safe `ToastQueue`); modal (centred, backdrop cancel, Tab cycling); inner window (title drag with threshold, 8-way resize with minimum, close/collapse, raise, clamp) |
| `menu_model.rs`, `menu.rs`, `select.rs` | menu data and pick logic, `MenuNav` keyboard navigation (skip separators, wrap, Home/End, Right/Left across submenus and bar menus, Escape levels, typeahead, mnemonics, Alt/F10), `menu-bar`/`context-menu` roots with submenu delay and safe triangle, entry kinds; `select` with per-OS closed-arrow behaviour |
| `status_bar.rs`, `dialog_buttons.rs` | section order, timed messages, status tips; platform button order (Windows, KDE, GNOME, macOS) |

Tests (written, **not run** locally, per the batch rule): placement, scrolling and latching, fast commits
only when passive, stale writes, wheel curve/fling/velocity, virtual-list recycling and splices,
selection keys, tree navigation, overlay dismiss chains and modal blocking, tooltip/toast timing, inner
window drag/resize, menu keyboard navigation, submenu delay, select per OS, dialog button order,
registration of every view kind, record goldens. CI runs them.

Every kind is written against WIDGETS-BASIC's kind interface (`crate::shim::core::Kind`, `kind_props!`,
`interact::Policy`) and looks through UA tables over `paint::StyleView` (theme tokens, CSS-overridable per
kind and `::part`); presto-core sees them through `host::register`. The lane's first shim, bridge and
`draw.rs` are gone. `placement.rs` stays: `interact` has no anchor module.

## Stubbed or left out

- `activate` (menus) and `changed` (select) are not cancelable: the pick is applied before the signal.
- Menu roots are not in the Tab order (F10/Alt and the keymap reach them).
- `VirtualList::plan`/`set_viewport`/`take_reveal` have no caller yet (no virtual-children hook in
  `host` or the kind interface); per-row AccessKit nodes (`row_a11y`) are built but not attached.
- `ScrollArea` caches the last event's time and policy for the overlay-bar fade (`RecordCx` has
  neither) and has its own `reduced_motion` field (not in `interact::Policy`).
- Popup `anchor` is a record (`{node}`, `{x,y,width,height}`, `{x,y}`).
- Toast spacing is Rust constants; colours, borders, radii and shadows come from CSS.
- Scroll animation is sampled on the UI thread via timer events (~16 ms); it should become an
  `AnimationSpec { property: ScrollOffset }` the compositor runs. Keys use Chromium's constant 150 ms curve,
  `scrollTo` its programmatic curve. No macOS rubber band (presto-anim has `RubberBandReturn`; `Scroller` clamps and needs overscroll state), no edge shadows, no drag auto-scroll. A wheel step during a fling starts a fresh smooth scroll without the fling's velocity (presto-anim's flings do not retarget). Flings use 96 ppi (no display ppi in the kind contexts); wheel detents assume 3 lines (no Windows lines-per-notch in the policy).
- Modal: no resize, no default/cancel button routing. Toasts: no rich text or declarative children.
  No exit transitions. Inner-window moves report native prop changes, no fast commit.
- Menus: no `menu-button`, no shortcut matching while closed, focus restore left to the focus model,
  `activate` not cancelable.
- Virtual list: no row DnD (lane TIER1's `dnd`), no grid layout, no deferred-bind retry, no Rust row
  sources.
- Pane layout hooks: core must call `SplitView::layout`, `Panel::set_layout`,
  `StatusBar::set_message_area`, `VirtualList::set_viewport` (from the enclosing scroll-area) and apply
  `take_reveal()`.

## Requests (to presto-core, or to WIDGETS-BASIC's kind interface and `host`)

1. Timers for kinds through `host` (`EventCx::wake` is dropped by `Hosted` today): tooltips, submenu
   delay, toasts, drag-expand, the scroll animation until it moves to the compositor.
2. A fast-commit path from `EventCx` to the compositor (today `views::shim::fast`).
3. Wheel gesture `phase` and `momentum` through presto-core's `WheelInput` and `host` (per-gesture
   latching needs them; without them touchpads latch per burst).
4. `RecordCx` with the time and the policy; `interact::Policy` with `reduced_motion`.
5. A virtual-children hook in the kind interface/`host` (`presto_core::VirtualChildren` is implemented
   by `VirtualList` and `TreeView`), and a way to attach per-row AccessKit nodes.
6. `Copy` on `shim::core::Styles` (menu-bar copies it field by field to paint).
7. Confirm `host`'s `set_native` path carries `List` values (`selected-ids`, `expanded-ids`).

## Questions for the coordinator (with recommendations)

1. **Wheel latch timeout**: the views now follow presto-anim's `WHEEL_LATCH_TIMEOUT` (300 ms). Chromium's
   `kDefaultMouseWheelLatchingTransactionMs` is 500 ms from memory (unverified); recommend FRAME-ANIM
   checks it against Chromium's source.
2. **Inner windows** in the overlay stack (band below popups) or a separate layer group? Recommend the
   stack: one structure owns hit routing and raise order above the panes.
3. **macOS native menu bar** (`NSMenu`): recommend lane presto-platform-macos owns it, fed from the same
   `MenuModel`; presto's `menu-bar` hidden on macOS by default.
4. **Disabled menu rows**: reachable by arrows (svelters-egui today, ARIA) or skipped (Qt, GTK)?
   Recommend reachable by default, skipped through `disabled_focusable = false`.
5. **Lone Alt** needs `Key::AltLeft/AltRight` press/release events from the platform lanes; recommend
   confirming with them.
6. **Name overlaps**: done for `title`: `modal`/`inner-window` now have `heading` (the common `title` stays
   the tooltip). Open: `scroll-area`'s `scroll` signal vs the common DOM `scroll` event; recommend
   emitting the common one.
7. Menu roots out of the Tab order (VS Code, Windows): confirm.
8. Popup `anchor` as one record prop: recommend keeping.

## Cross-lane edits

None outside `crates/presto-widgets/src/views/` and this record. `presto-base` unchanged.

## Not run

No tests were run locally (batch rule); `cargo check -p presto-widgets --all-targets` was clean (no
presto-widgets warnings) at each commit. CI results: see the report to the coordinator.
