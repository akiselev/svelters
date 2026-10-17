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

## Decisions

- **Acronyms** (`naming.kebab`): a run of capitals is one word, except its last capital starts the next
  word when a lowercase letter follows: `QLCDNumber` -> `lcd-number`, `GtkGLArea` -> `gl-area`,
  `QHBoxLayout` -> `h-box-layout`, `QOpenGLWidget` -> `open-gl-widget`. Digits stay with the previous word.
  Same function as `crates/svelters-apple/src/names.rs` (tested against its `PAIRS`).
- **Qt box/grid**: one `box-layout` (QWidget + QBoxLayout) with the native `direction` prop
  (`left-to-right`, `right-to-left`, `top-to-bottom`, `bottom-to-top`), not h-/v-box-layout: the element
  already constructs a QBoxLayout and the old `orientation` prop was `QBoxLayout::setDirection`, so this is
  the native API one-to-one and one element in the generator. `grid` -> `grid-layout`. Tag rule in
  `gen_cpp.element_tag`: a plain QWidget carrying a layout takes the layout's class tag. `frame` and
  `group-box` (also QBoxLayout-carrying) get `direction` too. Qt style property `orientation` -> `direction`.
- **Qt contextmenu** folded into `menu` (`construct.context_menu: true`); `menus.hpp.in ctx_role`: a menu
  inserted into a QMenu/QMenuBar/QToolBar is a submenu, into any other widget a context menu.
- **Qt menuseparator** folded into `action separator={true}` (accessor prop: QAction::isSeparator /
  setSeparator; gen_cpp now supports getter/setter props without a Q_PROPERTY; their JS name is
  kebab(setter minus `set`)).
- **Synthetic names** (no native class or member): GTK `menu-bar`, `tool-bar`, `context-menu`,
  `menu-separator`, `menu`, `action` (menus.rs), `canvas` (GTK: a DrawingArea; Qt: canvas.hpp.in),
  `virtual-list` (was `virtuallist`); Qt action `group` (dynamic property); Qt event-filter events
  key-press, key-release, mouse-press, mouse-release, mouse-move, wheel, focus-in, focus-out, enter, leave,
  resize, context-menu, and renamed `double-click` -> `mouse-double-click`, `close-request` -> `close`
  (QWidget::mouseDoubleClickEvent / closeEvent).
- **GTK window**: old `<window>` was GtkApplicationWindow (an override claimed the tag). Tags are now
  derived, so `window` is GtkWindow and GtkApplicationWindow is `application-window`. The backend sets
  `application` on any object that has the property, so `<window>` behaves the same; fixtures keep it.
- **Event aliases removed** everywhere (renderer, native-runtime, DOM shim, semantics rules, docs IR,
  typings): `onclick` -> `onclicked`, `onchange` -> `onchanged`, `onclose` -> `onclose-request` (GTK).
- **Methods** are not renamed (not in scope; GTK keeps GIR snake names, Qt camelCase).
- **Rule enforcement**: GTK ingest rejects an override tag that differs from the derived one without
  `tag_reason` and child attrs not named after their property; Qt gen_cpp rejects tags/props/signals/attrs
  that break the rule (`check_names`). The Qt overrides still spell `tag:` and `js:` (validated restatements).
- **Generated from metadata now**: renderer `TEXT_PROPS` (from the specs' `text_content` + WinUI
  overrides) and `ROW_OUTLETS` (from the specs' new `row_outlet`), both in `bindings.generated.js`; the
  compiler's row outlet comes from `row_outlet` (was `ROW_OUTLET_KIND = "box"`).
- **Qt TestHost schema**: `gen_cpp.py --schema packages/testhost/src/schema.qt.json` (coarse prop types for
  Qt-vocabulary fixtures on the Node TestHost).
- **Qt docs IR**: the IR already held all 54 annotated Qt element classes (55 tags) when built from this
  tree; the docs test now asserts that every override element tag is in the IR, one entity per class.
