# SKELTER-widgets-views: skelter-widgets `views` (scrolling, layers, menus, popups, lists, trees)

Lane WIDGETS-VIEWS of skelter batch 2 (`plans/cloud/presto-b2/LANE-widgets-views.md`). Branch
`ci/presto-widgets-views`, from `ci/presto-base`, with `ci/presto-base` merged up to 8a810ca9 (skelter-anim,
skelter-compositor, skelter-core with CORE-3's hooks and fast channel, skelter-text, the WIDGETS-BASIC substrate, MACOS's native menu API). Session `session_01UQTt4dFy9hz4VEv6q2vmpg`.

## What landed (`crates/skelter-widgets/src/views/`)

| File | What |
|---|---|
| `common.rs` | `ms`/`secs` (policy times as spans), `platform`, `emit` by signal name |
| `mod.rs` | `register_all`: the 19 view kinds through WIDGETS-BASIC's `host::register`; each kind declares the `::part`s it reads (`Kind::parts`) and, when it places its children, `Kind::places_children` |
| `placement.rs` | popup placement: side/align, gap, flip, shift, fit, anchor width (svelters-egui `interact/anchor.rs`) |
| `scroll.rs` | `Scroller` (offset, bars, overlay fade, thumb drag, track page/jump with repeat, keys, a11y, reveal), `ScrollLatch` (**latching per gesture**: a phased touchpad gesture and its OS momentum stay on one scroller and never chain mid-gesture; plain wheels latch per burst, skelter-anim's 300 ms; `overscroll-behavior: contain`; the gesture timing is `skelter_anim::scroll::ScrollLatch`, the chain choice is local), `ScrollArea` kind (curves, flings, velocity and per-OS defaults from `skelter_anim::scroll`; smooth wheel on Windows/Linux, 1:1 touchpad, own fling on Linux, OS momentum elsewhere; each step hands the offset to the host, `EventCx::scroll_to`, which writes it to the document with core's `scroll_fast` and emits the common `scroll` event; it is core's scroll container on its axes through `theme/base.css`; stale JS offset writes rejected), `Scrollbar` kind |
| `panel.rs`, `split.rs` | `Panel` with an inner-edge sash; `SplitView` (native sash drag after the drag threshold, Escape cancels, keys, a11y, live or on-release commit), `Sash` |
| `virtual_list.rs` | `RowHeights` (uniform, then Fenwick prefix sums), `KeySelect` (none/single/multi/toggle/range, typeahead), the M5 row protocol as `RowRequest` (setup/bind/unbind/teardown), the RowPool plan (reuse free slots, bind only new rows, pool limit), splice with scroll anchoring, `VirtualList` kind |
| `tree.rs` | `TreeView` over the list: model with lazy children, flatten, expand/collapse splices, Right/Left/`*`, disclosure presses, drag-expand delay, TreeItem levels |
| `overlay.rs` | the per-window overlay stack (architecture §14): bands (adorner < window < popup/menu/modal < toast < tooltip < drag image), click-through, hit routing with modal blocking, light-dismiss chains, Escape one level, focus restore, `LayerDelta`s |
| `popup.rs`, `tooltip.rs`, `toast.rs`, `modal.rs`, `inner_window.rs` | popup/popover kind; tooltip timer (delay, follow-on, suppression); toast host (queue, max visible, timeouts with hover pause, actions, thread-safe `ToastQueue`); modal (centred, backdrop cancel, Tab cycling); inner window (title drag with threshold, 8-way resize with minimum, close/collapse, raise, clamp) |
| `menu_model.rs`, `menu.rs`, `select.rs` | menu data and pick logic, `MenuModel::to_menu_desc` (skelter-platform's native `MenuDesc`, behind the `native-menu` feature), `menu-bar`'s `native-bar` prop (no layout, no input while a native bar shows the menus) and `MenuBar::menu_activated` (a native pick applied as a click), `MenuNav` keyboard navigation (skip separators, wrap, Home/End, Right/Left across submenus and bar menus, Escape levels, typeahead, mnemonics, Alt/F10), `menu-bar`/`context-menu` roots with submenu delay and safe triangle, entry kinds; `select` with per-OS closed-arrow behaviour |
| `status_bar.rs`, `dialog_buttons.rs` | section order, timed messages, status tips; platform button order (Windows, KDE, GNOME, macOS) |

Tests (written, **not run** locally, per the batch rule): placement, scrolling and latching, the offset
handed to the host, stale writes, wheel curve/fling/velocity, virtual-list recycling and splices,
selection keys, tree navigation, overlay dismiss chains and modal blocking, tooltip/toast timing, inner
window drag/resize, menu keyboard navigation, submenu delay, select per OS, dialog button order,
registration of every view kind, record goldens, the native-bar switch and native picks, and (under
`--features native-menu`) the native menu description. CI runs them (the `native-menu` test only with
that feature).

Every kind is written against WIDGETS-BASIC's kind interface (`crate::shim::core::Kind`, `kind_props!`,
`interact::Policy`) and looks through UA tables over `paint::StyleView` (theme tokens, CSS-overridable per
kind and `::part`); skelter-core sees them through `host::register`. The lane's first shim, bridge and
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
- Scroll offsets: core's `scroll_fast` always sends a fast commit; whether the compositor may move the
  content before the commit (every listener passive) is core's to decide (it computes `passive` per
  scroll node). The views no longer track listeners.
