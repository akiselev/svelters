# WG4b: native dialogs, date and colour pickers, drag and drop (WIDGETS, Tier 1)

Design: `plans/research/EGUI.md` §5 (Tier 1 of the element catalog, the version rules) and §8 (the
2026-10-22 decision: the catalog is the built-in widget set, fast-tracked); `plans/research/TRACKS.md`
WIDGETS WG4. Builds on WG1's shared machinery (`origin/ci/wg1`, merged in) and E1's async exports
(`plans/milestones/E1-exports-offload.md`). Sibling lanes: WG2a (menus, popups, modal), WG2b (Tier 0
controls), WG4a (table, tab-view, split-view, toast).

## What landed

| Piece | Where |
|---|---|
| `date-picker` (`egui_extras::DatePickerButton`) and `color-picker` (`egui::color_picker::color_edit_button_hsva`): tables, payload, props, paint, measure | `crates/svelters-egui/src/kinds/pickers.rs` |
| Drag and drop on any node: `drag-type`, `drag-data`, `drop-types` on `Widget`; `drag-begin`, `drag-end`, `drag-enter`, `drag-leave`, `drop`; the public `DragPayload` | `crates/svelters-egui/src/kinds/dnd.rs` |
| `svelters:app/egui`: `openFile`, `openFiles`, `saveFile`, `pickFolder`, `messageBox` (Promises), `DialogBackend` + `set_dialog_backend`, the rfd backend | `crates/svelters-egui/src/dialogs.rs` |
| Gallery pages (pickers, drag and drop, dialogs) and the dumped `svelters:app/egui` stub | `fixtures/gallery-egui` |
| Tests | `crates/svelters-egui/tests/wg4b.rs`, `tests/js/dialogs.js`; `kinds/dnd.rs` unit test |

Shared files got dispatch arms only: `kinds/builtin.rs` (two `KINDS` entries, three `Widget` props, the
`Widget` signals), `kinds/mod.rs` (modules, six `P` keys), `scene/node.rs` (`W::Picker`, `Common::dnd`,
`text_of`), `scene/props.rs` (`is_common`, one set and one get arm each for DnD and pickers),
`scene/tree.rs` (payload), `leaf.rs` (three arms: value before, paint, change), `layout.rs` (measure),
`walk.rs` (two `dnd::node` calls), `backend.rs` (`dialogs::link()`), `lib.rs` (`dialogs`, re-exports of
`egui_extras` and `DragPayload`).

Dependencies: `egui_extras =0.37.0` (feature `datepicker`, so `jiff`), `jiff 0.2` (date parsing), `rfd 0.17.2`
(optional, feature `dialogs`, on by default), and the `svelters` facade (the export macros). One egui copy:
egui_extras is pinned to egui's minor; rfd has no egui dependency.

## The vocabulary

### Pickers

```svelte
<date-picker bind:value={due} format="%d %b %Y" calendar-week={false} />
<color-picker bind:value={layer} alpha="blend" />
```

- Values are strings so they are the same on every backend: a date is ISO 8601 (`2026-10-22`, `""` for
  none: the button shows today); a colour is CSS hex, `#rrggbb` when opaque and `#rrggbbaa` otherwise
  (`#rgb`, `#rgba` accepted and stored normalized). A malformed value is a `set_prop` error.
- `value` is `Native::Widget` (exact observation), so `bind:value` lowers onto observation; `changed(value)`
  fires on user edits. The date changes when the user presses Save in the calendar popup; the colour on
  every edit in the popup (once per frame while dragging).
- `date-picker`: `format` (display only, `strftime`), `calendar-week`. `color-picker`: `alpha` (`opaque`,
  `blend`, `blend-or-additive`); with `opaque`, alpha is forced to 255.
- The colour picker keeps the popup's HSVA while the user edits, so a grey keeps its hue; a host write
  drops it.

### Drag and drop (proposed vocabulary)

