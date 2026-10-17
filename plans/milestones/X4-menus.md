# X4: menus and actions (non-widget child objects)

Lane X4, started 2026-10-17 on master 2aa2127. Closes `metadata/A1-GAPS.md` G7. Notes: `docs/notes/x4-menus.md`. ABI: no change (see Decisions).

## Goals

A declarative, reactive Svelte model for menus that becomes native menus on both toolkits: menubar, menus and submenus, actions with shortcuts, icons, enabled state, check and radio state with `bind:checked`, context menus on any widget, toolbar actions.

```svelte
<box orientation="vertical">
  <menubar>
    <menu label="File">
      <action label="Open" icon="document-open" shortcut="Ctrl+O" onactivate={open} />
      <action label="Autosave" checkable bind:checked={autosave} />
      <separator />
      <menu label="Recent">{#each recent as r (r)}<action label={r} onactivate={() => open(r)} />{/each}</menu>
      <action label="Quit" shortcut="Ctrl+Q" onactivate={quit} />
    </menu>
    <menu label="Size">
      <action label="Small" group="size" bind:checked={small} />
      <action label="Large" group="size" bind:checked={large} />
    </menu>
  </menubar>
  <toolbar><action icon="edit-copy" label="Copy" onactivate={copy} /><separator /></toolbar>
  <button>Right click me <contextmenu><action label="Paste" onactivate={paste} /></contextmenu></button>
</box>
```

## Decisions

- **Elements (same on both toolkits).**
  - `menubar`, `toolbar`: widgets, placed in a box like any widget.
  - `menu`: a submenu (child of a menu or menubar), or the popup of a `menubutton` (Qt `toolbutton`) as its `menu` child.
  - `contextmenu`: a menu that attaches to its parent widget and pops up on the secondary click (and the `popup(x, y)` method).
  - `action`: props `label`, `icon` (theme icon name), `shortcut` (`Ctrl+O`), `tooltip`, `enabled`, `visible`, `checkable`, `checked`, `group`; signals `activate(checked)` and `toggled(checked)`; `bind:checked`.
  - `separator`: a separator between items (GTK: a section boundary).
- **Radio:** `group="name"`: the actions of one parent with the same group form an exclusive set, each keeps its own `checked` (a user pick turns the old one off natively, JS sees both `toggled` events). Qt: `QActionGroup` kept per parent and name. GTK: one `GSimpleAction` with a string state per (parent, group); the item target is the action id. Flat, so reordering and keyed `{#each}` need no grouping node.
- **ABI: no change.** Menu nodes are kinds; `insert/remove` with a `before` anchor are the existing ops (reorder = insert with `before`); props/signals are the existing ones. Where a parent has no explicit slot the default slot (0) is used and the backend decides by the child kind.
- **Qt.** `QAction` and `QActionGroup` are ingested (QtGui); elements `action`, `separator` (QAction + `setSeparator`), `menu`, `menubar`, `toolbar`, `contextmenu` (QMenu). Slot variant `order: actions` in the generator (host `QWidget::insertAction/removeAction/actions()`, a `QMenu` child contributes `menuAction()`) with generated order/ownership tests; hand-written glue (groups, context popup, ownership) lives in a template. Actions are QObject-parented to the menu/bar/toolbar holding them. `popup`, never `exec`.
- **GTK.** Hand-written module `backend/menus.rs` like `virtuallist`/`gio-file` (the generated tables have no GMenuModel kinds). A Rust node tree per root; every structural or label change re-syncs the root: its `GSimpleActionGroup` (prefix `svl`, inserted on the root widget), the `GMenu` models (sections split at separators; submenus reference the child's model), a global-scope `GtkShortcutController` with one `GtkShortcut` per shortcut and the `accel` attribute for display. `menubar` = `GtkPopoverMenuBar` over the root `GMenu`; `contextmenu` = `GtkPopoverMenu` parented to the target widget with a secondary-button gesture; `menubutton` takes a `menu` child via `set_menu_model`; `toolbar` = a `GtkBox` of buttons rebuilt from the nodes.
- **Facts and ledger.** Qt elements are generated and verified fact by fact (`metadata/verified/qt.json`). The GTK module is hand-written; tests in `crates/svelters-gtk/tests/menus.rs` cover each member.
- **Bindings.** `bind:checked` on `action`: Qt through the YAML `bindings`; GTK through a small hand-written source read by `gen_bindings.py`.

## Steps

1. Plan, notes (this commit).
2. GTK: `menus.rs`, wiring in `backend.rs`, bindings, tests, fixture `fixtures/menus`, host test.
3. Qt: ingest QAction/QActionGroup, generator (`construct.args`, `order: actions`, `init`), template glue, overrides, generated tests, ledger, host test.
4. Gallery pages (GTK and Qt), `.d.ts`, screenshots.
5. A1-GAPS G7 closed, full gates, report.
