# skelter: vector icons and live icon colours

How skelter draws SVG icons and SVG images, and how their colours change without a UI phase.
Milestone record: `plans/milestones/SKELTER-svg.md`.

## Vector or raster

A registry icon (`IconRegistry`, `<icon name="file.save">`) keeps ICON-STYLE's rules: its box snaps
to whole device pixels (Z1), and `choose_master` picks the optical-size master for the physical size
P (Z2). Then:

- **P ≤ 24 px** (`paint::icon::vector::VECTOR_THRESHOLD_PX`): resvg rasterises the master, pixel
  snapped. The threshold comes from side-by-side screenshots at 1×, 1.25×, 1.5× and 2×. Up to
  24 px, resvg's analytic coverage of a hinted master is crisper than 4× MSAA. From 32 px the two
  look the same.
- **P > 24 px**: the master draws as vectors, one `Prim::Path` per layer through the renderer's
  lyon/MSAA path pipeline. Zooming and scale-factor changes redraw from the renderer's mesh cache
  (one mesh per path per 9% scale step). Nothing is rasterised, and nothing is uploaded to the atlas.

`<image src="….svg">` and the `icon` kind's `source` follow the same rule. Above the threshold they
draw as vectors, clipped to the box and its corners, in the SVG's own colours (or tinted, for an
icon with `tint`). `image-rendering: pixelated` keeps the raster.

## Monochrome or multicolour: `IconColors`

Whether an icon keeps its literal colours is chosen per icon when it is registered, never inferred:

- `Icon::svg(..)` is `IconColors::Tint` (the default). The icon is monochrome: its shapes are
  coverage tinted by `color`. Literal colours keep only their alpha, unless slots are set.
- `Icon::multicolour(..)`, or `.colors(IconColors::Own)`, keeps its literal colours at every size:
  - at 24 px or below, one RGBA raster of the literal colours plus one live mask per slot it uses;
  - above 24 px, its vectors in their literal colours.

  Its `currentColor` and slot layers stay live palette slots either way.

## `VectorArt`

`paint::icon::vector::VectorArt::parse(svg)` parses an SVG once per content (cached). It returns
the size and the layers in paint order. Each `VectorLayer` has:

- `path`: the outline in the layer's coordinates. Strokes are outlined on the CPU by tiny-skia's
  stroker, with caps, joins, miter limits and dashes, so every layer is a fill.
- `transform`: from the layer's coordinates into the drawing.
- `kind`: `Fill` or `Stroke`.
- `opacity`.
- `source`: a literal `Color`, `CurrentColor`, or `Slot(IconSlot::{Subject, Accent, Badge})`.

`VectorArt::record` uploads each layer's path through `Resources::path`, keyed by the SVG's content.
skelter-core's `PrepareCx::share_path` shares the path across the document, so a toolbar of 200
copies of an icon draws two `PathId`s and two meshes. It then pushes the `Prim::Path`s. The types
are `#[non_exhaustive]`.

### Fallback, per icon

`VectorArt::parse` returns `Err(NotVector::Unsupported(why))` for what the vector path cannot draw
yet:

- filters, masks, clip paths and blend modes;
- patterns and gradients (linear and radial);
- embedded images and text;
- a group opacity over several shapes.

Such an icon keeps its raster at every size. The registry reports `IconError::Fallback { key, why }`
once per icon and reason (`registry::take_diagnostics`).

## Live colours: palette slots

`skelter_base::display::Palette` has `PALETTE_SLOTS` (4) colours:

| Slot | Colour |
| ---- | ------ |
| 0 | `currentColor` |
| 1 | `--icon-subject` |
| 2 | `--icon-accent` |
| 3 | `--icon-badge` |

`Prim::Icon` and `PathPrim` carry an optional `PaletteSlot { index, alpha }`. The primitive's
recorded colour is the slot's colour times `alpha` (the layer's own opacity). While the palette of
the item's effect chain sets that slot, the compositor draws the palette's colour instead
(`Palette::apply`, `PropTrees::palette`).

- **Vector layers** reference their slot.
- **Raster icons without slots** are one `A8` mask tinted from slot 0.
- **Raster icons with slots** (when the icon vectorises) are one `A8` mask per slot, made by
  `vector::slot_masks`, each tinted from its slot. A later layer knocks out what it covers of the
  earlier masks. The masks don't depend on the colours, so a theme switch rasterises and uploads
  nothing. Literal colours go into one RGBA raster drawn under the masks.
- **A fallback icon with slots** keeps the old baked RGBA raster: its slots are not live.

### Colour transitions on the compositor

`AnimProperty::Palette(slot)` (with `AnimValue::Color`) is a compositor property on the node's effect
node, like opacity. If a node's recorded list draws only slotted primitives (an icon), skelter-core
sends its CSS `transition: color` (hover, press, disabled, a theme switch) to the compositor as
`Palette(0)`. The list is recorded once at the end colour. The compositor eases the palette with
skelter-anim's keyframe sampling and clears the override when the transition ends. There is no UI
phase, no re-raster and no atlas upload during the transition. A node that draws anything else,
text for example, keeps its UI-thread colour transition.
