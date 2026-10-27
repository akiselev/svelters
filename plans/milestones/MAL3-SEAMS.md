# MAL3-SEAMS: svelters input and widget seams (SV73, S1, S2, SV78, SV79)

Lane `S-D seams` of cadatomic's MAL3 batch, branch `ci/mal3-seams`, base master `b6f0ee86`.
Coordinator: cadatomic's batch coordinator session. Every seam is generic: no host names or types.

**Status:** done; CI green on `953c48e3`, waiting for the coordinator's merge.

## What landed

### S1: pointer and wheel input filters (`skelter-core/src/input/filter.rs`)

- `Document::add_input_filter(priority: i16, owner, f)` with
  `f: FnMut(&mut FilterCx, &InputEnvelope) -> Filtered`, `Filtered::{Continue, Consumed}`;
  `remove_input_filters(owner)` removes all of an owner's filters (also from inside a filter);
  `input_filter_owners()` lists them in run order.
- Runs in `Document::dispatch` for pointer and wheel events, after hit testing (the pointer's
  capture holder, else the hit target; wheels hit-test as the wheel default does) and before
  everything else: hover, press, focus, JS listeners and `Kind::event`. Highest priority first,
  insertion order within a priority, until one consumes.
- A consumed event does nothing else; `Dispatched::filtered_by` names the consuming filter's owner
  (for a scenario recorder), and `handled` is set.
- Never during an IME composition or an in-app drag; never for keys (the key hook is their twin).
  Rust only: no JS filters.
- `FilterCx` (`#[non_exhaustive]`): `doc: &mut Document`, `window`, `target`, `local` (the target's
  local position).
- Test: skelter-core `tests::dispatch::input_filters_take_pointer_and_wheel_events_before_the_tree`.

### SV73: a pre-dispatch tap (`skelter/src/backends.rs`)

- `CoreDocument::add_pre_dispatch_tap(FnMut(&mut PreDispatchCx) -> bool)`, the twin of
  `add_event_tap`. `PreDispatchCx` (`#[non_exhaustive]`, like `KeyHookCx`): `doc: &mut Document`,
  `input: &InputEnvelope`. Taps run in the order added before core dispatches; the first that
  returns true consumes the input: no core dispatch, no post-dispatch taps, `Dispatched` reports
  `js_events: 0` and `dirty` when the tap changed the document. Every input kind, keys included.
- Exported as `skelter::{PreDispatchCx, PreDispatchTap}`.
- Test: skelter `backends::tests::a_pre_dispatch_tap_sees_the_input_first_and_may_consume_it`.

### S2: text keys as a table (`skelter-widgets/src/text/keys.rs`)

