# N1-G: GTK tests, counter example and gallery on the native names

Sub-lane of N1 (see `n1-rename.md`). Old -> new names from `n1-rename-map.json`.

## Changed

- `crates/svelters-gtk/tests/*.rs`, `tests/js/*.js`: every quoted tag/prop name (`h.create("textview")`
  -> `"text-view"`, `menubar` -> `menu-bar`, `menuseparator` -> `menu-separator`, `contextmenu` ->
  `context-menu`, `toolbar` -> `tool-bar`, `aboutdialog` -> `about-dialog`, `contentWidth` ->
  `content-width`, `updatePolicy` -> `update-policy`, `defaultWidth` -> `default-width`, ...),
  `menu_find("menu-bar", ..)`, oplog_replay's `create virtual-list` skip. Rewritten mechanically (only
  whole quoted strings), then the leftovers in doc comments by hand. `window` stays `window`.
  Methods (`getFirstChild`, `fillRect`, ...) unchanged, as decided.
- `crates/svelters-host/tests/reload.rs`: the inline Svelte app's `onclick` -> `onclicked`.
- `fixtures/gallery-gtk/src/**`: tags (`<scrolled-window>`, `<menu-button>`, `<event-controller-key>`,
  `<gesture-click>`, `<menu-bar>`, `<tool-bar>`, `<context-menu>`, ...), props (`use-markup`,
  `placeholder-text`, `icon-name`, `default-width`, ...), child attrs (`parent:tab-label`,
  `parent:column-span`), events (`onclicked`, `onclose-request`).
- `fixtures/gallery-gtk/style-setters.expected`: the TestHost now logs `margin-top` / `width-request`.
- `examples/counter/tests/gallery.rs`: `<window>` is GtkWindow now, so the 08-dialogs toplevel test looks
  for `GtkWindow` (was `GtkApplicationWindow`) for the main and the tool window.
- `examples/counter/tests/integration.rs`: `if_toggle_qt_reports_missing_headerbar` expects the error to
  name `header-bar`.
- `tools/docs/screenshots/gtk.py`: recipes keyed by kebab tag (`r_check_button` -> `check-button`; the
  RECIPES map turns `_` into `-`); `r_window` builds a GtkWindow, new `r_application_window`; UNSUPPORTED
  keys renamed (`popover-menu`, `emoji-chooser`, `media-controls`, `drag-icon`, `gl-area`).
- `fixtures/gallery-common/*`: no change needed (already toolkit-neutral: it only clicks handles and
  reads `style-setters.expected`).

## Qt twins (`fixtures/<name>-qt`, made by n1-fixtures)

- `examples/counter/tests/integration.rs`: `fixture_for(backend, name)` sends Qt runs to the `-qt` twin
  (script.json too); hand-written scripts type with `text-changed` on Qt (`changed(backend)`), and their
  files are named per backend (they differ now). `if_toggle_qt_reports_missing_headerbar` runs the GTK
  `if-toggle` bundle on Qt on purpose (`run_dir`), no twin.
- `examples/counter/src/driver.rs`: a script `text-changed` event types into the widget like `changed`
  (QLineEdit::textChanged is the twins' edit signal).
- `crates/svelters-gtk/tests/oplog_replay.rs`: skips `*-qt` (Qt tags).
- `crates/svelters-host/src/testing.rs`: `run_fixture` reads `"svelters": {"toolkit"}` from the
  fixture's package.json; Qt twins run with `schema.qt.json` and QSS style answers.
- Surprise: the log backend's schema was a process-wide `OnceLock` and the fixture test runs every
  fixture in one process, so the schema could not be chosen per fixture. Added (additive, default
  unchanged) `LogBackend::with_schema` and `with_style_target` in `crates/svelters-backend-log`.

## Status

- `check-pages.mjs` on gallery-gtk: PASS (16 pages).
