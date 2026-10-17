# N1: native kebab-case names for tags, props, events and child attrs

Lane N1, branched from master at ff0fcb4. Full old -> new map: `docs/notes/n1-rename-map.md`.

## Rule

- Element tag = native class name without the framework prefix (`Gtk`, `Q`, `NS`, WinUI namespace),
  kebab-cased: `QMainWindow` -> `main-window`, `GtkDrawingArea` -> `drawing-area`.
- Props, events, child attrs and `bind:` names = the native name, kebab-cased (`minimumWidth` ->
  `minimum-width`, `textChanged` -> `text-changed`). GObject names are already kebab.
- Handlers are `on<kebab-event>` (`ontext-changed`); the X3 `…capture` suffix is unchanged.
- No aliases: the old names and the DOM-ish event aliases (`click` -> `clicked`) are gone.

## Survey (start)

- `tools/metadata/appkit/names.py` already had the rule (mirrored by `crates/svelters-apple/src/names.rs`);
  WinUI had an equivalent regex. Those become the shared `tools/metadata/common/names.py`.
- GTK: tags were `name.lower()` (`textview`), props `camel(name)`, child attrs camel; overrides renamed a
  few (`window` = ApplicationWindow, `clickgesture`, `keycontroller`, ...).
- Qt: every tag and JS name was hand-written in `metadata/overrides/qt/*.yaml` (`js:` / `tag:`), in the
  GTK vocabulary (`entry`, `button`, `label` prop for `text`, `scrolledwindow`, ...).
- Shared fixtures are written in the GTK vocabulary and several Qt tests run them; after the rename the
  vocabularies differ, so Qt runs need Qt-vocabulary twins.