- `KeyChord { key, mods }` (Shift, Ctrl, Alt, Super; AltGr is text and not part of a chord).
- `TextAction::{Key(KeyAction), PassToIme}`.
- `TextKeymap { entries: Vec<(KeyChord, TextAction)> }`, a host layer over a platform's keys:
  `bind`, `unbind` (binds `KeyAction::Ignore`: the key bubbles), `reset` (back to the platform's),
  `get`, `resolve(cx, ev)` (the layer, else `map_key`). `TextKeymap::defaults(cx)` is the platform's
  table built from `map_mac`/`map_pc` (every chord of the keys they bind, with its action for a field
  like `cx`); `table(cx)` is the defaults with the layer applied, for a keymap editor.
- `TextPolicy::keys: Option<Arc<TextKeymap>>`. `TextPolicy` is no longer `Copy` (it holds the
  layer); callers clone it.
- `TextEditor::key` resolves through the layer; `PassToIme` takes no action there, and
  `TextEditor::route` sends such a chord to the IME (`KeyRoute::Ime`).
- `host::set_text_keymap(doc, keymap)` installs the layer as a document service; `text-edit` reads
  it on each event, as it reads the policy.
- Tests: skelter-widgets `text::keys::tests::the_text_keymap_is_the_platform_map_as_data_with_a_host_layer`,
  `text::editor::tests::a_host_keymap_layer_rebinds_unbinds_and_leaves_keys_to_the_ime`.

### SV78: `scroll-area` gets its geometry (`skelter-widgets/src/views/scroll.rs`, `host.rs`)

- Cause: `ScrollArea::set_geometry` had no caller in a document, so the scroller had no content or
  viewport; its wheel handler returned `Handled::Yes` without moving, and core's default scroll
  never ran.
- skelter-core: `ScrollGeometry { viewport, content, offset, axes }` (`#[non_exhaustive]`),
  `Document::scroll_geometry(node)` and `NodeView::scroll_geometry()`.
- The widget shim's `Kind::scroll_geometry(&ScrollGeometry)` (default: nothing); the host gives it
  before each event and each prepare while the node is a scroll container. `ScrollArea` lays out
  its scroller and bars from it (`BAR_WIDTH` 10, `MIN_THUMB` 20).
- An area whose content fits on every axis it scrolls (and has no animation running) returns
  `Handled::No` for the wheel, so the container around it scrolls.
- Test: skelter-widgets `views::scroll::tests::a_wheel_scrolls_a_laid_out_area_and_passes_when_it_fits`.

### SV79: menu entries on screen for assistive technology (`skelter-core/src/a11y.rs`, `views/menu.rs`)

- Cause: core set every node's AccessKit bounds from its layout box; menu entries are drawn by
  their root's levels and never laid out, so every entry was 0×0.
- skelter-core: `PrepareCx::place_a11y(descendant, Option<Rect>)`. A placed node's AccessKit
  bounds are the rect in the placing node's local space (its transform composes from the placing
  node's); `None` hides it. Placements apply after the prepare; a changed one re-sends the node in
  the same frame. Freed nodes drop theirs.
- The widget shim's `Kind::a11y_placements()` (paths of child indices, like `emit_at`); the host
  turns them into `place_a11y` after each prepare. `menu-bar` and `context-menu` place each entry
  at its open row or bar title, and hide it otherwise (`native-bar` places nothing).
- `Menus::sync_elements` no longer closes an open menu when its items change shape: the open
  levels, current rows, hover and bar title follow their entries by id (anchor, else position); a
  level whose container is gone closes with the levels below it.
- Tests: skelter-widgets `views::menu::tests::open_menu_entries_have_their_rows_as_accesskit_bounds`,
  `views::menu::tests::an_open_menu_stays_open_while_its_items_change`.

## Carry-overs

- `ScrollArea`'s bars follow content growth on the next event or prepare; a child that grows
  without re-recording the area leaves the bars stale until then.
- Bar width and minimum thumb are constants; the theme's `scrollbar-w` token is not read yet.
- `PassToIme` is honoured by `TextEditor::route`; core's dispatch decides `to_ime` itself and does
  not consult the field's layer.
- An AccessKit action on a menu entry (click, focus) still reaches the entry node, whose kind does
  nothing with it; picking through AccessKit is the root's to add.
- The number, hex and cell editors built by the table and pickers take `TextPolicy::from_policy`
  without the document's text keymap (their shim contexts have no services).

## Tests run locally

Only the tests above, filtered (`cargo test -p <crate> --lib <name>`), plus
`cargo test -p skelter-widgets --lib views::menu` (28), `cargo test -p skelter-core --lib a11y` (13),
`cargo test -p skelter-test --test menu` (3),
`cargo test -p skelter-widgets --lib views::scroll` (17) and `cargo check -p skelter -p skelter-core
-p svelters-skelter -p skelter-test --all-targets`, `cargo check -p skelter-widgets --all-targets`.
Not run: the full workspace tests, conformance, clippy.

## CI

All workflows green on `953c48e3` (code; the commit after it only records this): js 40546957597,
linux-gtk 40546957907, linux-qt 40546957675, macos-appkit 40546957807, skelter 40546957827,
windows-winui 40546957696. The earlier push `5eb90e2` (before SV79) was not waited on.
