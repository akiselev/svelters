# N1-F: fixtures, compiled path, conformance, bench after the N1 rename

Sub-lane of N1 (`docs/notes/n1-rename.md`). Scope: `fixtures/**` (not the galleries), `packages/testhost`,
`experiments/native-compiler/**`, `tests/renderer-conformance/**`, `bench/**`.

## TestHost toolkit selection

- `createTestHost({ toolkit })` / `runBundle({ toolkit })`: `gtk` (default, `src/schema.json`) or `qt`
  (`src/schema.qt.json`). `toolkit: 'qt'` also makes the host answer the QSS style dialect (`styleDialect`
  still overrides). `schemas` is exported.
- `bin/svelters-test.mjs` reads the toolkit from the fixture's `package.json`: `"svelters": { "toolkit": "qt" }`.
  `script.json` stays a plain step array (the Rust `parse_script` reads the same file).

## Log

### GTK fixtures (done)

- Renamed in every fixture App.svelte (galleries excluded): `onclick` -> `onclicked`, `onclose` ->
  `onclose-request`, headerbar/scrolledwindow/textview/aboutdialog/menubar/toolbar/menuseparator/contextmenu
  -> kebab tags, `placeholder` -> `placeholder-text`, `programName` -> `program-name`,
  `parent:columnSpan` -> `parent:column-span`. Scripts unchanged (signals are native names already).
- Regenerated oplogs: only name changes in the diff (create <tag>, set <prop>, attr column-span).
- `stepIncrement` -> `step-increment` on `<scale>` (buffer-bind) once the metadata was regenerated.
- counter-mac, counter-win, gallery-appkit: no DOM aliases (`onaction`, WinUI `onclick` = native Click).

### Qt twins

`fixtures/<name>-qt` (package `@svelters-fixtures/<name>-qt`, `"svelters": { "toolkit": "qt" }`), same
layout as the original, Qt vocabulary: main-window/window-title, box-layout/direction, push-button,
line-edit (+ `ontext-changed`, script `text-changed`), scroll-area, grid-layout, menu title / action
text / `ontriggered` (script `triggered`), `<action separator={true}/>`, context menu = `<menu>` in a
push-button; styles: `push-button.primary`, `line-edit.field`, only the `@native qt` block.
Requested: counter, child-attrs, styles, menus, lists-stress. Also (so the compiled qt target keeps
every case it compiled before): keyed-list, keyed-stress, native-close, timers, virtual-list,
lists-recycle, bind-this, borrowed-walk; bind-value (line-edit at #3, for examples/counter integration
bind_value_qt). Oplogs reviewed: vs the originals only tag/prop/signal names differ.

Qt `<label>` text content: the renderer picks the first TEXT_PROPS candidate the host resolves
(`label: ["label", "text"]`). The schemas now list every settable prop per tag (n1-rename regenerated
them) and the TestHost `propId` returns null for a prop missing from a kind the schema knows (unknown,
synthetic kinds such as virtual-list resolve anything), so Qt twins log `set #N text`, like real Qt.

### virtual-list props (owned here)

`rowHeight` -> `row-height`, `minHeight` -> `min-height` (synthetic kind, kebab rule): ListView.svelte
(`'row-height'` prop, forwarded as `row-height`), native-runtime `lists.js` (+ its test),
GTK `backend/vlist.rs`, Qt `templates/vlist.hpp.in`, fixtures. Methods (`scrollTo`, `setRow`, `rowAt`,
`splice`, `count`) unchanged (methods are not renamed). Plans docs (backend-abi.md, M5-lists.md) still
say rowHeight/minHeight.

### bench

- `bench/keyed-bench`: GTK renames (`onclicked`, `scrolled-window`); new Qt twin `bench/keyed-bench-qt`
  (`@svelters-bench/keyed-bench-qt`). `compiled/bench-lists.mjs` runs the twins on qt.

### Compiled path (experiments/native-compiler)

- `compiled/cases.mjs`: shared fixtures compile for `gtk` (and `testhost` for the X2/X3 ones); each
  Qt twin `fixtures/<name>-qt` is a qt-only case `<name>-qt` compared with the twin's JS pipeline
  (state at every boundary, plus `script.deferred.json`). No qt hand-written equivalents.
  if-toggle (header-bar), sync-handlers, listen-phases have no twin: their qt rejections are gone.
- Own fixtures (`experiments/native-compiler/fixtures/<name>`): `App.svelte` is GTK vocabulary; the qt
  target compiles `<name>/qt/App.svelte` (Qt-vocabulary copy, components alongside, `.svelte.js`
  modules imported from `..`; `qt/script.json` when signals differ, e.g. form-binding `text-changed`).
  `cases.mjs` exports `sourceOf` / `scriptOf`. slots-gtk / snippet-slots-gtk stay GTK (rejected on qt),
  slots-qt / snippet-slots-qt are rewritten in the Qt vocabulary (rejected on gtk).
- `run.mjs` runs each bundle on the TestHost in its target's vocabulary (`toolkit` = target backend:
  qt schema + QSS for qt), so `expected.qt.oplog` style lines are QSS now. `galleries.mjs` likewise.
- `native.mjs` only runs a case on the backends it targets, and the JS-pipeline twin from the case's
  fixture dir. The counter driver must map scripted `text-changed` to typing for the Qt twins (it
  mapped `changed`); not checked here (needs a display).
- `handwritten`: `header-bar`, `scrolled-window`, `placeholder-text`; oplogs regenerated (names only).