No other backend had a DnD vocabulary in the egui sense. GTK has attached controllers (`DragSource`:
`drag-begin`/`drag-end`; `DropTarget`: `enter`/`leave`/`drop`) and WinUI `UIElement` signals (`drag-starting`,
`drag-enter`, `drag-leave`, `drop`). WG4b proposes props on the `Widget` base (any element, leaves and
containers) with GTK's source names and WinUI's target names:

| Member | On | Meaning |
|---|---|---|
| `drag-type: string` | source | non-empty makes the node a drag source; a MIME-like type (`cad/part`) |
| `drag-data: string` | source | the payload (JSON for structured data) |
| `drop-types: string` | target | accepted types, space or comma separated; `*` any, `cad/*` a prefix |
| `drag-begin()` | source | the drag started |
| `drag-end(dropped: boolean)` | source | the drag ended; `dropped` when a target took it |
| `drag-enter(type: string)` | target | an accepted drag came over it |
| `drag-leave()` | target | it left, was cancelled, or was dropped here (always pairs with `drag-enter`; before `drop`) |
| `drop(type, data, x, y)` | target | released over it; `x`, `y` in the node's own points |

```svelte
<button drag-type="cad/part" drag-data={JSON.stringify(part)}>{part.name}</button>
<box drop-types="cad/*" ondrop={(type, data) => place(JSON.parse(data))} />
```

