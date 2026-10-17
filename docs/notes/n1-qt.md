# N1-Q: Qt tests and gallery on the native Qt names

Sub-lane of N1 (see `n1-rename.md`, `n1-rename-map.md`). Scope: `examples/counter-qt/**`, `fixtures/gallery-qt/**`,
hand-written name strings in `tools/metadata/qt/templates/*.in`.

## gallery-qt

- Every page, `App.svelte`, `StyledDemo.svelte` translated to the Qt vocabulary (tags, props, events, child
  attrs, `bind:` names, getProp/setProp names in scripts). Checked against `packages/renderer/types/qt.d.ts`
  with a throwaway checker (every tag/prop/event/bind/parent: name of every page exists in the typings).
- Box orientation -> `direction="left-to-right|top-to-bottom"` on box-layout/frame; Qt::Orientation props
  (slider, scroll-bar, splitter, tool-bar, dialog-button-box) keep `orientation="horizontal"`.
- `11-menus`: `<menuseparator/>` -> `<action separator={true}/>`, `<contextmenu>` -> `<menu>` inside the
  push-button; the `kinds:` header now lists classes (`QMenuBar QMenu QAction QToolBar`) like every other
  page (it listed old tag-ish names, which coverage.py could not match).
- `09-dialogs`: the dialog kind list (button labels, `done(kind)` strings) uses the new tags.
- Style: `button.primary` -> `push-button.primary`, `entry.field` -> `line-edit.field`,
  `.setters { orientation: horizontal }` -> `direction: left-to-right`; `style-setters.expected` now expects
  `direction`, `contents-margins`, `minimum-width`, `maximum-width`.
- Echo labels renamed to the native names (`echo current-index: N`, ...); gallery.rs only requires
  `echo <x>: 0`, so names are free.
- Build: `pnpm --filter ./fixtures/gallery-qt build` OK (gtk-side style warnings for Qt-only tags/props are
  expected: the bundle compiles rules for both toolkits); `check-pages.mjs` PASS (16 pages).

## counter-qt tests

- Tests that ran GTK-vocabulary fixtures now run the N1-F twins: counter.rs (comment; its ops are
  hand-written), host_childattr.rs -> child-attrs-qt, host_style.rs -> styles-qt, host_menus.rs -> menus-qt,
  lists_e2e.rs -> lists-stress-qt. oplog_replay.rs requires `child-attrs-qt` and `counter-qt` to replay;
  its native-event whitelist is `clicked | triggered | toggled` (action `activate` is `triggered` now).
- Hand-written ops in counter.rs, borrowed.rs, serdes.rs, style.rs, toplevels.rs and tests/js/*.js use the
  Qt tags/props/events (main-window/window-title, box-layout/direction, push-button/text, line-edit,
  scroll-area, text-edit/text-document props plain-text/block-count/undo-redo-enabled, color-dialog,
  message-box, tool-bar, dock-widget, `close` instead of `close-request`). `native_prop` / `.native()` in
  serdes.rs read QObject properties by their Qt names, so those stay camelCase; methods are not renamed.
- serdes.rs enums: box-layout `direction` members are `left-to-right, right-to-left, top-to-bottom,
  bottom-to-top` (QBoxLayout::Direction order); there is no member-renaming override any more.

## Templates (hand-written strings)

- verify.cpp.in (run by tests/generated.rs) still used `window`, `entry`, `box` and
  `G("window", "close-request")`: now `main-window`, `line-edit`, `box-layout`, `G("main-window", "close")`.
- canvas.hpp.in: props `contentWidth`/`contentHeight` -> `content-width`/`content-height` (the GTK canvas is a
  GtkDrawingArea, whose properties are `content-width`/`content-height`; same names on both toolkits), and its
  event `double-click` -> `mouse-double-click` like every Qt widget. backend.cpp.in: comments only.
- vlist.hpp.in: virtual-list props `rowHeight`/`minHeight` are left alone: they are the portable list
  protocol shared with packages/lists, native-runtime lists.js and GTK vlist.rs (not N1-Q's files).

## tools/docs/screenshots/qt.py

- Recipes renamed to the new tags (`r_main_window`, `r_box_layout`, `r_push_button`, `r_line_edit`,
  `r_scroll_area`, `r_status_bar`, `r_menu_bar`; `_` maps to `-`), and the override tag regex accepts `-`.
- Unresolved (not mine): test_screenshots.py `test_button_both_toolkits` expects `qt/button`; Qt has
  `push-button` now, so that test needs a per-toolkit tag (or `--only` with both names).
