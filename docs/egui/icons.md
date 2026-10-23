---
title: Icons
description: "The icon registry: keys, extension namespaces, registering icons and sets, optical sizes, tint and colour slots."
status: pending   # ICR1 is on ci/icr1 (102b4ffb), not yet merged into master
verified_at: ci/icr1@102b4ffb
sources:
  - crates/svelters-egui/src/icons.rs        # on ci/icr1
  - crates/svelters-egui/src/paint/icon.rs
  - plans/milestones/ICR1-icon-registry.md   # on ci/icr1
  - plans/research/ICON-STYLE.md
---

# Icons

> **Pending merge.** This page describes the icon registry as built on `ci/icr1` (102b4ffb). Until it
> merges, master has only the font-glyph path: `<icon name family>` over a font family the embedder
> registers with `fonts::register_icons`. Check `docs/egui/status.md` or `STATUS.md` (ICR1 row) at
> your pin.

Icons are drawn from a Rust **`IconRegistry`**: a key maps to SVG drawings at several optical sizes (or to
a font glyph). The registry picks the drawing for the box's size in physical pixels, rasterises it once
per size and colour set, and paints it pixel-exact. No JS runs on the common path.

Use the registry for every app and extension icon. Don't paint SVGs yourself, and don't use `source=`
for icons that have a key: `source`/`tint` are for one-off images.

## Keys

- One or more lowercase, dot-separated, kebab-case segments: `chrome.close`, `ecad.via-stitch`,
  `debug.step-over`.
- An optional badge segment for composed ids: `mcad.extrude+remove` (from props only; CSS `content`
  can't spell it yet).
- **Extensions** register only under `ext.<id>.<name…>`, where `<id>` is one kebab segment. The host
  registers any key outside `ext.`.

`icons::validate_key(key)` checks a key. A refused registration returns an `IconError` and is also queued
as a diagnostic (`icons::take_diagnostics()`).

## Using an icon

From Svelte, by key (the `icon` kind's `family` defaults to `icons`):

```svelte
<icon name="chrome.close" />
<icon name="chrome.close" frame tooltip="Close" onclicked={close} />   <!-- an icon button -->
<icon name="mcad.extrude" size={24} />
```

- `size` is the box in points (16 when unset). The icon is tinted with CSS `color`.
- `frame` draws it as an icon button (role `Button`, takes focus); `selected` gives the toggle look.
- An unregistered key falls back to the font-glyph path (its placeholder box if no font has it).

On own widgets' glyph parts, from CSS:

```css
button.save::part(icon)            { content: icon(file.save); }
tree-view::part(twistie)           { content: icon(chrome.chevron-right); }
.warn::part(icon)                  { --icon-accent: #cca700; }
```

The registry icon's box is the part's `width`/`height`, else its `font-size` (`button::part(icon)` is
16px by default). The button's `icon` prop takes a key too.

## Registering icons (Rust)

```rust
// not run
use svelters_egui::icons::{Icon, IconRegistry, Owner};

let reg = IconRegistry::global();
// one SVG per optical size; High Contrast copies are optional
reg.register(Owner::Host, "chrome.close",
    Icon::svg(16, include_bytes!("icons/16/close.svg").as_slice())
        .master(12, include_bytes!("icons/12/close.svg").as_slice())
        .master(24, include_bytes!("icons/24/close.svg").as_slice())
        .hc_master(16, include_bytes!("icons/hc/16/close.svg").as_slice()))?;

// a font glyph
reg.register(Owner::Host, "chrome.gear", Icon::glyph("codicons", "\u{eb51}"))?;

// a whole set built by IC1's `icons.py build` (manifest.json, <size>/<id>.svg, hc/<size>/<id>.svg)
let n = reg.register_set(Owner::Host, &assets, "icons", "")?;

// an extension's icons, confined to ext.<id>.*
reg.register(Owner::Extension("gerber-tools".into()), "ext.gerber-tools.panelize", icon)?;
reg.unregister_owner(&Owner::Extension("gerber-tools".into()));   // on unload
```

- `IconData` is `Static(&'static [u8])`, `Shared(Arc<[u8]>)` or `Asset(Arc<Assets>, AssetRef)`. An
  asset-backed icon is re-read when the asset's version changes (hot reload); svelters-egui itself does no
  file I/O.
- `unregister(owner, key)` removes only that owner's key.
- `IconRegistry::global()` is the process registry. `icons::set_registry(ctx, reg)` gives one egui
  context its own (tests, an embed kept apart); `icons::registry(ctx)` reads the one in effect.
- **No built-in icons are registered.** Vendored codicons as SVG built-ins are a later lane (DS5/IC2).
- **Registration is Rust-only.** The JS module `svelters:icons` (extensions passing `Asset` handles) is
  a follow-up. Lookup by key works from Svelte.

## Optical sizes

Masters exist for 12, 14, 16, 20, 24 and 32+ px (ICON-STYLE). For a box of `size` points at
`pixels_per_point`, the drawn size is `P = round(size × ppp)`, and the box is snapped to whole physical
pixels. The master is chosen as follows (`icons::choose_master`):

1. a master of exactly `P`;
2. else a master `m` with `P = 2m` (drawn at 2×);
3. else the largest master (or 2× master) at most 2 px smaller, drawn at its own size and centred;
4. else the nearest master, scaled.

`reg.resolve(key, box_pt, ppp) -> Option<Choice { master, px, offset, fit }>` shows the choice. The icon
is rasterised at the drawn pixel size and painted 1:1.

## Colour: tint and slots

- **No slot set**: the SVG is rendered with `currentColor` white and multiplied by CSS `color`. Disabled
  opacity comes through `color`'s alpha.
- **Slots**: `--icon-subject`, `--icon-accent` and `--icon-badge` (inherited custom properties) colour the
  SVG's elements of class `subject`, `accent` and `badge`. Only `color` is set: no geometry, opacity or
  stroke overrides, and no other variables. With any slot set, the icon is painted untinted, so a warning
  yellow stays yellow. Elements that inherit `currentColor` paints are made local first, so a classed
  group's inherited stroke takes its slot.
- Each distinct slot set is one cached raster per key and size.
- **State belongs to the control, not the glyph.** Pressed, selected and hover visuals come from the
  framed icon or the host widget.

## From Rust UI (no scene)

```rust
// not run
use svelters_egui::icons::{self, IconStyle};
let style = IconStyle::new(16.0, ui.visuals().text_color());   // .slots, .high_contrast
icons::paint(ui.painter(), &icons::registry(ui.ctx()), "chrome.close", rect, &style);
```

`icons::size(ctx, &reg, key, size_pt)` gives the box. High Contrast is not detected from the theme yet:
pass `IconStyle::high_contrast` yourself.

## Not done yet

- `svelters:icons` (JS registration), built-in sets, HC detection from the theme.
- `content: icon(a.b+badge)` and `icon(family:name)` in CSS.
- The texture cache has no bound across distinct colour sets.
