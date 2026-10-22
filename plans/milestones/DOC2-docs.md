# DOC2: own widgets, styling, icons, automation, loader and HMR docs

Lane DOC2 (thinkpad), branch `ci/doc2` from master d0542a3a. Docs only: nothing was built or run.
Assignment (coordinator, 2026-10-22): the next pages from DOC1's plan, in the order cadatomic's agents need
them. This replaces the DOC2 row of `plans/research/DOCS-PLAN.md` §7.1 (Rust integration core), which is
still open for a later lane.

## Pages written

All are hand pages under `docs/egui/`, with DOCS-PLAN §4's front matter (`title`, `description`, `status`,
`verified_at`, `sources`). Links are relative. Code blocks that weren't compiled or run are marked
`// not run`. The two examples copied from tested files say which test checks them.

| Page | Covers | Sources |
|---|---|---|
| `docs/egui/rust/own-widgets.md` | `#[svelters::widget]` for own widgets (`#[behavior]`, `#[part]`, `replaces`, `style_as`, superset extras), `KindSet`/`SVELTERS_OWN_WIDGETS`/`DEFAULT_OWN`/`OWN_ONLY`, the units (Hoverable, Pressable, Focusable, Toggleable, Resizable, Scrollable; Expandable and Roving marked pending), new units, scene-free `interact::run`, `StyleView`/`Ua`/primitives, part painters, `dispatch(..).prevented()`, `with_peer` (pending), `consume_key`/`record_part_focus`, `cached_shapes` and its invalidation, PaintTrace, worked examples | `provider/mod.rs`, `interact/*`, `paint/*`, `widgets/*`, `svelters-macros` docs; OW0, OW1a, OW1a2 (ci/ow1a2), EV1, EV3, SG6, OW-CSS records |
| `docs/egui/css/own-widgets.md` | `::part()` (state on the host, multiple names, P1 logical positions, P3 part layout), which widgets read part rules today, list rows (`virtual-list::part(row)` per-row state, marked pending OW5a; from `ci/ow5a` d80f80e5's `OW5a-lists-tree-dnd.md`), the state pseudo-class ↔ bit table, `content: icon()/none/paint()/url()`, `paint(name)` painters, box features, transitions (R11 delay, `transition: none`, reduced motion), the R-rules in CSS | OW-CSS (both stages), `svelters-style/src/state.rs`, `provider/state.rs`, `paint/painters.rs`, gallery `23-own-buttons.svelte`, DESIGN §2.2 |
| `docs/egui/icons.md` | **pending merge (ICR1 on ci/icr1 102b4ffb)**: keys, `ext.<id>.*`, `Owner`, `Icon::svg/master/hc_master/glyph`, `IconData`, `register_set`, per-context registries, optical-size choice (Z1/Z2), tint and `--icon-subject/-accent/-badge` slots, `icons::paint` from Rust UI | `icons.rs` and the ICR1 record on ci/icr1, `paint/icon.rs`, the regenerated `icon` reference page on ci/icr1 |
| `docs/egui/rust/automation.md` | `#[svelters_automation::test]` and `E2e`, `@svelters/test` and `svelters-e2e` (workers, reset, seeds, retries, reporters), `mount`/`setProps`, locators, the virtual clock and seeds, `svelters-run`, `Session` over a `Driver`, `Server`/clients | `svelters-automation` sources, `tests/e2e.rs`, `packages/test/test/e2e/*.e2e.js`; AU1, AU1b, AU2, AU2b |
| `docs/egui/rust/loading-code.md` | `SourceProvider`/`SvbProvider`/`BundleProvider`, `SourceConfig`, what the loader compiles (TS, JSON, assets, `import.meta.glob`, egui `<style>`), the overlay and `check`, `ModuleHooks` (schemes, visibility, natives, `isolate()`, `for_extension`), `.svb` and `svelters-bundle`, `--app`, the bytecode cache, the Q4b startup table | `svelters-modules`, `svelters-host` (`lib.rs`, `hooks.rs`, `bytecode.rs`), runner bins; Q1, Q2, Q4, Q4b, LD1, LD2, LD3 |
| `docs/egui/rust/hot-reload.md` | turning HMR on (`dev`+`hmr`, `watch`, `apply_changes`, `ReloadReport`), what an edit does, what's kept (H3 signature rule, SV36, H3c children), `@hmr:reset`, `setHmrPreserveState`, fallbacks and the swap budget, embed behaviour | `svelters-host` `reload.rs`/`lib.rs`/`core.rs`, runtime `index.js`; H1, H3, H1b+H3c, LD2, LD3 |

No site or sidebar config was touched: `docs/egui` has no site wiring yet (DOC11), and `START-HERE.md`
doesn't exist on master yet (DOC0), so the new pages aren't linked from an index. Whoever writes
START-HERE should link them. Paths follow DOCS-PLAN §5.2 (`loading-code.md`, `hot-reload.md`,
`automation.md`) and §5.6 (`icons.md`). The plan predates the own widgets, so `rust/own-widgets.md` and
`css/own-widgets.md` are new paths; the plan's §5.2 table should gain them.

## Gaps between code and records (for the coordinator)

1. **`::part()` rules don't reach most own widgets.** OW-CSS stage 2 fills `part_computed`, but only
   `button` reads it (`cx.part`). `checkbox`, `switch` and `progress-bar` on master, and also `radio-button`,
   `frame` and `collapsing-header` on ci/ow1a2, paint parts through `widgets/parts.rs::part_view`: the host's
   computed style over a hard-coded UA table, still marked `TODO(OW-CSS ::part)`. STATUS says "part UA
   tables hard-coded until OW-CSS stage 2", and stage 2 has merged, so this is now a follow-up for OW1a3 or
   OW-CSS: switch `part_view` to `cx.part`. The CSS page states the current behaviour.
2. **`with_peer`, `PeerCx`, `Expandable`, `Roving`, `sibling_ids`/`tag_of`/`focus_node` are on ci/ow1a2 only**
   (f7cdbbe6), not on master. The guide marks them pending. OW1a2's record says `with_peer` is crate-level
   (not re-exported), so app crates such as cadatomic can't write group widgets with it. If cadatomic needs
   that, it's an API request.
3. **`focus::record_with` is `pub(crate)`.** The assignment lists it as an API. Outside svelters-egui, the
   only seams are `WidgetCx::record_part_focus` (Role::Slider, tab stop) and `Widget::consume_key`. The
   guide documents those and names `record_with` as crate-private.
4. **`DEFAULT_OWN` is `["button"]` on master.** The OW1a slice-1 widgets are own only with
   `SVELTERS_OWN_WIDGETS=all` or a tag list until OW1a2 flips them. That matches STATUS, but cadatomic
   agents may assume the flip.
5. **The OW0 guide's step 5.1** (`cx.set_state(StateBits::CHECKED, …)` before `interact`) duplicates
   Toggleable, which owns `checked`/`indeterminate` and sets them from its params. `widgets/checkbox.rs`
   doesn't call it. The new guide follows the code.
6. **The SG6 record** says only a prop write or freeing the node drops a `cached_shapes` entry. OW-CSS stage 2
   added restyles and running transitions (`Scene::step_style_transitions`). The code follows OW-CSS. The
   SG6 record could point there.
7. **`content: url(…)`** parses (OW-CSS P2) but the icon primitive doesn't draw URLs (OW0 "Not done", until
   ICR1's AS1 swap). ICR1 reads `Content::Url` in `content_icon` but draws only registry keys and glyphs.
8. **ICR1's STATUS row says QUEUED**, while ci/icr1 has a done record (102b4ffb). `icons.md` is marked
   `status: pending` and `verified_at: ci/icr1@102b4ffb`. When ICR1 merges, change `status` to `available`
   and `verified_at` to the merge sha, and drop the banner. Recheck the `register` examples if the API
   changes in review.
9. **`widgets/mod.rs`'s module doc** points to `plans/milestones/OW0-widget-foundation.md` for "Write a widget".
   It should point to `docs/egui/rust/own-widgets.md`. That's a code edit, which this docs lane didn't make.
10. **Superseded statements in records (no action needed; the pages follow the later lanes):** AU2's
    "`--clock real` not supported" (AU1b added it), LD2's "scope class from the absolute filename" (Q4b
    uses the logical id), and H1's "an `acceptExports` mismatch falls back to a full reload" (H1b
    propagates it).
