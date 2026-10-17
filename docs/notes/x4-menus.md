# X4: menus and actions (working notes)

Lane X4, 2026-10-17, master 2aa2127. Plan: `plans/milestones/X4-menus.md`. ABI text: `plans/architecture/backend-abi.md`, "Non-widget kinds: menus and actions (X4, no ABI change)".

## Survey (before coding)

- The Qt generator is widget-centred: slots cast children with `qobject_cast<T*>` but the generated tests assume `QWidget` parents (`isAncestorOf`). QAction is QtGui, so there was no `metadata/qt/QAction.json`: the ingest filter was `module == qtwidgets`.
- GTK extras (`gio-file`, `virtuallist`) are hand-wired in `backend.rs` (create/destroy/set_prop/get_prop/call/listen/resolve/describe). The menu module follows the `virtuallist` pattern.
- GTK's `separator` tag is `GtkSeparator`, so the menu separator is `menuseparator`.

## Svelte syntax

```svelte
<box orientation="vertical">
  <menubar>
    <menu label="File">
      <action label="Open" icon="document-open" shortcut="Ctrl+O" onactivate={open} />
      <action label="Autosave" checkable bind:checked={autosave} />
      <menuseparator />
      <menu label="Recent">{#each recent as r (r)}<action label={r} onactivate={() => open(r)} />{/each}</menu>
    </menu>
    <menu label="Size">
      <action label="Small" group="size" bind:checked={small} />
      <action label="Large" group="size" bind:checked={large} />
    </menu>
  </menubar>
  <toolbar><action icon="edit-copy" label="Copy" onactivate={copy} /></toolbar>
  <button label="Right click"><contextmenu><action label="Paste" onactivate={paste} /></contextmenu></button>
</box>
```

`bind:checked` needs `checkable` (or `group`). Typings: `packages/renderer/types/menus.d.ts`.

## GTK (hand-written `crates/svelters-gtk/src/backend/menus.rs`)

- A node tree per handle; every structural or presentational change re-syncs the whole tree of its root: the action group (`svl` prefix), the `GMenu` models (an unchanged model is left alone: a signature string per container), the toolbar buttons and the shortcut controller.
- Surprises:
  - A handler connected to `GSimpleAction::activate` replaces GIO's default toggle (`g_simple_action_activate` only toggles when no handler is pending), so check actions toggle in our own handler and radio actions call `change_state(param)`.
  - `GtkShortcutTrigger::to_str` normalises `<Control><Shift>p` to `<Shift><Control>p`.
  - A bare attribute reaches a host without descriptors as the text `""`; the backend accepts `""`/`"true"`/`"false"` for booleans and publishes bool descriptors for the menu props so the renderer coerces them.
  - The test harness named signals by the generated `SIGNALS` table; menu signal ids (`0x4431`...) are named by `menus::signal_name`.
- Shortcuts: a global-scope `GtkShortcutController` on the root widget with one `GtkShortcut` per `shortcut`, whose `GtkCallbackAction` activates the action in the root group (a disabled action is not run). The model item gets an `accel` attribute only for display. Real key injection (xdotool/XTEST under a shared Xvfb without a window manager) did not reach the window (no focus) and is unreliable with parallel tests, so the test runs the shortcut's action the way the controller does.
- Context menu: `GtkPopoverMenu` parented to the target widget (`connect_destroy` unparents it), opened by a secondary-button `GtkGestureClick` or `popup(x, y)`. Its action group sits on the popover; the shortcut controller on the target widget.
- Tests: `crates/svelters-gtk/tests/menus.rs` (10), `host_menus.rs` (real host + `fixtures/menus`), oplog replay of the fixture (`oplog_replay.rs`, menu events are produced through the action group), native menu-item clicks (`GtkModelButton::clicked` found in the menubar's popovers).
- Ledger: facts of the hand-written kinds are in `metadata/extras/gtk-menus.yaml` (`facts:`), run and fingerprinted by `tools/metadata/ledger.py` (tests named `<binary>::<fn>`). They are not in `generated/facts.json`, so the coverage tables do not list them.

## Qt (generator + `templates/menus.hpp.in`)

- Ingest: `QAction` and `QActionGroup` (QtGui) added to `tools/metadata/qt/ingest.py` (`QTGUI_CLASSES`, header module per class). The rest of the ingest output is byte-identical.
- Generator additions: `construct.args`, `construct.context_menu`, prop `dynamic` + `after_set`, slot `order: actions`. The `items` slot is `QWidget::insertAction/removeAction` with a `QMenu` child contributing `menuAction()`; glue in `menus.hpp.in` (`menu_insert`, `menu_remove`, groups, context menus).
- Elements: `action`, `menuseparator` (QAction + `setSeparator(true)`), `menu`, `contextmenu` (both QMenu), `menubar`, `toolbar`. The `group` prop is a dynamic property `svlGroup`; the items slot puts equal names into one exclusive `QActionGroup` child of the holder.
- Ownership: an item is parented to its holder, a removed item is released, deleting the holder deletes the items (tested with QPointer, like the other slots).
- Generated tests: per-slot `order`, `default_slot_is_slot_0`, `separator_and_submenu`, `rejects_<tag>`, `remove_released`, `parent_destroy_deletes_children`, `radio_group`; the `contextmenu` construct fact (attach, request opens it, detach, delete with the widget).
- Surprises:
  - `QObject::setProperty` returns false when it creates a dynamic property (not an error).
  - The binding test fires the signal's trigger with `$i = 1`, so `toggled`'s trigger flips (`setChecked(!isChecked())`).
  - `CHECK(a == QList<QAction*>{...})` needs parentheses (commas in the macro).
  - Shortcuts need an active window; under xvfb without a window manager the test hook calls `QApplication::setActiveWindow` and runs the shortcut map with `qt_sendShortcutOverrideEvent` (exported by QtGui, declared in QtTest's header).
  - Context-menu actions' shortcuts are only live while the menu is open (a hidden QMenu is not a shortcut context); GTK keeps them live on the target widget. Not unified.
- Tests: generated (`examples/counter-qt/tests/generated.rs`), `host_menus.rs` (real host and fixture: trigger, check, radio, keyed reorder, toolbar, shortcut through the shortcut map, real context-menu event), `oplog_replay.rs` (`menus` replays; events go through the new `action.trigger()` method).

## Open

- Qt `toolbutton` has no `menu` slot (QToolButton::setMenu does not reparent, and the generator's one-slot ownership tests assume it does); in Qt a menu in a `toolbar` becomes a tool button with a popup instead.
- `contextmenu.popup`: GTK takes `(x, y)`, Qt a `Point` record.
- `menu.visible` exists on GTK only (a QMenu's `visible` is the popup window).
- No window-level slots: place `menubar` and `toolbar` in a vertical `box` (QMainWindow `menuBar`/`addToolBar` areas stay G1).
