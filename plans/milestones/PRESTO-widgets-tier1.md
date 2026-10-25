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
After CORE-3's hooks landed (ci/presto-base 0ef700c1, merged at `c9e8694e`), the container kinds
implement `presto_core::Kind` directly (the hosted interface carries no child props and hard-codes its
placing kinds): `tabs/kinds.rs` has `tab-view` (`places_children`: the strip as `chrome_padding`, the
selected page shown and the others `display: none` through `child_layout`; captions read through
`NodeView::child`) and `tab`; `table/kinds.rs` has `table` (`folds_children`: its `table-column`
children are data it reads in `measure`/`prepare`; JS methods `set-rows`, `set-ids`, `clear-rows`,
`scroll-to-row`, `fit-columns` through `Kind::call`) and `table-column`. Both shape with the document's
presto-text service (`text_painter::{with_text, record_core}`). Only the popup kinds (`date-picker`,
`color-picker`) have no `Kind` impl (gap 4). Nothing here returns a fake success.

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

1. **Children's props.** Closed: `NodeView::child`/`view` (CORE-3); `tab-view` and `table` read their
   children through them. (The hosted interface, `shim::core`, still carries none, so these kinds
   implement `presto_core::Kind` directly.)
2. **Styled text.** Closed through the substrate: `host.rs` shapes `RichText` with any style through the
   thread's presto-text `TextSystem` (`table/text_painter.rs`).
3. **Parent-placed children.** Closed: `places_children` + `child_layout` and `chrome_padding` (CORE-3).
4. **Popups.** `date-picker`/`color-picker` popups need an overlay layer anchored to the node
   (architecture §14; views lane's placement).
5. **Focus stops inside a kind.** `color-editor` has several inner controls (square, strips, channel
   boxes); Tab between them needs sub-node focus stops or Tab routing to the kind.
6. **DnD.** Coordinator (2026-10-25): CORE-2 adds the common DnD members to presto-core (`drag-type`,
   `drag-data`, `drop-types`; `drag-begin/end/enter/leave/drop` with pointer capture and hit testing in
   core). `dnd::DragSession`/`DragSource` then become a reference for that plumbing (their tests state the
   event order to keep); what stays here for core to call: `accepts`/`type_matches`, `DragPayload::best`,
   `DropAction::from_modifiers`, `uri_list`, `edge_scroll`, and the ghost and drop-line recording.
   Now (0ef700c1) core runs in-app drags (`presto_core::input::dnd`); `dnd/mod.rs`'s doc marks what that
   supersedes. Still open: OS file drags (core ignores `FileHovered`/`FileDropped`), JS-side id payloads
   (core's `drag-data` is a string), and a drag-layer / drop-indication hook from which to record
   `dnd::ghost` (the target only gets `:drop-target` for CSS).
7. **Emitting from `prepare`.** `table`'s `range` (and a restored `header-state`'s events) come out of
   its `prepare`, which cannot emit: the kind holds them and delivers them with the table's next event
   or method call. Ask: an emit queue (or `wake_at`) in `PrepareCx`.
8. **Emitting on, and writing props of, another node.** `close-requested` belongs on the `tab`, but
   `EventCx::emit` emits only from the tab-view (it carries the page index instead); a column resize
   cannot write the `table-column`'s `width` (it stays in the table; `column-resized` reports it). Ask:
   `EventCx::emit_on(child)` and `set_native_on(child)`.
9. **Smaller ones.** `record` has no clock (the table's resize-edge hover delay uses the last event's
   time); a `tab`'s `title` change does not tell its tab-view (the strip refreshes at the tab-view's next
   layout); captions are not AccessKit nodes (core builds one node per tree node).

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
3. **Remaining Kind glue.** Only `date-picker` and `color-picker` (gap 4: overlay layers). Their models
   and specs are done; the glue follows `pickers/kinds.rs`. Recommendation: WIDGETS-BASIC either adds
   child views and a placing list to `shim::core`/`host.rs`, or the container kinds stay on
   `presto_core::Kind` as `tabs/kinds.rs` and `table/kinds.rs` do now.
4. **rfd default.** svelters-egui had `dialogs` on by default; here it is off (fewer CI dependencies).
   Recommendation: the presto app crate (`presto` facade or svelters-presto) turns `presto-widgets/dialogs`
   on.

## Not run

Per the batch rule nothing was run beyond `cargo check`/`cargo clippy -p presto-widgets --all-targets`
(warning-free). No test was run; GitHub CI runs them.
