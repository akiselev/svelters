# PRESTO-widgets-tier1: presto-widgets tables, tabs, pickers, DnD, dialogs, zoom

Lane WIDGETS-TIER1 of presto batch 2 (`plans/cloud/presto-b2/LANE-widgets-tier1.md`). Session
`session_01DjBCnf7oMK3XEzkghWoiQs`, branch `ci/presto-widgets-tier1` from `ci/presto-base` e34079e6.

## State

presto-core and presto-text had no API skeleton on `ci/presto-base` when this lane started, so every
tier-1 widget is written as **toolkit-independent native state, geometry, input state machines and
recording** over presto-base types, plus a small local shim (`crates/presto-widgets/src/table/shim.rs`).
presto-core's skeleton and WIDGETS-BASIC's widget substrate landed mid-lane (merged at `d6725daa` and
`d7785fc5`). `pickers/kinds.rs` declares the four picker kinds' specs with `kind_props!` and implements
the leaf kinds `calendar` and `color-editor` against the substrate's kind interface (`shim::core::Kind`),
registered on presto-core through `host::register` (`Hosted<W>`). `table/text_painter.rs` implements the
tier-1 `TextPainter` over the substrate's `TextShaper` (presto-text), honouring the requested family,
size, weight and colour, with the shaper's own one-line ellipsis. The picker look comes from the built-in
theme's tokens (`PickerLook::from_theme`).
The container and popup kinds (`table`, `tab-view`, `date-picker`, `color-picker`) have no `Kind` impl
yet: see "presto-core gaps". Nothing here returns a fake success.

## What landed

