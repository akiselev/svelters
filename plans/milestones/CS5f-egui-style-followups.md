# CS5f: egui style follow-ups (typography, leaf padding, multi-value shorthands)

Design: `plans/research/CSS.md` §2.7 ("egui: in-process"), §2.8; `plans/milestones/CS5e-egui-cascade.md`
("Deferred"); `plans/research/TRACKS.md` STYLE/WIDGETS rows; CADATOMIC SVP6 (fonts). This lane covers the CS5e
deferred items that cadatomic's SC1 needs most. Built on CS5e (master) and WG1 (`origin/ci/wg1`: `fonts::register_weight`,
the SC1 widget kinds). Lane: CS5f (STYLE writer), branch `ci/cs5f`.

**Decided (coordinator, 2026-10-22):**
- On egui, `margin` and `padding` are **engine properties** (`via: css`, `decl_type: insets`, 1 to 4 values), not
  setters. Inline `style=` sends them to the engine as part of its inline block (the runtime already sends the raw
  `style=` text on egui), and `<style>` rules reach the engine as before. A `margin`/`padding` attribute is still a kind
  prop, set explicitly, and it beats every rule. This fits `Table::computes` (dialect `none`) and CSS.md §2.8. The
  alternative was a setter typed `insets`: the compiler would then lower `padding: 4px 8px` to a
  `{top,right,bottom,left}` record, and the egui kind props (`F64`, `Common::margin/padding`) can't take that without
  a scene-wide change.
- **Merge order:** `ci/cs5f` is stacked on `ci/wg1`, so WG1 (with WG1b) merges to master first, then CS5f, after
  `origin/master` is merged in again.

### Behaviour change

Inline `margin`/`padding` in `style=` are **no longer explicit props**. They are the engine's inline declarations, so
an `!important` author rule now beats them, as in CSS. Attributes (`padding={8}`) are unchanged.

## What landed

| File | Content |
|---|---|
| `style.rs` (STYLE) | `ENGINE_PROPS` gains `font-family` (raw), `font-weight` (`font-weight`), `margin` and `padding` (`insets`). `leaf_style(scene, i, ctx, base)` builds the one `egui::Style` a leaf is measured and painted with: every text style at the computed `font-size`, `font-family` (the first family of the list the context can draw; `sans-serif`/`serif`/`system-ui` → Proportional, `monospace` → Monospace, other names → `FontFamily::Name` only if bound; never Monospace's text style) and `font-weight` (`fonts::weighted_family`: the registered family for the nearest weight, with WG1b's glyph check). An explicit `font-weight` prop wins, and the leaf applies it. For button-like kinds (`button`, `toggle-button`, `combo-box`, framed `icon`), the CSS padding becomes `Spacing::button_padding`. `content_rect` is the leaf's rect less its taffy padding. `layout_style` reads `margin`/`padding` as `Insets`, and it applies `padding*` to leaves padded around the widget (every built-in leaf except the button-like ones). `restyle_node` remeasures a leaf (`mark_measure`) when a font or padding value changes. `before_leaf` sets `leaf_style` on the child `Ui` before the visuals. |
| `layout.rs` (SCENE/STYLE, small) | `styled_metrics`: a `Metrics` over `leaf_style` for leaf measure (`compute_child_layout`) and `remeasure`. |
| `walk.rs` (EMBED, one line) | The leaf's child `Ui` gets `style::content_rect` as its max rect. |
| `leaf.rs` (WIDGETS, one line) | `size` is the child `Ui`'s max rect (the content box), so sliders, text edits and combo boxes fill the padded box. Unchanged when there is no padding. |
| `fonts.rs` (WIDGETS, one word) | `weighted_family` is `pub(crate)`. |
| `metadata/semantics/egui.json`, `crates/svelters-style/src/tables.generated.rs`, `packages/style/src/props.generated.js` | Regenerated (`SVELTERS_UPDATE=1 … --test spec`, `gen_props.py`). |
| `svelters-compiler` `tests/g1_egui_spec.rs` | `inline_margin_and_padding_go_to_the_engine`. No compiler code change: `is_setter_property` is false for engine properties, so the declarations stay in the inline text. |

### Merge with WG1 (for WG1's own master merge)

