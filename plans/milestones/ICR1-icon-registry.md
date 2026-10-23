# ICR1: the icon registry and `<icon>` by key

Lane ICR1, branch `ci/icr1` (thinkpad). Coordinator's assignment (2026-10-22), from the ICR1 row in `STATUS.md`.
Inputs: `plans/research/ICON-STYLE.md` (IC0, approved direction), `plans/research/ICONS.md` (Decided block),
AS1's record (`AS1-asset-imports.md`: the SVG loader on resvg/usvg 0.45.1, `asset://`, `icon source`/`tint`),
`plans/research/ARCH-WIDGETS.md` §1.1 (the icon primitive), `WIDGET-PARITY.md` P2.

## What landed

### `svelters_egui::icons` (new)

- **Keys.** `validate_key`: one or more lowercase dot-separated kebab segments (`[a-z0-9]+(-[a-z0-9]+)*`), plus an
  optional `+<badge>` segment for IC1's composed ids (`mcad.extrude+remove`).
- **Namespaces.** `Owner::Host` registers any key outside `ext.`. `Owner::Extension(id)` registers only
  `ext.<id>.<name…>`, where the id is one kebab segment. This is a namespace rule and not a policy: no audit and no
  capability (the security scope was cut). A refusal is returned as `IconError` and queued as a register-time
  diagnostic (`take_diagnostics`). `unregister(owner, key)` removes only the owner's own key, and
  `unregister_owner` removes all of them (an extension unloading).
- **`Icon`.** `Icon::svg(size, data).master(size, data).hc_master(size, data)` takes one SVG per optical size
  (12/14/16/20/24/32+) and IC1's High Contrast copies. `Icon::glyph(family, text)` is an icon-font glyph (SVP6).
  `IconData` is `Static(&'static [u8])` (embedded), `Shared(Arc<[u8]>)`, or `Asset(Arc<Assets>, AssetRef)`. An
  asset is re-read when its AS1 version changes; no file I/O happens in svelters-egui (ZU3).
- **`IconRegistry`.** `register`, `get`, `contains`, `keys`, `resolve(key, box_pt, ppp) -> Choice`, and
  `register_set(owner, &Arc<Assets>, root, dir)`. The last one registers an IC1 `icons.py build` output from its
  `manifest.json`: masters at `<size>/<id>.svg` and HC copies at `hc/<size>/<id>.svg`, all read through Assets.
  `IconRegistry::global()` is the process registry. `set_registry(ctx, reg)` gives a context its own, and
  `registry(ctx)` reads it.
- **Master choice, rule Z2** (`choose_master`, ICON-STYLE §5.3): an exact master; else `P = 2m`; else the
  largest master (or ×2 master) at most 2 px smaller, at its own size, centred with an offset of
  `floor((P−m)/2)`; else the nearest master (or ×2 master), scaled. It reproduces §5.3's table for a 16pt icon at
  zooms 1.0–2.0.
