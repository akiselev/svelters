# POL3-OVERLAYS: a top layer, pickers' open states, tab overflow (batch polish-b3, lane overlays)

**Branch:** `ci/pb3-overlays` (base `master` `242d688`).
**Brief:** `plans/cloud/polish-b3/BRIEF.md`, `LANE-overlays.md`.
**Status:** done (pending the last CI run; see "Tests and CI"). All three items built, seen in the gallery, tested.

## Design note: the top layer (POL2-REGISTER Q2)

What exists: each window keeps an overlay stack (`skelter-core/src/overlay.rs`, `toplayer.rs`). A kind records an
**overlay part** (a display list, `RecordCx::overlay`): menus, the combo's list and the pickers' drops already draw
there, in a layer above every content layer, hit before the content, light-dismissed and closed by Escape in order.
What is missing is a **node subtree** in that layer: a `popup`'s or `modal`'s Svelte children, a `tooltip` element,
the toast host. They are laid out and drawn in their owner's subtree, so a clipping or scrolling ancestor cuts them, a
later sibling covers them, and a popup takes room in its parent's flow.

The shape (as the web's top layer: the node stays in the tree, only its box and paint move):

1. **Membership.** A node whose taffy style is `position: fixed` is in its window's top layer. Kinds set it in
   `Kind::layout_style` while they are open (CSS `position: fixed` keeps mapping to `absolute`; this is not the CSS
   value). taffy 0.15 already bubbles a fixed box to the root, which claims it: the node is laid out against the
   window, out of every ancestor's flow and overflow. `out_of_flow` and `positional_parent` treat `fixed` as
   out-of-flow with the window root as containing block.
2. **What it is:** `Kind::top_layer(&self, &TopLayerCx) -> TopLayer` (`#[non_exhaustive]`: class, `at`, inside rects,
   opener, takes input, modal, dismiss flags; defaults from `OverlaySpec::new`). `at` is where the border box goes in
   window points (`None`: where layout put it). The kind computes it with `views/placement.rs` from its anchor's box
   (`TopLayerCx::world_rect`, this rebuild's, so the anchor is placed first; the default anchor is the owner) and its
   own size.
3. **Frame.** The rebuild walk defers each top-layer node it meets and walks its subtree after the whole content (and
   after the top-layer nodes found before it): its transform node hangs off the window root at `at`; no ancestor's
   clip, effect or scroll applies; its subtree's items go in its own layer. That layer is an entry of the window's
   overlay stack (its z from the stack's bands and open order, as an overlay part's), so a combo's list opened inside
   a popup goes above the popup, a tooltip above both. A top-layer node inside another one opens from it (its
   parent entry). The stack's layer deltas are sent before the items of the layers it adds.
4. **Input.** The stack's hit routing comes first, as for parts; a press on a top-layer entry is hit-tested inside
   that node's subtree (the hit list's entries for it), so it reaches the button under the pointer. Light dismiss and
   Escape close entries in the stack's order (children first, one level per Escape); the kind hears it in
   `Kind::overlay_closed` and stops being `fixed`.
5. **Ownership.** The node never leaves its owner: removing the owner removes it (its entry closes at the next
   rebuild), focus order and the accessibility tree are the tree's, so AccessKit keeps the owner as its parent (bounds
   are its new window box relative to the owner's).

Not done: a top-layer node does not take a CSS transform of its own (only `at`); `fit` (shrinking a popup to the room)
is left to the display-list kinds that already do it.

Built as designed. One core change beyond the note: a `Structure`-class prop write now re-runs the kind's layout
style, as a `Layout`-class one did (popup's and modal's `open` are `Structure`; before, only `Layout` set
`Dirty::STYLE`). CI on `4f824d1` (that commit alone) was green in all six workflows.

## What changed

1. **Top layer (core):** `layout/mod.rs` (`in_top_layer`, `fixed` out of flow with the root as containing block),
   `frame.rs` (deferred walk, `enter_top`, the stack's deltas before the top layer's items, `top_at`),
   `toplayer.rs` (`sync_overlays` keeps top entries; `overlay_hit` hit-tests inside a top-layer subtree),
   `overlay.rs` (`TopLayer`), `cx.rs` (`TopLayerCx`), `kind.rs` (`Kind::top_layer`), `input/dispatch.rs` (`hit_in`).
2. **Kinds on it** (skelter-widgets): `popup` (open: fixed, placed next to its owner or `anchor` with
   `placement.rs`, its anchor counts as inside, light dismiss by the stack instead of the subtree rule), `modal`
   (open: fixed, inset 0: covers the window, a modal stack entry; it still cancels itself), `tooltip` element (fixed,
   `top-center` of its owner, click-through), `toast-host` (fixed, inset 0, click-through, hit only on its toasts).
   Shim `Kind::top_layer` and the host's forwarding (`shim.rs`, `host.rs`).
   Menus, the combo's list and the pickers' drops were already overlay parts (display lists) in the same stack.
3. **Pickers:** `open` (bindable, written back when the user opens or closes the drop) on `date-picker` and
   `color-picker` (`combo-box` had it). `pickers::Today`, a document service for the pickers' today
   (`pickers::set_today`; handed to the kinds before each prepare by shim `Kind::today`); the gallery pins
   2026-10-22 (`skelter-gallery/src/render.rs`). An empty date picker shows its format's fields dimmed
   (`yyyy-mm-dd`), not today.
4. **Tab overflow:** an overflowing strip has scroll buttons (one caption per press, dimmed at the ends) and an
   overflow button at its end, opening a menu of every tab (the shown one checked; Up/Down/Home/End/Enter/Escape;
   an overlay part of the window's stack, light-dismissed); `menu-open` mirrors it (`bind:menu-open`). The wheel
   still scrolls; a selection written before the first layout is revealed.

## Findings (polish)

| # | Finding (story) | Fix | Seen fixed |
|---|---|---|---|
| O-1 | `popup/*`: popup in its parent's flow, `placement` ignored (R-19) | top layer, placed next to its owner | yes (`a-popup.png`: four placements, flips) |
| O-2 | (new story `popup/clipped`) a popup inside a short scroll area was cut by it and covered by the panel after it | top layer | yes (`a-popup.png`, `z2-popup.png`) |
| O-3 | `date-picker`, `color-picker`: the drop could not be shown (R-20, Q5) | `open` prop; stories `*/open` | yes (`a-date-picker.png`, `a-color-picker.png`) |
| O-4 | `calendar`, `date-picker/empty`: today was the real clock; an empty picker showed today (R-16, Q4) | `pickers::Today`, gallery's fixed day; placeholder | yes (`a-date-picker.png`) |
| O-5 | `tab-view/closable`: overflowing captions cut with no sign (R-15, Q3) | scroll buttons, overflow menu | yes (`a-tabs.png`, `a-tab-view-closable.diff.png`) |
| O-6 | (found) `tab-view`: `selected` written before the first layout was not scrolled into view | reveal at relayout | yes (`a-tabs2.png`) |
| O-7 | (found) `tooltip/*`: a long tooltip ran off a 320 pt window's side (hover tip and element) | wrap to the window's width less 8 pt each side | yes (`a-tooltip2.png`) |
| O-8 | (found) pickers in HC Dark/HC Light drew with the Modern theme of their scheme | the document's theme | yes (`z1-date-hc.png`) |
| O-9 | (found) HC Dark: the picker's Save button black on black | `button-border` edge | yes (`z1-date-hc2.diff.png`) |
| O-10 | (found) HC Dark: the overflow menu's current row invisible | outlined with `focus` in HC | yes (`a-tabs3.png`) |

Before/after images (this container, not committed; "before" from a gallery built at `242d688`), all in
`target/gallery/pb3/`: `a-popup-form.diff.png`, `a-modal-confirm.diff.png` and `a-toast-levels.diff.png` (unchanged
pixels where the owner already covered the window: the same look, now from the top layer),
`a-tab-view-closable.diff.png` (buttons, the selection revealed), `z1-date-hc2.diff.png`. Sheets looked at: popup,
modal, toast, tooltip, date-picker, color-picker, tab-view in dark-modern, light-modern, hc-dark and hc-light, zoom 1
and (popup, date-picker, tab-view) 2. No snapshot test moved; none re-blessed.

Seen and left (not this lane's, or a design question): the `focus-visible` state shows no ring on a `tab-view`
caption in the gallery (the forced state does not reach a core kind's own focus flag); `popup/clipped`'s panel is
invisible in HC Dark (`--sv-surface` is black there; story only); the colour picker's drop shows `#2e86c1` for
`#2e86c180` (alpha `opaque` by default: by design).

## Coordinator request: Q-E2 (a window that never went idle)

From cadatomic gal3 (on `674d7de`): with the pointer over a `<label>` in a property-grid virtual row,
`Document::needs_frame` stayed true for ever. Cause: `LAYOUT`/`DESC_LAYOUT` were cleared only by the record walk,
which skips nodes with no record work, so a relayout that moved nothing left them on the window root. Fix (own
commit): the layout pass clears them on each node it lays out (`Tree::set_final_layout`, `layout/mod.rs`). Test:
`skelter-core` `tests::runner::a_relayout_that_records_nothing_leaves_the_window_idle` (a hover restyle that relays
out with no geometry change; failed before the fix). `cargo test -p skelter-core --lib` 191, `-p skelter-widgets
--lib` 575 pass.

## The finish lane's requests (POL3-FINISH "Requests to the overlays lane")

After merging `origin/master` (`0a3ab14`, the finish lane; `skelter.json` regenerated, no other conflict):

| # | Request | Fix | Seen |
|---|---|---|---|
| O-1 | `dialog/confirm`: Cancel (first tab stop) took the default look under auto-default | core: a dialog scope's initial focus is its usable `default` button (`input/focus.rs` `initial_in`, `input/scope.rs` `default_button`); test `tests::hooks::a_dialog_opens_on_its_default_button` | yes (`a-dialog.png`: Save default) |
| O-3 | `menu-bar/titles`: no focus ring; HC hover a fill; a host's disabled state did not reach the titles | `TITLE_UA` focus-visible ring on the current title, `TITLE_HC_UA` (dashed hover, solid open outline in `focus`), host `:disabled` dims the titles (`views/menu.rs`) | yes (`a-menubar.png`, dark-modern and hc-dark at zoom 2) |
| O-5 | pickers in HC and every density; today vs cursor; drop buttons; disabled colour editor; white handles; empty date picker | the document's theme (density metrics with it, O-8 above); today is a bar under its number (the cursor keeps the ring); the picked day outlined in HC; drop buttons tint on hover/press (node state or the drop's own); a disabled colour editor fades whole (`color_editor::fade`); handles edged in dark; the empty picker's placeholder (O-4 above) | yes (`a-cal.png`, `a-dp-states.png`, `a-ce.png`) |
| O-15 | tooltip wider than the window | O-7 above | yes |
| tabs | the strip 27 pt against `tab-h` 35; a disabled strip's full-accent marker | `TabLook::min_height` from `tab-h`; inactive captions `tab-fg`, the active one `tab-active-fg`; the marker in `disabled-fg` when disabled | yes (`a-tabs4.png`, four themes) |

Left from O-5: an HC hover outline (dashed) for the drop buttons (they tint, as in the Modern themes).

## Cross-lane edits (small, additive; for the coordinator)

- `crates/svelters-skelter/src/surface/tier1.rs` (finish lane's): three surface rows, `date-picker` `open`,
  `color-picker` `open`, `tab-view` `menu-open`, with their `native_state`; `metadata/semantics/skelter.json`
  regenerated (`SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec`); `gen_props.py --check` clean. At a
  merge with the finish lane, keep both sides' rows and rerun the generator.
- `crates/skelter-widgets/src/{shim.rs,host.rs}` (the rest of `src/`, finish's): `Kind::top_layer` and `Kind::today`
  hooks and their forwarding (a few lines each).
- `crates/skelter-gallery/src/render.rs`: the gallery's fixed today (`GALLERY_TODAY`).
- Stories: `dialogs/popup` (owner box, `placement` args, `clipped`), `pickers/{date-picker,color-picker}/open`,
  `tabs/tab-view/overflow`, notes in `tooltip`, `calendar`.

## Tests and CI

Added or changed (filtered runs on the last code commit, all pass):
- `cargo test -p skelter-core --lib` 191 (Q-E2's test, and 3 new `tests::toplayer`: a menu-like node in a clipped box draws whole
  above a later sibling and takes the click; Escape one level and outside presses children first; the a11y parent
  is the owner and removing the owner takes the entry and its layer).
- `cargo test -p skelter-widgets --lib -- pickers:: tabs:: views::` 193 (new: popup top layer, picker `open` and
  write-back, date picker placeholder and `set_open`, tab overflow buttons and menu, the reveal on first layout;
  changed: popup light dismiss, the overflow test's edge, the empty date picker's text).
- `cargo test -p svelters-skelter --test spec --test widgets_all` 11.
- `skelter-gallery check`: 47 stories, 174 variants, no problems.
- Not run locally: the full workspace, GPU gallery tests, conformance (CI runs them).

CI: `09e2f13` (top layer, pickers, tabs, Q-E2) green in all six (js 40633564142, linux-gtk 40633564088, linux-qt 40633564161, macos-appkit 40633564091, skelter 40633564100, windows-winui 40633564110). `4f824d1` green in all six (js 40630278355, linux-gtk 40630278372, linux-qt 40630278336, macos-appkit
40630278348, skelter 40630278422, windows-winui 40630278420). Last code commit: see the end of this file.

## Questions (recommendations, for the coordinator)

- **Q1 (`anchor-to`):** a popup anchors to its owner; the Rust `anchor` record (`{node}`, rect, point) is still
  shadowed in Svelte by the common `anchor` (a name). Recommend a surface prop `anchor-to` naming another element's
  `anchor` (core's anchor names, as `for` resolves them), so `<button anchor="t"/><popup anchor-to="t">` works
  without a wrapper box. Small (kind prop + surface row); a design choice on the surface, so not built.
- **Q2 (CSS `position: fixed`):** CSS `fixed` still maps to `absolute`; only kinds put nodes in the top layer.
  Recommend keeping it so (the web's `fixed` is not the top layer); revisit if cadatomic needs viewport-fixed
  boxes.

## Requests to the finish lane

- Keep the three surface rows above at a merge (rerun the skelter.json generator).
- `tab-view` captions get no focus ring under the gallery's forced `focus-visible` (finding above), if the review
  of every story touches the forced states.