- Scroll animation is sampled on the UI thread via timer events (~16 ms); it should become an
  `AnimationSpec { property: ScrollOffset }` the compositor runs. Keys use Chromium's constant 150 ms curve,
  `scrollTo` its programmatic curve. No macOS rubber band (skelter-anim has `RubberBandReturn`; `Scroller` clamps and needs overscroll state), no edge shadows, no drag auto-scroll. A wheel step during a fling starts a fresh smooth scroll without the fling's velocity (skelter-anim's flings do not retarget). Flings use 96 ppi (no display ppi in the kind contexts); wheel detents assume 3 lines (no Windows lines-per-notch in the policy).
- Modal: no resize, no default/cancel button routing. Toasts: no rich text or declarative children.
  No exit transitions. Inner-window moves report native prop changes, no fast commit.
- Menus: no `menu-button`, no shortcut matching while closed, focus restore left to the focus model,
  `activate` not cancelable.
- Virtual list: no row DnD (lane TIER1's `dnd`), no grid layout, no deferred-bind retry, no Rust row
  sources.
- Pane layout: `split-view` lays its panes out itself through the container hooks (`layout_style` a
  flex row/column with the sash lines as gaps; `child_layout` gives each pane its `weights` share:
  `sizes` as grow weights, else `divider-position` fixed for the first of two, else the last laid sizes,
  else equal; `prepare` takes the laid sizes and sashes). Pane CSS `min-*`/`max-*` bind in taffy but not
  in `shares`, so sashes can be off by the clamp until the next drag. `panel` fixes its size in
  `layout_style`, keeps the separator clear with `chrome_padding` and takes its box in `prepare`.
  `inner-window` places its content in the body (absolute) and hides it while collapsed;
  `modal` and `popup` hide their content while closed. Still external: `StatusBar::set_message_area`,
  `VirtualList::set_viewport`/`plan`/`take_reveal` (from the enclosing scroll-area), `Modal::content`.
- **Hooks adopted (CORE-3):** `child_layout`/`layout_style`/`prepare` (above); `chrome_padding` (panel);
  `prop_state` `:open` (popup, modal, context-menu, select) and `:placeholder-shown` (select); light
  dismiss (`light_dismiss` + `Event::OutsidePress`, closing with reason `outside-press`, the overlay
  stack's name for it) on popup (open, light-dismissable, not modal), menu-bar/context-menu (a menu open
  or the bar highlighted; the press goes on) and select (list open; a press in the list is not outside);
  `emit_sync` for modal `cancel` (the modal closes unless a listener prevents it). Not adopted:
  `children_inert` (hidden content takes no input anyway), `focus_anchor` (it names an `anchor`
  string, the views' openers are node ids), peer groups (no view is a peer set), `emit_sync` for menu
  `activate`/select `changed` (the pick is applied first; moving it after the sync emit touches the
  model's pick logic), tooltip light dismiss (the tooltip kind is driven by its host's `TooltipTimer`
  input, which already hides on press). No `:collapsed` state bit exists for `inner-window`.

## Requests (to skelter-core, WIDGETS-BASIC's kind interface and `host`, or the runner)

1. Wheel gesture `phase` and `momentum` through skelter-core's `WheelInput` and `host` (per-gesture
   latching needs them; without them touchpads latch per burst).
2. `RecordCx` with the time and the policy; `interact::Policy` with `reduced_motion`.
3. A virtual-children hook in the kind interface/`host` (`skelter_core::VirtualChildren` is implemented
   by `VirtualList` and `TreeView`), and a way to attach per-row AccessKit nodes.
4. `Copy` on `shim::core::Styles` (menu-bar copies it field by field to paint).
5. Confirm `host`'s `set_native` path carries `List` values (`selected-ids`, `expanded-ids`).
6. **Consolidation 2, the runner (`crates/skelter`)**: call `PlatformCx::set_menu_bar` with the menu-bar
   kind's `MenuModel::to_menu_desc()` (build with `native-menu`), set the kind's `native-bar` prop from
   the result (true: the native bar took the menus), route `AppHandler::menu_activated(id)` to
   `MenuBar::menu_activated` (it needs the kind's `EventCx`: a document entry that runs a kind method
   with one, or a core event for it), and send the bar again after each pick so check and radio marks
   follow.

Done: the fast channel (core's `EventCx::scroll_fast` through `host`, replacing the views' own queue);
timers (host forwards `wake`); the container hooks; `parts`/`places_children` read from each kind
(WIDGETS-BASIC's `Kind::parts`/`places_children`, which the views now declare).

## Decided (coordinator, 2026-10-25)

1. Wheel latch timeout: skelter-anim's 300 ms stays; checking it against Chromium's source is queued for
   consolidation 2.
2. Inner windows live in the overlay stack.
3. The macOS native menu bar is lane MACOS's, fed from `MenuModel` (`to_menu_desc`); skelter's `menu-bar`
   shows itself only when no native bar took the menus (`native-bar`, set by the runner). Done.
4. Disabled menu rows are reachable by default; `disabled_focusable = false` skips them.
5. The platform lanes emit `AltLeft`/`AltRight` press and release (for the lone-Alt rule).
6. `scroll-area` emits the common `scroll` event (through `host`); its own `scroll` signal is gone. Done.
7. Menu roots stay out of the Tab order.
8. Popup `anchor` stays one record prop.

## Cross-lane edits

- `crates/skelter-widgets/src/shim.rs` (WIDGETS-BASIC): `core::EventCx::scroll_to` (additive field) so a
  kind can hand its scroll offset to the host.
- `crates/skelter-widgets/src/host.rs` (WIDGETS-BASIC): `run()` forwards `scroll_to` to core's
  `EventCx::scroll_fast` and emits the common `scroll` event (`scrollLeft`, `scrollTop`, as core's wheel
  default does).
- `crates/skelter-widgets/src/theme/base.css` (WIDGETS-BASIC): `scroll-area` gets `overflow-x`/`overflow-y`
  by its `direction`, which makes it core's scroll container on those axes.
- `crates/skelter-widgets/src/shim.rs` + `host.rs` (WIDGETS-BASIC): `Kind::folds_elements` (additive):
  with `folds_children`, the host folds the children in as element records (`{tag, anchor, props,
  children}`) instead of text runs; `menu-bar` and `context-menu` build their `MenuModel` from them
  (`Menus::sync_elements`, ids = `anchor`, else the position path), so a real document's menus reach
  the bar and `to_menu_desc` (APP-TEST question 12).
- `crates/skelter-widgets/src/host.rs` (WIDGETS-BASIC, bug fix): `Hosted::prepare` now calls the widget's
  own `prepare` before the dry recording; before, no kind's `prepare` ran in a document (split-view,
  panel, menus, select and toast lay out there).
- `crates/skelter-widgets/Cargo.toml`: optional `skelter-platform` dependency behind the `native-menu`
  feature (skelter-platform pulls winit).
- `skelter-base` unchanged.

## Not run

No tests were run locally (batch rule; the last filtered run was skipped at the usage limit);
`cargo check -p skelter-widgets --all-targets` (with and without `--features native-menu`) was clean (no
skelter-widgets warnings) at each commit. CI checks without the feature, so
`a_bar_built_from_elements_describes_the_native_bar` and `the_model_describes_the_native_bar` run only
with `cargo test -p skelter-widgets --features native-menu`. CI results: see the report to the coordinator.