`b3bb5168` merged `origin/ci/wg1` into the lane and resolved the CS5e/WG1 conflicts like this:
- `walk.rs` `paint_own`: CS5e's `before_container` and the opacity child `Ui` come first, then WG1's
  `crate::leaf::container(arena, i, r, origin, ui, out)` in place of CS5e's child loop.
- `layout.rs` `compute_child_layout`: WG1's match, `W::Grid` → grid, `W::CollapsingHeader` → `tree.collapsing`, then
  `w if w.is_container() && style.display == Display::Grid` → grid (CS5e), then flexbox. Both branches added an
  identical `impl taffy::LayoutGridContainer`, and only one is kept.
- `Cargo.toml`: taffy features of CS5e (`grid`, `calc`, …) are kept.
- `egui.json` and `props.generated.js` were taken from WG1 in the merge and regenerated in the CS5f commit.

WG1 later merged CS5e itself (`ef0584a7`) and resolved these hunks the same way. The lane merged it again,
with one formatting-only conflict in `layout.rs` (WG1's form kept) and regenerated tables, and then merged
`origin/master` (WG3), which conflicted nowhere.

## Acceptance

- `tests/style_text.rs::fonts_remeasure_and_repaint`: `font-size: 28px` added by a class change remeasures (the layout
  rect grows) and repaints (the drawn rect grows, and the drawn height equals the measured height). It is inherited
  from a box, also on a button. Removing the class remeasures down. `font-weight: bold` draws in the family
  registered for 700, and it is measured with the painted font. An explicit `font-weight` prop of 400 wins over the
  rule. `font-family: "Not Installed", monospace` skips the unbound name with no warning and draws monospace.
- `tests/style_text.rs::leaf_padding_and_shorthands`: a label with `padding: 4px 8px` measures 16 wider and 8 taller
  and paints at (+8, +4). A button with `padding: 2px 10px 6px 20px` is `30 − 2·button_padding.x` wider than an
  unpadded one, its height is text + 8, and its frame fills the rect. `margin: 1px 2px 3px 6px` places a label.
- `tests/style_text.rs::inline_padding_and_explicit_attribute`: inline `padding: 4px 8px` beats `box { padding:
  20px }` in the engine (computed `Insets`, child placed at (+8, +4)). A `padding` attribute of 8 beats the rule.
- Compiler: `g1_egui_spec::inline_margin_and_padding_go_to_the_engine`.

## Run locally, and not

Run (filtered, after the last code change): `svelters-egui` `binary(style_text)`, `binary(style)`, `binary(widgets)`,
`binary(spec)`; `svelters-compiler` `binary(g1_egui_spec)`; `svelters-style` `generated_tables_are_sorted_and_typed`;
`packages/style` `test/docs.test.mjs`; `gen_props.py --check`; clippy on `svelters-egui` (no new warnings).
**Not run locally:** the full workspace nextest, the rest of the svelters-egui, compiler and runtime suites, the JS
suites that import `svelte` (no `node_modules` in the worktree), conformance, and the GTK/Qt/AppKit/WinUI workflows.
GitHub CI is the gate.

## Deviations, and why

- **Button padding is symmetric in paint.** egui centres a button's text in its frame, with one `button_padding` for
  both sides. Measure uses the exact sides (left + right, top + bottom). Paint uses their means, so `padding: 2px
  10px 6px 20px` has the right size, with the text centred.
- **`text-edit` is padded around its frame**, not inside it. `TextEdit::margin` is a builder argument, not part of the
  `Style`, and changing it would mean more WIDGETS code. Measure and paint agree.
- **`%` padding on leaves counts as 0 in `leaf_style`** (button padding). Leaves padded around the widget resolve
  `%` through taffy.
- **Provider (`Ext`) leaves get no CSS font or padding.** Their measure and paint are the provider's own (RA1). A
  provider can read `EguiBackend::computed_style`.
- **The collapsing header's caption** is not restyled (it is a container; its header font is the global one).

## Deferred

- Absolute positioning and insets (`top`/`left`/…): after SG4 (hoisted children).
- `box-shadow`, `line-height`, `font-style`, `text-align`; leaf box paint beyond `WidgetVisuals` for stock widgets.
- Inline `style=` setters other than margin/padding through the engine (the rest of D9's inline step).
- Kittest snapshot gallery (needs the `wgpu` kittest feature).