Built on egui's `DragAndDrop` with a public payload type, `svelters_egui::DragPayload { ty, data, source }`, so
Rust panels of the host app (cadatomic's) start drags Svelte accepts (`DragAndDrop::set_payload(ctx,
DragPayload::new("cad/part", id))`) and accept Svelte drags (`response.dnd_release_payload::<DragPayload>()`,
then `payload.accept()` so the source's `drag-end` says `dropped`).

Feedback: `Grab` cursor over a source, `Grabbing` while over an accepting target, `NoDrop` elsewhere; a
label with the source's text (or type) follows the pointer in the tooltip layer; the active target gets a
selection-coloured outline.

### Dialogs

```js
import { openFile, openFiles, saveFile, pickFolder, messageBox } from 'svelters:app/egui';
const path = await openFile({ title: 'Open part', filters: [{ name: 'STEP', extensions: ['step', 'stp'] }] });
const answer = await messageBox({ description: 'Save?', level: 'warning', buttons: 'yes-no-cancel' });
```

- File dialogs resolve with the path(s), `null` when cancelled. Options: `title`, `directory`, `fileName`,
  `filters`. `messageBox` resolves with `ok`/`cancel`/`yes`/`no`, or the caption clicked when `labels`
  (one to three captions) replaces `buttons`.
- Each is a bare `async fn` (E1: JS thread, polled when rfd's future wakes it), so the frame never blocks.
  The trailing `{ signal }` aborts: the Promise rejects with `svelters`/`cancelled` at once; the native
  dialog stays open until the user closes it and its answer is dropped.
- Without the `dialogs` feature and without an installed backend they reject with
  `NativeError { domain: 'app:egui', code: 'unsupported' }`.

## Decisions and deviations

- **Dialogs are functions, not elements.** EGUI.md §5 says "returning Promises"; the Qt gallery's G9 note
  shows why elements fit badly (no element methods from Svelte, toplevel mounting rules). Rust exports give
  Promises, cancellation, typings and the reload/dispose rules of E1 for free.
- **Module `svelters:app/egui`.** The specifier scheme is `svelters:app/<module>` (D6). `egui` avoids taking a
  name an app is likely to use (`dialogs`). Every binary linking svelters-egui exports it (`EguiBackend::new`
  calls `dialogs::link()` so the linker keeps the registrations), so `svelters_egui::dump` now also writes
  `egui.js` and its `app.d.ts`/`exports.json` entries: the `egui-kinds` example's checked-in dump was
  regenerated.
- **Tests use a fake backend** (`set_dialog_backend`, thread-local: the JS thread). No display for native
  dialogs in CI. rfd itself is not exercised in CI beyond compiling.
- **No parent window.** rfd dialogs open unparented (no `set_parent`); on macOS and Windows that means app-
  modal rather than sheet/owner-modal. Follow-up: thread the eframe window handle (runner) or an embedder-
  supplied parent through `DialogBackend`.
- **DnD payload data is a string.** Built-in props have no "any" type (the provider `Ty::Provider` path needs a
  provider kind). JSON in a string keeps the ABI and the typings simple; `type` does the filtering.
- **The drop target is one pass late.** Targets claim the pointer during the walk (pre-order: the deepest,
  last-painted accepting target wins); the claim from the previous pass decides. A drag repaints every
  frame, so this is invisible, and it makes nested targets (a box and the labels in it) correct without a
  second walk. A target never accepts its own drag.
- **The source's drag area is a drag-only interact under the widget**: registered before a leaf's widget and
  before a container's children. A widget on top that senses only clicks (a button) keeps its clicks and hands
  drags to the area beneath (tested); one that senses drags itself (slider, text field) keeps them, so it
  cannot be a drag source except by its unsensed margins. A first version registered the area on top: it ate
  the button's clicks.
- **The date picker has no calendar icon and no year/month/day combo boxes.** egui 0.37's default fonts
  have no emoji (`📆` draws as tofu, and kittest panics on missing glyphs), and egui_extras' combo boxes have
  no accessible names (kittest's accessibility check fails). The popup keeps the calendar, the
  `<<< << < > >> >>>` arrows and Save/Cancel.
- **egui's colour popup draws a `📋` copy button** that the default fonts cannot draw (tofu in apps). The
  tests set `MissingGlyphPolicy::Tofu` for it. Follow-up for WG1's `fonts`: register an emoji-capable font.
- **The dialog functions' parameter is `opts`.** Named `options`, the generated stub was
  `function openFile(options, options)` (E1's trailing `options` for `{ signal }`), a duplicate parameter. The
  export codegen should rename or reject a declared parameter called `options` (host follow-up).
- **`drag-end` fires on the pass after the release**, when every target of that pass has run (so `dropped` is
  final).
- **`Common::dnd`** is one boxed `Option` (no cost on nodes without DnD); the walk checks it per node.
- **`fixtures/gallery-egui` is new.** No egui gallery existed; it is one `App.svelte` with a page switcher and
  one file per page (`src/pages/wg4b-*.svelte`), so other WIDGETS lanes add pages without touching each
  other's. It imports `svelters:app/egui`, so it carries the dumped stub in `.svelters/app`
  (`gallery_export_stubs_are_current`, `SVELTERS_UPDATE=1` rewrites). `tests/wg4b.rs`
  `gallery_pages_drive_pickers_dnd_and_dialogs` drives all three pages from the built bundle.

## Notes for other lanes

- **EVENTS:** DnD signals are delivered through the outbox like other widget signals (not sync); a
  `use:` behaviour (EV2b) can read the same `DragPayload` in `after_ui`.
- **WG4a (tree-view later, table):** row reordering can use `drop`'s `x`/`y` and `Hit::Part` keys; virtual rows
  as drop targets need a hook in the virtualized walk (not done).
- **STYLE:** the target outline uses `Visuals::selection.stroke`; a `:drop-target` pseudo-class could map onto
  the `dnd-in` state when CS5 lands.

## Not run locally

- Run locally: `cargo check -p svelters-egui --all-targets`; nextest `-p svelters-egui --test wg4b` (7/7:
  pickers, two DnD tests, dialogs, gallery stub, gallery pages) and `--test spec` (3/3), the `kinds::dnd`
  unit test; `SVELTERS_UPDATE=1` for `egui.json`/`egui.d.ts`, the gallery stub and the `egui-kinds` dump;
  `pnpm build` of `fixtures/gallery-egui`.
- Not run: the rest of the svelters-egui suite, full workspace nextest, conformance, the other backends'
  suites, `pnpm test` of `examples/egui-kinds/app` (svelte-check over the regenerated `app.d.ts`); CI runs them.
- rfd's real dialogs are compiled, never shown (no display in CI).
