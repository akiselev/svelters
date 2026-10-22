# WG2b: the rest of Tier 0 on egui (controls, containers, media, canvas) + W5

Design: `plans/research/EGUI.md` §5 (the element catalog, approved as the built-in widget set on 2026-10-22, §8)
and §8's W1/W5 answers. Parallel lanes: WG1 (SC1 subset, merged into this branch from `origin/ci/wg1` 50e680f7),
WG2a (menus, popups, modal, inner-window), WG4a, WG4b. This file records WG2b as built.

## Scope

Thirteen kinds, and the Linux primary selection in the text kinds:

- inputs: `hyperlink`, `radio-button` (with a group binding), `switch` (synthetic), `range-slider`, `drag-value`;
- containers: `panel`, `sides`, `frame`, `stack` (synthetic);
- display: `image`, `progress-bar`, `spinner`, `canvas` (the X8 display list replayed with egui's painter);
- W5: select-to-copy and middle-click paste in `text-edit` (Linux only).

## The elements

Every kind also has the `Widget` props (`enabled`, `visible`, `tooltip`, `accessible-name`, `anchor`, `invalid`,
the size and flex item props). `panel` and `frame` have the `Box` props too (`flex-direction`, `gap`, `padding`, …).
"bind" = the prop is written by the widget (`Native::Widget`): `bind:` lowers onto exact observation, and the
`changed` event carries it.

| Tag | Native | Props (own) | Events | Bindings |
|---|---|---|---|---|
| `hyperlink` | `egui::Hyperlink` (`egui::Link` without url) | `text` (text content; the url when empty), `url`, `new-tab` | `clicked`, `secondary-clicked` | — |
| `radio-button` | `egui::RadioButton` | `text` (text content), `checked`, `value`, `group` | `changed(checked, group)`, `clicked` | `checked`, `group` |
| `switch` | synthetic toggle (role `Switch`) | `active`, `text` (text content) | `changed(active)`, `clicked` | `active` |
| `range-slider` | `egui::RangeSlider` | `low`, `high`, `min`, `max`, `step`, `text`, `show-value` | `changed(low, high)`, `drag-started`, `drag-stopped` | `low`, `high` |
| `drag-value` | `egui::DragValue` | `value`, `min`/`max` (nullable: unbounded), `speed`, `decimals` (nullable), `prefix`, `suffix` | `changed(value)`, `drag-started`, `drag-stopped` | `value` |
| `panel` | panel fill + separator + grip, flex box | `side` (left/right/top/bottom), `size` (nullable), `resizable`, `separator` | `changed(size)` | `size` |
| `sides` | `egui::Sides` as a flex row (space-between) | `gap`, `padding`, `align-items` | — | — |
| `frame` | `egui::Frame` + flex box | `label`, `frame-style` (group/canvas/window/menu/popup/panel/none) | — | — |
| `stack` | synthetic, one child shown | `visible-child-name` (a child's `anchor`), `visible-index`, `homogeneous`, `padding` | — | — |
| `image` | `egui::Image` texture | `source` (path, `file://`, `bytes://`, any egui loader uri), `fit` (contain/cover/fill/none/scale-down) | `clicked` (senses clicks only while listened) | — |
| `progress-bar` | `egui::ProgressBar` | `fraction`, `text`, `show-percentage`, `animate` | — | — |
| `spinner` | `egui::Spinner` | `spinning`, `size` (nullable) | — | — |
| `canvas` | `egui::Painter` replay of X8 `ops` | `ops` (nullable JSON), `content-width`, `content-height` | `resize(width, height)`, `clicked` | — |

Cross-backend vocabulary: `radio-button`'s `checked` + `toggled`-style exclusivity is Qt's (auto-exclusive siblings)
and WinUI's; `group` + `value` gives Svelte's own `bind:group={choice}` idiom (it compiles: `group` is an ordinary
bound prop in the egui spec). `switch`/`active`, `stack`/`visible-child-name`, `frame`/`label`, `progress-bar`/
`fraction`, `spinner`/`spinning`, `canvas`/`content-width`/`content-height`/`ops`/`resize` are GTK's names (so the
same `@svelters/canvas` `draw()` attachment works unchanged: egui posts `resize(width, height)` like GTK).
`hyperlink`, `drag-value`, `range-slider`, `sides`, `panel` are kebab-cased egui types (N1).

## Files (as built)

| File | Content |
|---|---|
| `crates/svelters-egui/src/kinds/{inputs,panes,media}.rs` | the kind tables (one `const KindDef` per kind, appended to `KINDS` after WG1's) |
| `crates/svelters-egui/src/elements/mod.rs` | payload defaults, prop get/set, measure dispatch, the leaf paint wrapper (a11y name, tooltip, `invalid`, focus, click/drag events), `changed` |
| `elements/inputs.rs` | paint/measure of the five inputs; the radio sibling logic; the switch widget |
| `elements/media.rs` | paint/measure of image, progress bar, spinner, canvas; `fit_rect` |
| `elements/panes.rs` | container styles (panel size, sides row, frame padding), the stack layout (taffy, generic over the tree), container chrome, the panel grip |
| `elements/canvas.rs` | the display-list replay (`shapes`), CSS colours, ear clipping |
| `elements/images.rs` | an egui `ImageLoader` for `file://` and bytes-loader uris, decoding PNG with the `image` crate |
| `crates/svelters-egui/src/primary.rs` | W5: the `PrimarySelection` provider, `use_system` (arboard, Linux), `text_edit` |
| shared files | one dispatch each: `scene/node.rs` (W variants, `is_container`, `text_of`), `scene/tree.rs` (payload), `scene/props.rs` (set/get, converters made `pub(crate)`), `layout.rs` (style hook, measure arm, stack layout arm), `leaf.rs` (leaf and container dispatch, `text-edit` through `TextEdit::show` + W5), `walk.rs` (`paint_at` `pub(crate)`), `spec.rs` (normalizing bindings), `runner/eframe.rs` (`primary::use_system`) |

## Behaviour and layout notes

- **Layout boundaries (SG1).** A write of `panel.side`/`size`, `frame.label`/`frame-style` or `stack.visible-*`/
  `homogeneous` restyles the node and relays out from its parent (`Dirty::LAYOUT` from `elements::set_prop`). They
  are not CSS setters (`layout: false`); only the `Box` props of `sides`/`stack`/`frame`/`panel` are, which
  `tools/metadata/style/gen_props.py` put into `packages/style/src/props.generated.js` and
  `crates/svelters-style/src/tables.generated.rs` (regenerated). A user resize of a panel does the same from the
  walk (`restyle` + `invalidate_style`, repaint). Leaf content changes (`text`, `url`, `source`, `decimals`, …)
  remeasure. An image measured before an async loader finished is remeasured when it loads.
- **`stack`** is laid out by `elements::panes::stack_layout`: sized like a leaf whose content is the visible child
  (homogeneous: the largest child), the visible child fills the content box, the others get the hidden layout and
  are neither drawn nor hit.
- **`frame`** pads its content by the egui frame's margins (inner + stroke + outer) and a caption row, taken from
  egui's *default* style, so layout does not depend on the live theme; paint draws the live theme's frame in the same
  rect.
- **`sides`** is a flex row with `space-between`: its first child goes to the start, the second to the end. More
  than two children share the space (egui's `Sides` takes two closures; wrap groups in a `box`).
- **`radio-button`**: a click checks it, writes its `value` into `group` (when it has one), and unchecks its sibling
  radio buttons (same parent), setting their `group` too; each changed sibling announces and fires `changed`. With a
  `value`, a host write of `group`, `value` or `checked` re-derives `checked = (group == value)`; that derived change
  is not announced (a host write never is).
- **Canvas replay**: the GTK op set; curves flattened; `fill` fills each subpath as a simple polygon (convex ones
  anti-aliased, others ear-clipped; no holes or fill rule); `clip` cuts to the path's bounding box; `clearRect`
  paints the panel background; `lineCap`/`lineJoin` are not drawn; a dash uses the first two entries; text is
  translated and scaled but not rotated, `strokeText` fills. Unknown ops are skipped; a list that is not a JSON array
  is refused on write.
- **Images**: PNG only built in (`image` with `png`, the version arboard already locks). Other formats and `http(s)`
  come from loaders the embedder installs (egui_extras); egui asks every loader in turn.
- **Spinner and `progress-bar animate`** repaint every frame while shown (egui's own animation); a settled app has
  none.
- **Bad input never panics** (WG2b-2). An `image` whose source does not load (missing file, unknown scheme, not a
  PNG, decode error) is measured at 16×16 and drawn as a crossed-out frame; a loaded one replaces it and remeasures.
  A PNG with no pixels is a load error; one with a side over the renderer's `max_texture_side` is scaled down to fit
  (egui asserts the limit, a GPU backend refuses it). Numeric props keep what the host wrote, but NaN and
  infinities never reach egui (whose `f64::clamp`s panic on a NaN bound): a `range-slider` rail end falls back to
  0/1 and stays within ±1e300, a handle falls back to its rail end, a reversed rail is sorted; a `drag-value` NaN
  bound is no bound, a NaN value is shown as the lower bound or 0, a non-finite speed is 1; a `progress-bar` NaN
  fraction is empty, +inf full. Layout lengths (`panel.size`, `spinner.size`, `canvas` `content-width`/
  `-height`) drop NaN and infinities on write (null, or 0 for the canvas) and are at most 1e7. The canvas replay
  keeps every screen coordinate within ±1e6, font sizes within 512 and stroke widths within 1e4 (a transform can
  overflow finite arguments), ignores `NaNpx`/`infpx` fonts, and strokes a dash pattern with a period under a
  point solid. Test: `tier0.rs` `bad_input_falls_back_and_never_panics`.

## W5: Linux primary selection

- `svelters_egui::primary`: a `PrimarySelection` trait (`set`, `get`), one process-wide provider (`install`,
  `uninstall`, `provider`), `use_system()` (arboard's `LinuxClipboardKind::Primary`, X11 or Wayland
  data-control), `SUPPORTED = cfg!(target_os = "linux")`. Elsewhere everything is a no-op.
- The eframe runner calls `use_system()`. An embedder that owns its window (cadatomic) calls it, or installs its own
  provider. Without a provider, the text kinds behave as plain egui.
- `text_edit(ui, &TextEditOutput, id, &mut text, password, char_limit)`: a new non-empty selection is copied (never
  from a password field); a middle click inserts the primary selection at the pointer (bounded by `char-limit`),
  moves the cursor after it, focuses the field, and the caller announces the edit (`changed`, `bind:text`).
  `text-edit` calls it; a provider kind built on `egui::TextEdit` can too (it is public).
- W1: nothing else in `TextEdit` changes, and no platform shortcut is hard-coded in the new code.

## Gallery

`fixtures/gallery-egui` (new; the other widget lanes add their pages to it): an app with a sidebar of page buttons,
and WG2b's pages `20-inputs`, `21-containers`, `22-media`. `crates/svelters-egui/tests/gallery.rs` loads the bundle
on QuickJS under kittest, opens each page and drives it: `bind:group`, `bind:checked`, `bind:active`,
`bind:low`/`bind:high`, `bind:value` (clamped), a link click reaching JS, the stack switched from JS, the panel at its
bound size, the image at its size, the canvas `draw()` attachment getting `240x120` from `resize`. No JS errors.
`pnpm test` of the fixture is its `vite build` (the compiler's check against `egui.json`). There are no screenshot
tests: kittest runs without a renderer here.

## Tests

- `crates/svelters-egui/tests/tier0.rs` (one kittest per kind, through the vtable): hyperlink (role `Link`, the
  `OpenUrl` command, `clicked`); radio buttons (role `RadioButton`, toggled state, group and checked modes, sibling
  announcements, unannounced host re-derivation); switch (role `Switch`); range slider (AccessKit `SetValue` on the
  `Range low`/`Range high` handles, handles never cross); drag value (role `SpinButton`, clamping, null bounds); image
  (`bytes://` and a bare file path measured at the image size, click, new source remeasured) and `fit_rect`;
  progress bar and spinner (role `ProgressIndicator`, busy); canvas (resize posted once, click, the replay's shapes,
  a concave fill ear-clipped, transforms, arcs, text alignment); panel (grip drag, `min-width` bound, relayout);
  sides; frame (padding by margins and caption, `none`); stack (by name, by index, homogeneous sizing); W5 (Linux:
  select-to-copy and middle-click paste through a fake provider).
- `src/elements/canvas.rs` unit tests: CSS colours, ear clipping area, transform order.
- `tests/observe.rs`: every new prop is observable and host writes never echo (an `ops` value that parses).
- `tests/spec.rs`: `metadata/semantics/egui.json` and `packages/native-runtime/types/egui.d.ts` regenerated
  (`SVELTERS_UPDATE=1`), which also brings WG1's kinds into them.

## Decisions and deviations

- **Kinds in family files, one dispatch line per shared file.** Payloads are `W` variants (WG1's pattern), but their
  get/set, measure, paint and layout live in `crate::elements`; the shared files call it before their own matches.
- **`frame` margins from the default style** (see above): a theme with different frame margins draws its frame in
  the laid-out rect, but the content keeps the default inset.
- **`panel` is a flex item, not an egui `Panel`.** egui's panels take a side of a `Ui`; in the retained scene the
  panel is a child placed at the edge of a row or column, with the panel look and a grip. Its `size` is the main-axis
  size and `flex-shrink: 0` while set.
- **`stack` names children by `anchor`.** The egui backend has no child attributes (GTK uses `parent:name`); the
  anchor is unique in the root already.
- **`spinner.size` and `panel.size` reuse WG1's `P::Size`** (same prop name, nullable f64).
- **`image` decodes PNG only.** JPEG etc. would pull new crates; the loader seam takes egui_extras' loaders.
- **No `clearRect` transparency.** egui cannot erase what was painted below; it paints the panel background.

## Notes for the other lanes

- WG2a/WG4a/WG4b: append kinds after WG2b's in `KINDS`; `elements::{is_leaf, is_container}` show the dispatch
  pattern if you want the same split. Add pages to `fixtures/gallery-egui/src/pages/` with numbers that sort apart
  (WG2b uses 20–22) and tests to `tests/gallery.rs`; `open(title)` opens a page by its sidebar button.
- A provider kind with a text field gets W5 by calling `svelters_egui::primary::text_edit` with its
  `TextEditOutput`.
- `canvas` takes no pointer events beyond `clicked` yet (cadatomic's viewports are Rust-drawn); pointer
  press/move/release with coordinates is the next step if a Svelte canvas needs them.

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets`: clean (one pre-existing svelters-host warning).
- Before merging WG1's latest (0a7c8c30..66fcd9df): `cargo nextest run -p svelters-egui` 77/77 with the egui
  fixtures built.
- After the last merge and code change: `cargo nextest run -p svelters-egui --test tier0 --test gallery --test
  observe --test spec` 24/24 (gallery bundle rebuilt with a rebuilt `svelters-compile`, which embeds `egui.json`);
  `pnpm --filter @svelters/style test` 25/25; `gen_props.py --check` clean.
- Not run locally: the rest of the svelters-egui suite after the last merge, `svelters-style`, `egui-kinds-example`,
  the workspace nextest, conformance, the other backends' suites, the JS test run. CI runs them.