11. `rust/own-widgets.md` is 318 lines, over DOCS-PLAN §4's 300-line target, because of the full example.
    A later pass can move the example to `examples/egui-docs` once DOC4's snippet checker exists.

12. **OW5a is on `ci/ow5a`** (d80f80e5), not on master. The list-rows section of `css/own-widgets.md` follows
    its record and is marked pending. When it merges, drop the banner.

## Follow-up (after OW1a2 and OW3 merged)

- OW1a2 (ba5f7239): the pending markers on `Expandable`, `Roving` and `with_peer` are gone, and the
  worked examples point to master.
- OW3 (e832e001): `rust/own-widgets.md` documents `RovingList` (an index inside one widget) next to
  OW1a2's `Roving` (focus across sibling nodes), when to use each, and a "Floating surfaces and tooltips"
  section (`Anchored`, `Dismissable`, `Typeahead`, `Tooltipable`; the common `tooltip` prop needs no
  widget code; `widgets::tooltip::{show, hide, is_shown}`).
- Gap 1, updated: `css/own-widgets.md` now has a per-widget table of which parts read `::part()` on
  master. Still on built-in tables: checkbox `mark`, radio `dot`, switch `knob`, frame `indicator`/`mark`,
  progress `track`/`fill`, collapsing `header`/`twistie`.
- Gap 4 is resolved: `DEFAULT_OWN` now includes the OW1a/OW1a2 kinds (not `label`) and OW3's kinds.

## Not run

Nothing was built or run (docs lane). No code block was compiled. The gallery CSS and the e2e examples are
copied from tested files (`tests/gallery.rs`, `svelters-automation/tests/e2e.rs`,
`packages/test/test/e2e/*.e2e.js`). Everything else is marked `// not run`.
