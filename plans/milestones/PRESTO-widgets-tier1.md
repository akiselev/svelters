# PRESTO-widgets-tier1: presto-widgets tables, tabs, pickers, DnD, dialogs, zoom

Lane WIDGETS-TIER1 of presto batch 2 (`plans/cloud/presto-b2/LANE-widgets-tier1.md`). Session
`session_01DjBCnf7oMK3XEzkghWoiQs`, branch `ci/presto-widgets-tier1` from `ci/presto-base` e34079e6.

## State

presto-core and presto-text had no API skeleton on `ci/presto-base` when this lane ran (both `lib.rs`
were doc lines only), so every tier-1 widget is written as **toolkit-independent native state, geometry,
input state machines and recording** over presto-base types, plus a small local shim
(`crates/presto-widgets/src/table/shim.rs`). The `presto_core::Kind` impls (KindSpec statics, `record`,
`event`, `a11y` glue) are **not written**: they are thin wrappers once `presto_core::Kind` exists, and the
consolidation (or a follow-up of this lane) adds them. Nothing here returns a fake success.

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
- `table/`, `pickers/`: see their sections below (written by forks of this session).

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

## Cross-lane edits

- `crates/presto-widgets/src/lib.rs` (WIDGETS-BASIC's): one line `pub mod zoom;` and the doc line naming
  it. The lane file assigns `zoom.rs` to this lane but no module group held it.
- `crates/presto-widgets/Cargo.toml`: optional `rfd` dependency and feature `dialogs` (`Cargo.lock` gains
  `rfd` under presto-widgets; rfd 0.17.2 was already locked for svelters-egui).

## Questions for the coordinator (with recommendations)

1. **DnD units vs WIDGETS-BASIC's `interact/`.** svelters-egui's `interact/dnd.rs` (DragSource,
   DropTarget units) sits in `interact/`, which WIDGETS-BASIC ports. This lane wrote presto's DnD in
   `dnd/` (source state machine + app-wide session). Recommendation: WIDGETS-BASIC does not port
   `interact/dnd.rs`; `dnd/` is the one implementation.
2. **dialog-buttons.** WIDGETS-VIEWS' list has the `dialog-buttons` kind; this lane has "dialog button
   order per OS". Recommendation: as done: the order logic lives in `dialogs/` and the views kind calls
   `order_buttons`.
3. **Kind glue.** Recommendation: once presto-core's `Kind` is in `ci/presto-base`, this lane (fresh
   session) adds KindSpec statics and `Kind` impls for table, table-column, tab-view, tab, date-picker,
   color-picker, calendar, color-editor; DnD props go on presto-core's common node props.
4. **rfd default.** svelters-egui had `dialogs` on by default; here it is off (fewer CI dependencies).
   Recommendation: the presto app crate (`presto` facade or svelters-presto) turns `presto-widgets/dialogs`
   on.

## Not run

Per the batch rule nothing was run beyond `cargo check -p presto-widgets` (and `--tests`, in a scratch
copy of the crate while the forks' files were incomplete). No test was run; GitHub CI runs them.