- **`paint(painter, registry, key, rect, &IconStyle)`.** It snaps the box to whole physical pixels (rule Z1:
  `P = round(size × ppp)`, origin rounded), rasterises the chosen master at the drawn pixel size and paints it
  1:1, always at the physical size (ZM1's rule for image sources holds for registry icons too). Textures are
  cached per (key, registration generation, master, asset version, hc, px, colours). Stale generations or
  versions of the same key are dropped, and each key keeps at most `RASTERS_PER_KEY` (6) rasters, evicted by
  last draw, so rasters of earlier zoom levels go (coordinator request, ZM1). `cached_rasters(ctx, key)` reports
  the count. `size()` gives the box size. `DEFAULT_SIZE` is 16 (the
  default chrome size).
- **Colours (ICON-STYLE §6).**
  - With no slot set, the icon is rasterised with `currentColor` white and multiplied by `color`.
  - With any slot set (`Slots { subject, accent, badge }`), the icon is rasterised with a usvg style sheet
    (`slot_style_sheet`): `svg{color:<color>}` plus `.subject/.accent/.badge{color:…}`, and only `color`
    declarations. It is painted untinted (white at `color`'s alpha), so the warning yellow is not multiplied by
    the text colour.
  - `Slots::from_css` parses the custom property text and ignores anything that is not a colour. Colours parse
    through usvg itself (`parse_css_color`).
- **§6's open question, answered.** usvg 0.45 resolves `currentColor` at the element that *declares* the paint
  (`parser/style.rs` `convert_paint`: `node.find_attribute(Color)` on the declaring ancestor). A classed group
  whose stroke is inherited from the root would therefore keep the root colour. `slot_svg` rewrites the SVG
  before a slot raster (roxmltree, the copy usvg already links). Each `subject`/`accent`/`badge` element that
  inherits `fill` or `stroke` as `currentColor` gets its own `fill`/`stroke="currentColor"`, so its slot `color`
  applies. This covers the warning triangle's stroke and the subject's 100% outline. The SVGs need no split, so
  IC1's L4 fallback is not needed.

### Integration

- **Own-widget paint primitive** (`paint/icon.rs`):
  - `paint_icon`, `icon_size` and `content_icon` resolve keys of the default family `icons` through the
    context's registry first. Other keys keep the font-glyph path, and a key the registry lacks falls back to it
    too.
  - The registry icon's box is the part's `width`/`height`, else its `font-size` (the button's `::part(icon)` UA
    is 16px).
  - The slots come from the part's inherited custom properties `--icon-subject/-accent/-badge` (`icon_slots`).
  - `content_icon` reads the engine's typed `ComputedValue::Content` (`none`, `icon(key)`). Before this, only a
    keyword or raw value was read, so a stylesheet's `content: icon(…)` never reached the primitive. OW-CSS
    stage 2 landed the same fix, and the merge kept its version.
  - `IconKey::key(name)` and `IconKey::registry_key()` were added.
- **OW1a2's own `icon` widget** (`widgets/image.rs`, the default since OW1a2), re-pointed at merge: a `name` that is
  a registry key (family `icons`, or the own widget's default `""`) is measured with `paint::icon_size_sized` and
  painted with `paint::paint_icon_sized`. Those are `icon_size`/`paint_icon` with the box side given by the
  `size` prop (else 16) instead of the style. CSS `color` tints it and the host's `--icon-*` slots apply. AS1
  `source`/`tint` and the font glyph path are unchanged. `paint::is_registered(ctx, key)` was added.
- **The egui-wrapper `icon` kind** (`leaf.rs`, `layout.rs`, behind `SVELTERS_OWN_WIDGETS=none`): with no `source`, the family `icons` and a registered `name`, it
  draws from the registry.
  - It is measured as a square of `size` (else 16) and tinted with the text colour (CSS `color` through the
    visuals). The slots come from the node's computed style.
  - `frame` makes it an icon button with the button visuals, drawn without an egui widget (so no new
    `disallowed_types` allow). AccessKit sees an image, or a button when it is framed or clickable.
  - An unknown key keeps today's glyph path and its placeholder. Kind docs, `metadata/semantics/egui.json`,
    `types/egui.d.ts` and `docs/egui/reference/widgets/icon.md` were regenerated by their generators.
- `assets.rs`: `rasterize_styled(svg, hint, style_sheet)` (AS1's rasteriser with another injected sheet).

## For the parallel lanes (the API summary sent to the coordinator)

- **OW1a2 (own `icon`/`image`):** paint with `paint::paint_icon(cx, style, &IconKey::key(name), rect)` and
  measure with `paint::icon_size`. Keep AS1's `source`/`tint` for refs. At merge, the own widget's key path calls
  these and nothing else.
- **OW-RUST `imm::icon`:** `icons::paint(painter, &icons::registry(ctx), key, rect, &IconStyle { size, color,
  slots, high_contrast })`.
- **cadatomic:** at startup, `icons::IconRegistry::global().register_set(Owner::Host, &assets, "icons", "")` on
  an Assets root that serves the IC1 build. An extension host passes `Owner::Extension(id)`.

## Rules satisfied

ICON-STYLE:
- **§5.1/§5.2 optical sizes**: one drawing per size, with the master chosen per box.
- **§5.3 Z1** (boxes on whole physical pixels at every zoom; test `paint_snaps_the_box_to_physical_pixels` at
  1.0/1.25/1.5) and **Z2** (master choice, `z2_matches_icon_style_table`, `master_choice_by_physical_size`).
- **§5.3 cache key** `(id, P, colours)`.
- **§5.4** no bold weight. HC masters are used when the icon has them and `IconStyle::high_contrast` is set.
- **§6 colour slots**: only `color` reaches an icon; there are no geometry or opacity overrides and no other
  variables. There is no multiply when a slot is set. The subject's outline takes the slot.
- **§1 "the icon never carries state"**: pressed, selected and hover visuals come from the control (`frame`).

DESIGN:
- **R1/R2/R6**: state belongs to the control, not the glyph. A framed icon uses the control's interact visuals.
- **R9**: disabled at the control's opacity. The tint keeps `color`'s alpha in both colour paths.
- **§4.5**: semantic icon ids (keys) map to a drawing, so an icon theme is a registry swap. Icons take `color`.

WIDGET-PARITY:
- **P2** `content: icon(<key>)` on glyph parts resolves through the registry.
- **§2.1 `icon` (button icon key)** and `icon[frame]`: an icon-only button by key.
- **§2.1 `iconSize`** = CSS `width`/`height` (or `font-size`) on `::part(icon)` for registry icons.

VIS1:
- **§6.4.1 sizes within ±1px**: the icon box is exact (snapped, painted 1:1).
- **VIS1 gap 8** (the select chevron is not the codicon) closes when a lane registers a codicon `chevron-down` and
  the select part uses `content: icon(chevron-down)`. That is not done here. No VIS1 run was made (icons have no
  VIS1 references yet).

## Tests

- `svelters-egui` lib `icons::tests::{keys, namespaces, z2_matches_icon_style_table,
  slot_rewrite_makes_inherited_paints_local, css_colours}`.
- `svelters-egui` `tests/icons.rs` (IC1's built samples):
  - `keys_and_extension_namespaces`: refusals and their diagnostics, unregister by owner;
  - `master_choice_by_physical_size`;
  - `tint_without_slots_and_real_colours_with_them`: white without slots. With accent, every opaque pixel is
    #CCA700, including the inherited stroke;
  - `subject_slot_colours_the_tone_and_its_outline_only`: the arrow keeps `color`, the face is the translucent
    slot colour, the subject's outline is the slot colour;
  - `paint_snaps_the_box_to_physical_pixels`;
  - `rasters_of_earlier_zoom_levels_are_evicted`: 11 zoom levels leave 6 rasters of the key;
  - `content_icon_and_button_icon_part_use_the_registry`: CSS `content: icon(chrome.warning)` and
    `--icon-accent` on a node; `paint_icon` paints a texture, untinted. The own button's `icon` part is drawn
    from the registry at 16px;
  - `icon_kind_draws_registry_keys` (kittest): `<icon name>` is 16/24 square, an AccessKit image, and with
    `frame` an icon button. An unknown name falls back to the font path.

## Not done (later lanes)

- **Built-in sets** (decided, coordinator 2026-10-22): ICR1 ships the mechanism only. No built-in icons are
  registered by default, and unknown keys fall back to the font-glyph path, so today's interim icons keep
  working. **Vendoring codicons as SVG built-ins moves to DS5/IC2.** That lane keeps codicons' own CC-BY notice
  with the vendored files and adds no LICENSE file or licence field for svelters' code. IC0's samples are
  unapproved and stay test data. Embedding works today through `Icon::svg(16, include_bytes!(…))`.
- **Follow-up: the JS registration module `svelters:icons`** (extensions passing `Asset` handles). Registration
  is Rust-only in this lane; lookup from Svelte works by key.
- **High Contrast detection**: the theme's contrast does not reach `IconStyle::high_contrast` from the kind or
  the primitive (there is no context accessor for the theme yet). Embedders pass it in `icons::paint`.
- **`content: icon(a.b+badge)` and `icon(family:name)`**: svelters-style's `dotted_fn` parses only dotted idents,
  so composed ids are reachable from props, not from CSS `content`.