- `tabs/`: a presto-drawn `tab-view` (OW5c; svelters-egui's used egui widgets). Caption layout with
  min/max widths and elision, select on primary press (`changed`), close button and middle click
  (`close-requested`), keyboard (arrows/Home/End when focused; Ctrl+Tab, Ctrl+PageUp/PageDown cycle),
  horizontal wheel scrolling of an overflowing strip with reveal-on-select, recording (fills, title,
  × glyph, selected indicator, focus ring, clip), AccessKit `Tab` nodes. `caption_of` for `tab` titles
  and fallbacks.
- `dnd/`: in-app drag and drop. `DragPayload` entries carry text or a **JS-side id** (`PayloadData::Js`);
  `drop-types` matching (`*`, `cad/*`), drop actions from modifiers per OS, `DragSource` (threshold state
  machine), `DragSession` (one per app: deepest accepting target from hit-test candidates, enter/leave
  pairing, leave → drop → drag-end order, cancel, OS file drags as `text/uri-list` copies, cursor),
  ghost and drop-line recording, `edge_scroll` for auto-scroll, `file_uri`/`uri_list`.
- `dialogs/`: `ButtonOrder` (Windows, KDE, GNOME, macOS per QDialogButtonBox), `DialogRole` with `auto`
  resolution, `order_buttons` (for lane WIDGETS-VIEWS' `dialog-buttons` kind); `dialogs::native`: the
  file and message dialogs as futures with `DialogBackend`, `set_dialog_backend`, rfd behind feature
  `dialogs`.
- `zoom/`: zoom levels (1.2 steps), `Zoom` state (clamp, effective scale), shortcut detection per
  platform modifier, `ZoomKeys` (host or presto).
- `table/` (svelters-egui `views/table.rs`, OW5b): `Table` with `TableState` (props by spec name with
  change classes, the `set-rows` JS row window, `set-ids`, `range(first,last)` with overscan, sort,
  `header-state` JSON via serde_json), `ColumnState`, `Geom` (frozen header, sticky columns, stretch-last,
  visible rows/columns, hit tests, reveal offsets), `ColumnResize` (drag with min/max clamp, Escape
  cancel, double-click fit), KeySelect selection keyed by row id, keyboard navigation, type-ahead,
  `copy(ids)`, `activated`, header click sort, `header-context`, recording. Left out: the cell editor
  (needs TEXT-INPUT's field), the column menu popup, the Rust `RowModel` and tree rows
  (`svelters_host::rows` is not a dependency), scroll bars (views' scroll layer), AccessKit, error tooltips.
  Double-click time and drag threshold are local (`TableMetrics`) until WIDGETS-BASIC's Pressable lands.
- `pickers/`: hand-written civil date math (ISO weeks, month arithmetic, `strftime` subset), `calendar`
  (6×7 grid, first day, min/max, keys across months, `changed`/`commit`/`page-changed`), colour models
  (HSV/HSL/RGB, hex with alpha), `color-editor` (SV square as two gradient quads, hue and alpha strips,
  channel scrubbing, swatches and custom colours), the drop button and popup state (`popup.rs`),
  `date-picker` (confirm/immediate) and `color-picker` (live/confirm) models, `spec.rs` rows. Left out:
  hex field typing (TEXT-INPUT), popup placement (views). Deviations: the popup footer follows the OS
  button order (`dialogs::ButtonOrder`); `changed` is not cancelable (no `prevented()` yet); today is UTC
  (`Date::today_utc`); `first-day="auto"` is Monday; `custom-colors-changed` is a new signal name for the
  custom colours write-back (rename to svelters-presto's bind write-back convention).

## Shim (replaced at consolidation) — `crates/presto-widgets/src/table/shim.rs`

| Item | Stands in for |
|---|---|
| `Value` | `presto_core::Value` |
| `Emit` | `presto_core` `Dispatched` JS-facing events |
| `TextStyle`, `TextPainter` (measure, draw one line with elision) | `presto_text::{RichText, TextLayout}` recorded through `RecordCx` |
| `HAlign` | presto-text `Align` |
| `Platform` | presto-core's platform policy (WIDGETS-BASIC's `interact::Policy` on its branch) |
| `fill`, `frame`, `line` | WIDGETS-BASIC's `paint` helpers |
| `testing::FixedText` | test double (deterministic advances) |

WIDGETS-BASIC has a crate-level `shim` with `core::{Kind, KindSpec, Value, …}` and `text::TextShaper` on
its branch. Once it (or presto-core) is in `ci/presto-base`, this shim folds into it.

## presto-core gaps (for lane CORE / the consolidation)

1. **Children's props in `prepare`/`record`/`measure`.** `NodeView::children` gives ids only; `tab-view`
   needs its `tab` children's `title`/`closable`, `table` its `table-column` children's props. Ask: a
   `NodeView` for a child (`PrepareCx::child(i)` or `view(node)` as `EventCx` has).
2. **Styled text.** Closed through the substrate: `host.rs` shapes `RichText` with any style through the
   thread's presto-text `TextSystem` (`table/text_painter.rs`).
3. **Parent-placed children.** `tab-view` lays out only the selected page and hides the others; core's
   layout is not written yet. Ask: a hook for a container kind to place or hide its children.
4. **Popups.** `date-picker`/`color-picker` popups need an overlay layer anchored to the node
   (architecture §14; views lane's placement).
5. **Focus stops inside a kind.** `color-editor` has several inner controls (square, strips, channel
   boxes); Tab between them needs sub-node focus stops or Tab routing to the kind.
6. **DnD.** The DnD props belong on presto-core's common props, and dispatch must feed `dnd::DragSession`
   (hit-test candidates deepest first, pointer capture by the source, Escape cancel, OS file events).

## Cross-lane edits

- `crates/presto-widgets/src/lib.rs` (WIDGETS-BASIC's): one line `pub mod zoom;` and the doc line naming
  it. The lane file assigns `zoom.rs` to this lane but no module group held it.
- `crates/presto-widgets/Cargo.toml`: optional `rfd` dependency and feature `dialogs`, and `serde_json`
  (table `header-state`); `Cargo.lock` gains both under presto-widgets (already locked for other crates).

## Questions for the coordinator (with recommendations)

1. **DnD units vs WIDGETS-BASIC's `interact/`.** svelters-egui's `interact/dnd.rs` (DragSource,
   DropTarget units) sits in `interact/`, which WIDGETS-BASIC ports. This lane wrote presto's DnD in
   `dnd/` (source state machine + app-wide session). Recommendation: WIDGETS-BASIC does not port
   `interact/dnd.rs`; `dnd/` is the one implementation.
2. **dialog-buttons.** WIDGETS-VIEWS' list has the `dialog-buttons` kind; this lane has "dialog button
   order per OS". Recommendation: as done: the order logic lives in `dialogs/` and the views kind calls
   `order_buttons`.
3. **Remaining Kind glue.** Recommendation: once the core gaps above are closed, a fresh session adds
   the `Kind` impls of table, table-column, tab-view, tab, date-picker and color-picker (the models are
   done; the glue follows `pickers/kinds.rs`: a `shim::core::Kind` registered with `host::register`).
   The substrate's contexts also lack children's props (gap 1).
4. **rfd default.** svelters-egui had `dialogs` on by default; here it is off (fewer CI dependencies).
   Recommendation: the presto app crate (`presto` facade or svelters-presto) turns `presto-widgets/dialogs`
   on.

## Not run

Per the batch rule nothing was run beyond `cargo check`/`cargo clippy -p presto-widgets --all-targets`
(warning-free). No test was run; GitHub CI runs them.
