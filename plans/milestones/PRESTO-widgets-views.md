# PRESTO-widgets-views: presto-widgets `views` (scrolling, layers, menus, popups, lists, trees)

Lane WIDGETS-VIEWS of presto batch 2 (`plans/cloud/presto-b2/LANE-widgets-views.md`). Branch
`ci/presto-widgets-views`, from `ci/presto-base`, with `ci/presto-base` merged up to 622aef24 (presto-anim,
presto-compositor skeleton, presto-core, presto-text). Session `session_01UQTt4dFy9hz4VEv6q2vmpg`.

## What landed (`crates/presto-widgets/src/views/`, about 12.4k lines with tests)

| File | What |
|---|---|
| `shim.rs` | stand-ins for APIs the views needed before presto-core existed: a `Kind` trait over own fields (`set_prop`), `KindSpec`, `Value`, `Event`, `EventCx` (time, policy, emitted signals, change class, fast commits, wake deadline, capture, focus, cursor), `RecordCx` with `Labels`, the platform `Policy` table (svelters-egui `native::Policy`, view fields), `Palette` (theme stand-in); `scroll_physics` (stands in for `presto_anim::scroll`) |
| `bridge.rs` | `Bridge<K>`: every shim kind on presto-core's real `Kind` (spec conversion, `prop_changed`, event translation, signals by name, display list flushed in paint order with labels through `RecordCx::draw_text`, a11y); `register_all` registers the 19 view kinds |
| `draw.rs` | private recording helpers (fill, boxed, shadow, line) until WIDGETS-BASIC's `paint` lands |
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
bridge registration and value conversion. CI runs them.

## Stubbed or left out

- **Shim in use.** View logic and its tests are on `shim::Kind`; presto-core sees them through
  `bridge::Bridge`. The consolidation can port each kind to `presto_core::Kind` directly once core has
  the APIs below, or keep the bridge.
- Scroll animation is sampled on the UI thread via timer events (~16 ms); it should become an
  `AnimationSpec { property: ScrollOffset }` the compositor runs. Keyboard and programmatic smooth
  scrolls reuse the wheel curve. No macOS rubber band, no edge shadows, no drag auto-scroll.
- Labels in bridged kinds draw in the node's font and colour, and their measurement is an estimate
  (0.6 em/char) until `RecordCx` can measure (request 4).
- Modal: no resize, no default/cancel button routing. Toasts: no rich text or declarative children.
  No exit transitions. Inner-window moves mark `Property` but send no fast commit.
- Menus: no `menu-button`, no shortcut matching while closed, focus restore left to the focus model,
  `activate` not cancelable.
- Virtual list: no row DnD (lane TIER1's `dnd`), no grid layout, no deferred-bind retry, no Rust row
  sources.
- Pane layout hooks: core must call `SplitView::layout`, `Panel::set_layout`,
  `StatusBar::set_message_area`, `VirtualList::set_viewport` (from the enclosing scroll-area) and apply
  `take_reveal()`.

## Requests to presto-core (CORE lane), each a shim item today

1. `EventCx::now()` (the event's time) and access to the platform policy.
2. Timers for kinds: `EventCx::wake_at(Time)` and an `Event::Timer` delivery (tooltips, submenu delay,
   toasts, drag-expand, scroll animation until it moves to the compositor). Today: `bridge::take_wakes`.
3. `EventCx::fast_commit(FastCommit)` for native-owned writes (scroll offsets). Today: `bridge::take_fast`.
4. `RecordCx::measure_text` and `draw_text` at a given size and colour.
5. `WheelInput` with the gesture `phase` and `momentum` (presto-base `WheelEvent` has them): per-gesture
   latching needs them; the bridge passes `None` today, so touchpads latch per burst.
6. Hover enter/leave with a position; AccessKit action requests delivered to kinds.
7. The newest revision delivered to JS per node (`ScrollArea::js_saw`), for the stale-write check;
   or core's `native_owned` check alone (the bridge marks `Property`-class props native-owned).

## Questions for the coordinator (with recommendations)

1. **Wheel latch timeout**: 500 ms (Chromium `kDefaultMouseWheelLatchingTransactionMs`, from memory,
   unverified) or the notes' 300 ms (unsourced)? Recommend 500, verified against Chromium's source.
2. **Inner windows** in the overlay stack (band below popups) or a separate layer group? Recommend the
   stack: one structure owns hit routing and raise order above the panes.
3. **macOS native menu bar** (`NSMenu`): recommend lane presto-platform-macos owns it, fed from the same
   `MenuModel`; presto's `menu-bar` hidden on macOS by default.
4. **Disabled menu rows**: reachable by arrows (svelters-egui today, ARIA) or skipped (Qt, GTK)?
   Recommend reachable by default, skipped through `disabled_focusable = false`.
5. **Lone Alt** needs `Key::AltLeft/AltRight` press/release events from the platform lanes; recommend
   confirming with them.
6. **Name overlaps with common props/signals**: `inner-window`/`modal` `title` (the window title) vs the
   common `title` (tooltip); `scroll-area`'s `scroll` signal vs the common DOM `scroll` event.
   Recommend renaming the kind props to `heading` (or dropping the common `title` for these kinds) and
   letting `scroll-area` emit the common `scroll` event instead of its own.
7. `RecordCx` has no policy, so overlay vs classic bars follow the host OS; recommend adding it.

## Cross-lane edits

None outside `crates/presto-widgets/src/views/` and this record. `presto-base` unchanged.

## Not run

No tests were run locally (batch rule); `cargo check -p presto-widgets --all-targets` was clean (no
presto-widgets warnings) at each commit. CI results: see the report to the coordinator.
