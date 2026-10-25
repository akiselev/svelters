# tessel-text: the text stack for cadatomic's own toolkit (research notes)

Compiled 2026-10-25 for `docs/research/2026-10-25-own-gui-toolkit.md` §6.5. These notes extend
`notes/2026-10-25-own-gui-toolkit/gpu-2d-text.md` (§2) rather than repeat it. This was research
only: no build, benchmark or test was run. Sources were read on 2026-10-25.

**Grades** (one per claim):
- **[D] documented:** the cited doc, README, changelog or post says it.
- **[S] source-read:** I read it in the cited repository at the named commit.
- **[M] measured-by-others:** a number someone else reported on their own machine.
- **[I] inferred:** my reasoning from the cited material.

**Repositories read** (shallow clones, deleted afterwards):

| Repo | Commit, date |
|---|---|
| linebender/parley | `2133d78` (main, 2026-10-22); tag `v0.12.0` |
| pop-os/cosmic-text | `f1a3461` (2026-09-30) |
| harfbuzz/harfrust | `1503ff0` (2026-10-22) |
| googlefonts/fontations | `cf5676b` (2026-10-23) |
| dfrg/swash | `7773843` (2026-07-17) |
| grovesNL/glyphon | `49dc8f7` (2026-07-09) |
| emilk/egui | `bf5ac0d` (2026-10-21), `crates/epaint` only |
| zed-industries/zed | `a5c4ca8` (2026-10-25), crates `gpui`, `gpui_wgpu`, `gpui_windows`, `gpui_linux` and `gpui_apple` |
| microsoft/terminal | `98c8c79` (2026-10-23), `src/renderer/atlas` |
| GNOME/gtk | `c36202f` (2026-10-25), `gtk/gtkwidget.c` and `gsk/gpu/shaders` |
| ghostty-org/ghostty | `cec8b92` (2026-10-24), `src/renderer/shaders` |
| microsoft/vscode | `53e8ffb` (2026-10-25), `src/vs/editor/browser/gpu` |

The local source is quire at `/home/user/akiselev/quire` (STATUS handoff 2026-10-22).

---

## 0. Decision summary

| Area | Proposal (§6.5) | Verdict after research |
|---|---|---|
| Layout | parley 0.12 + fontique 0.12 + harfrust | **Keep for all UI text** (trees, grids, labels, dialogs, rich text, hover docs). parley gives min/max-content, re-breaking without reshaping, bidi, inline boxes and grapheme-correct editing. It lacks ellipsis and tab stops, and it has no cache of shaped output: tessel adds all three. |
| Editor shaping | quire per-line cache | **Keep quire's design** and move its shaper from egui `LayoutJob` to **`parley_engine` 0.12** (analysis, bidi, fallback, harfrust) per logical line, with the ASCII monospace fast path. Do not use `parley::PlainEditor` or cosmic-text's `Editor` for the code editor. |
| Outlines and hinting | skrifa, vertical-only | **skrifa 0.49** with `HintOptions::auto_fallback().with_preserve_linear_metrics(true)` (vertical-only) by default. It honours `ScalerFont::raster_policy()` (DirectWrite's gasp rules) and adds horizontal-only antialiasing at ≤20 ppem for instructed fonts. This needs a skrifa newer than parley's 0.44 (a version-skew risk, §2.6). |
| Rasteriser | unspecified | **Our own small area-coverage rasteriser** fed by skrifa's `PathSink` (font-rs/zeno style), plus an asymmetric (x-only AA) mode. COLRv1 goes through skrifa's `PaintSink` into a small painter, or into `vello_cpu` 0.3 if we take that dependency anyway. zeno 0.3.3 and vello_cpu are the fallbacks. |
| Atlas | R8, 4×1 subpixel, etagere | **Keep.** Add an RGBA premultiplied atlas for colour glyphs. Key entries by AA mode and dilation level too. |
| Gamma | DirectWrite gamma 1.8 / contrast 1.0 | **Keep the formula** (§3), but only with **non-sRGB (`*Unorm`) surfaces and blending in sRGB-encoded space**, as Windows Terminal, Zed, browsers and egui do. With an `*Srgb` surface the DirectWrite constants are wrong. |
| LCD | off | **Off for M1, but leave room.** Zed added LCD text, on by default on Windows and Linux, in Jan 2026, after shipping grayscale+gamma in Sep 2025. Plan a `SubpixelSprite`-style path (dual-source blend, opaque layers only) as a later, gated option. |
| Canvas text | Slug | **Slug**, as our own WGSL port of the MIT reference shaders fed by skrifa outlines. Stroke fonts (KiCad-style) go through the existing line primitives. Below about 6–8 px on screen, switch to atlas glyphs or greeking. MSDF (fdsm 0.8) is not needed. |
| Determinism | skrifa everywhere | **Keep for tests:** bundled fonts and `system_fonts: false`. Compare CPU-side goldens (glyph-run lists, atlas bytes) exactly and GPU pixels with a tolerance. Native look on macOS needs dilation emulation (§2.5). |

---

## 1. Layout: parley 0.12 vs cosmic-text 0.19 (and what egui and Zed do)

### 1.1 Versions and the ecosystem today

- **parley:** 0.12.0, released 2026-10-09, with fontique 0.12.0 and the new `parley_engine` 0.12.0. MSRV 1.88. [D] crates.io API; parley `CHANGELOG.md` at `2133d78`.
  - It depends on `harfrust 0.12`, `skrifa 0.44`, `read-fonts 0.41`, ICU4X `icu_segmenter ~2.3`, `icu_properties 2.3` (`compiled_data`) and `swash 0.2.10` (examples only). [S] `parley/Cargo.toml` at `v0.12.0`.
  - Main still pins the same fontations versions. [S]
- **Current fontations:** harfrust 0.15.0 (2026-10-22, matches HarfBuzz 15.0.0, read-fonts 0.46). skrifa 0.49.0 and read-fonts 0.46.0 (2026-10-21). [D] crates.io; harfrust `CHANGELOG.md`.
  - **So parley 0.12 is three harfrust and five skrifa releases behind.** [I]
- **egui 0.37.0** (2026-10-15) pins `harfrust 0.12` and `skrifa 0.44` for the same reason. Its workspace comments: "Can't update to 0.13+: 0.14 needs read-fonts 0.45, but fontique 0.12 pins read-fonts 0.41"; "`glifo` 0.4 (via vello_cpu 0.3) pins skrifa 0.44". [S] egui `Cargo.toml` at `bf5ac0d`.
- **cosmic-text:** 0.19.0 (2026-04-22), with `harfrust 0.5`, `skrifa 0.40`, `fontdb 0.24`, `unicode-bidi` and `unicode-linebreak`, plus optional swash rasterisation. [S] cosmic-text `Cargo.toml` at `f1a3461`; [D] crates.io.
- **Who uses which** [D] crates.io dependency lists:
  - Bevy 0.20 (`bevy_text` 0.20.0, 2026-10-08): parley 0.11 + swash.
  - Slint 1.18.1 (`i-slint-core`): parley 0.11, fontique 0.11, skrifa 0.44.
  - Blitz (`blitz-dom` 0.3.0-beta.4): parley 0.12 + taffy 0.15.
  - iced (`iced_graphics` 0.14): cosmic-text 0.15.
  - Linebender's 2026-Q1 report lists Bevy, CuTTY (an Alacritty fork), Gosub, drafft-ink and Takumi as adopters. [D] https://linebender.org/blog/tmil-25/ (Raph Levien, 2026-04-19)
- **Zed** uses the platform shapers on Windows (DirectWrite) and macOS (CoreText), and cosmic-text for per-line shaping on Linux, with no cosmic wrapping (`cosmic_text::Wrap::None`, `Hinting::Disabled`). It does its own wrapping. [S] `crates/gpui_wgpu/src/cosmic_text_system.rs` at `a5c4ca8`.
- **egui/epaint 0.37** does its own layout: harfrust shaping, `unicode-segmentation`, no bidi and no ICU line breaking. It uses skrifa for outlines and hinting, vello_cpu to rasterise, and fontique for system fonts. [S] `crates/epaint/Cargo.toml`, `text/font_face.rs` at `bf5ac0d`.

### 1.2 parley API on the axes asked

**Building**
- `LayoutContext::ranged_builder(&mut fcx, text, scale, quantize)` takes overlapping ranged styles. `tree_builder` takes a span tree with CSS whitespace collapsing. `StyleRunBuilder` takes non-overlapping indexed runs (0.8.0+). [S] `parley/src/context.rs`; [D] CHANGELOG 0.8.0, 0.12.0.
- `quantize: true` aligns layout coordinates to pixel boundaries. With `false`, the caller must round glyph-run baselines, inline-box baselines and selection `y0`/`y1` before painting. [S] `context.rs` doc comment.
- `LayoutContext` is reusable scratch: the analyzer, analysis, style tables, builders and the shaper. [S] `context.rs`.
- The shaper keeps small LRU caches of harfrust `ShaperData`, `ShaperInstance` and `ShapePlan`, linear-scan, "in the order of tens" of entries. **There is no cache of shaped output.** A new `Layout` reshapes everything. [S] `parley_engine/src/shape/shaper.rs`, `lru_cache.rs`.
- Main also reuses allocations when rebuilding (#906) and lets `PlainEditor` reuse its `Layout` allocations (#895). [S] git log at `2133d78`.

**Measuring for taffy**
- `Layout::calculate_content_widths() -> ContentWidths { min, max }`, where min means every soft break is taken and max means none is.
  - 0.12 fixed it for inline boxes after hard breaks, text indent, hanging whitespace, and max ≥ min. [S] `layout/mod.rs`; [D] CHANGELOG 0.12.0 "Fixed".
- Height-for-width: `break_all_lines(Some(w))` then `height()`. "`Layout` supports re-linebreaking and re-aligning many times" without rebuilding. Text or style changes need a new `Layout`. [D] https://docs.rs/parley/0.12.0/parley/
- So a taffy measure function costs one shape per (text, style), then one cheap re-break per width. [I]
- Blitz is the reference integration of parley 0.12 with taffy 0.15. [D] crates.io deps.

**Line breaking**
- UAX #14 via ICU4X (since 0.8.0, replacing swash's analysis). [D] CHANGELOG 0.8.0
- Other options: `CHROMIUM_LINE_BREAK_OVERRIDE` or a custom `AsciiLineBreakTable` (0.11); CSS `word-break`, `overflow-wrap`, `text-wrap-mode`, `line-break` (CJK, 0.12) and `white-space-collapse`. [D] CHANGELOG 0.11.0, 0.12.0
- The `complex-scripts` feature adds dictionary breaking for CJK, Thai, Khmer, Lao and Myanmar (0.10). Dictionaries can be loaded dynamically (#881, main). [D] CHANGELOG 0.10.0; [S] git log.
- Advanced: the `BreakLines` API sets per-line boxes (floats, exclusions) and supports `InlineBoxKind::{InFlow, OutOfFlow, CustomOutOfFlow}` (0.9). [D] CHANGELOG 0.9.0

**Bidi:** paragraph base direction can be set (0.12), and clusters can be iterated visually (`Run::visual_clusters`). Reshaping when a break falls inside a ligature or a cursive word is "in progress" in `parley_engine`. [D] CHANGELOG 0.12.0; `parley_engine` README.

**Rich spans:** `StyleProperty` covers font stack, size, weight, width, style, features, variations, line height, letter and word spacing, decorations, `VerticalAlign` (0.12) and locale. A brush type parameter `B` carries colours. [D] CHANGELOG; [S] `style/`.

**Inline boxes:** `InlineBox { id, kind, index, width, height, … }`. 0.12 added box baselines and `vertical_align`. On main (#902, after 0.12) an in-flow box must sit on a U+FFFC character. [S] `inline_box.rs` at `v0.12.0`; commit `26052b9` on main. **This fits icons and chips inside labels and tree rows.** [I]

**Ellipsis and truncation: not supported.**
- Issue #304 has been open since 2025-03-19.
- The maintainers' workaround is to lay out normally and decide at render time whether the line fits. nicoburns (2025-10-26) argues an ellipsis "should not require reshaping".
- [D] https://github.com/linebender/parley/issues/304 and the egui tracking issue https://github.com/linebender/parley/issues/386 (which lists "Text truncation with ellipsis")
- **tessel must implement it:** shape "…" once per style, walk clusters from the end until `width - ellipsis_advance` fits, and paint the clipped run plus the ellipsis glyph. Middle-ellipsis for paths walks from both ends. [I]

**Tabs: no tab stops.** Issue #302 is open ("tab character currently has no width"; still relevant, Jul 2026). [D] https://github.com/linebender/parley/issues/302. Property grids rarely need tabs; the editor does its own (quire `layout.rs` `TabStops`). [I]

**Editing:**
- `PlainEditor` and `PlainEditorDriver` keep a `String` buffer with a dirty flag and **relayout the whole buffer** on change (`update_layout`). [S] `parley/src/editing/editor.rs`
- Cursor and selection are grapheme-correct (0.12: clusters span extended grapheme clusters, carets never land inside one). [D] CHANGELOG 0.12.0
- AccessKit integration moved out of the crate into the `editor` example (0.12). [D] CHANGELOG 0.12.0
- **Fine for text fields and the property-grid editors; wrong for the code editor.** [I]

**Performance and memory:** no published numbers were found. `parley_bench` (tango paired benchmarks) has cases for defaults, styled, glyph-run iteration, content widths, line breaking, long line, page, cursor and system-font init, but the README publishes no results. [S] `parley_bench/` at `2133d78`; [D] no figures in the Linebender 2026-Q1 report. **A tessel gate must measure parley on the M1 corpus** (µs per tree-row label and per 200-word paragraph, bytes per `Layout`). [I]

**Known gaps** [D]/[S] as above, compiled:
- ellipsis;
- tab stops;
- no shaped-output cache;
- reshaping across breaks in progress;
- `PlainEditor` relayouts everything;
- fontations version lag;
- a breaking-change cadence of about one release per 1–2 months (0.8.0 2026-03-27, 0.9.0 04-21, 0.10.0 06-01, 0.11.0 06-24, 0.11.1 08-16, 0.12.0 10-09; [D] CHANGELOG).

### 1.3 cosmic-text 0.19 on the same axes

| Axis | cosmic-text 0.19.0 | Source |
|---|---|---|
| Model | `Buffer` of `BufferLine`s, each caching its own shape and layout. Setters are lazy since 0.19. `shape_until_scroll` shapes only up to the viewport. | [D] CHANGELOG 0.19.0, [S] `src/buffer*.rs` |
| Measurement | No min/max-content API; callers lay out with `width=None` for max and compute min themselves. | [S] grep at `f1a3461` |
| Breaking and bidi | `unicode-linebreak` and `unicode-bidi`. | [S] `Cargo.toml` |
| Rich text | `AttrsList` spans, mixed sizes per line (later versions), text decoration (0.19). | [D] CHANGELOG |
| Inline boxes | **none** | [S] grep |
| Ellipsis | yes, start, middle or end (0.18.0, 2026-02-19), fixed in 0.18.1 and 0.18.2 | [D] CHANGELOG |
| Hinting | `Hinting::{Disabled, Enabled}`: "metrics hinting" snaps x during layout; rasterisation uses swash's own hinter (`.hint(...)`) | [S] `src/layout.rs`, `src/swash.rs` |
| Caches | per-line, `ShapeRunCache` (key = text + attrs spans), `SwashCache`, `SubpixelBin` (4 bins in x and y) | [S] `shape_run_cache.rs`, `glyph_cache.rs` |
| Fallback | hard-coded per-OS family lists (`font/fallback/{windows,macos,unix}.rs`, e.g. "Segoe UI", "Segoe UI Emoji", "Segoe UI Symbol", "ja" → "Yu Gothic") over fontdb | [S] |
| Editor | `Editor`, `SyntaxEditor` (syntect), `ViEditor` | [D] docs.rs |

**Verdict:** cosmic-text has ellipsis and an editor, but no intrinsic sizing, no inline boxes and an older shaping and fallback stack. parley fits a taffy-laid-out retained toolkit better. Ellipsis is a small addition. [I]

### 1.4 What quire does today (local)

These matter because tessel inherits quire.
- Per-line galley cache keyed by (line content hash, token style hash, font, pixels-per-point, wrap width). It is LRU-bounded at 4096 entries and evicts a sixteenth at a time. [S] `crates/egui_quire/src/galley_cache.rs`
- **ASCII fast path:**
  - egui 0.37's `layout_job` costs "about 45 µs and 50 allocations per newly shown line". [M] quire `docs/design/E1.md`
  - Printable-ASCII lines in the monospace font are built from glyph templates, one for each of egui's 4 subpixel bins, and match `layout_job` glyph for glyph. [S] `ascii.rs`
  - This cut a 40-lines-per-frame fling from 1.69 to 0.74 ms mean (p99 3.8 → 1.2 ms). [M] `E1.md` "After widget4"
- Soft wrap follows VS Code's `monospaceLineBreaksComputer` with column advances. Rows are laid out as separate galleys. [S] `wrap.rs`
- Heights are kept in a Fenwick-tree index (`height.rs`). Longest-line widths are tracked incrementally (`widths.rs`). [S]
- Cached galleys hold atlas coordinates and are dropped when egui rebuilds its atlas (text options change, or more than 80 % full). [D] `E1.md`. **Lesson:** tessel's shaped runs must not hold atlas coordinates; resolve them at paint time from a glyph key. [I]

---

## 2. Glyph rasterisation quality at 12–13 px

### 2.1 skrifa 0.49: what it can do now

- skrifa "ships as the font rendering backend for Chrome on Android and Linux" and is "tested glyph by glyph against FreeType". [D] skrifa `README.md` at `cf5676b`
- **Formats** [D] README table:
  - `glyf` hinted by the TrueType interpreter or the automatic hinter;
  - CFF and CFF2 by PostScript hints or the automatic hinter;
  - Type 1 and VARC by the automatic hinter;
  - COLR v0/v1 painted through a `PaintSink`;
  - EBDT, CBDT and sbix returned "as coverage masks, BGRA pixels or PNG data for the caller to decode".
- **0.49 API** (new, 2026-10-19): `Font` → `ScalerFont::hinted(instance, ppem, HintOptions)` → `outline.draw(DrawMemory, &mut PathSink)`. [S] `skrifa/src/scaler/`. The old `outline::HintingInstance` API still exists. [S] `outline/hint.rs`
- **`HintOptions`** [S] `scaler/hinting.rs`:
  - engines: `auto_fallback()` (the font's instructions where worth running, the autohinter otherwise; a hint-reliant TrueType font always runs its instructions), `interpreter()`, `auto()`;
  - `RasterTarget::{Mono, Normal, Light, Lcd, VerticalLcd}`;
  - `with_classic_cleartype`, defaulting to the newer rules: "a version 1 `gasp` table selects symmetric smoothing and grid fitting per size, and glyphs that opt out of backward compatibility with `INSTCTRL` selector 3 keep fractional horizontal positions";
  - `with_symmetric_rendering`: "ClearType fonts often generate wider horizontal stems that look blurry with an analytic area rasterizer";
  - `with_preserve_linear_metrics(true)`: "Hinting then moves points only vertically… The TrueType interpreter runs in backward compatibility mode, and the automatic hinter works as it does for a `RasterTarget::Light` target". **This is the vertical-only mode the proposal wants.**
- **`ScalerFont::raster_policy() -> Option<RasterPolicy { symmetric_smoothing, grid_fit }>`** "Returns the smoothing and grid fitting the font requests at this size". [S] `scaler/mod.rs`
  - The fallback is "the policy DirectWrite applies when no version 1 `gasp` range covers the size": **asymmetric smoothing through 20 ppem for a font that has glyph instructions**. "Windows reads the same two properties from the same source, so honoring them makes text match Windows." [S] `outline/glyf/hint/cleartype.rs`
- This matches DirectWrite: `DWRITE_RENDERING_MODE_NATURAL` "Specifies that antialiasing is performed in the horizontal direction… allows glyphs to be positioned with subpixel precision", and `NATURAL_SYMMETRIC` antialiases "in both the horizontal and vertical directions… typically used at larger sizes", in ClearType or grayscale. [D] https://learn.microsoft.com/en-us/windows/win32/api/dwrite/ne-dwrite-dwrite_rendering_mode (updated 2025-10-07)
- **Consequence:** to look like Windows at 12–13 px with Segoe UI, Tahoma or Verdana (instructed fonts, ppem ≤ 20), the rasteriser should **not antialias vertically**: it should sample coverage per scanline at pixel-centre y, after the hinter has grid-fitted y. That is a mode zeno and vello_cpu do not offer; a small own rasteriser can. [I]

### 2.2 Evidence that vertical-only hinting plus gamma is the mainstream choice

- **GTK 4.16+ automatic mode** (the default since 4.16):
  - `cairo_font_options_set_antialias(GRAY)`, `set_hint_metrics(OFF)`, `set_hint_style(SLIGHT)` and `pango_context_set_round_glyph_positions(FALSE)`. That is grayscale, light vertical-only hinting, unhinted metrics and subpixel positions, with no DPI branch in this function. [S] `gtk/gtkwidget.c` at `c36202f` (the `GTK_FONT_RENDERING_AUTOMATIC` branch)
  - The property docs say automatic "consider[s] factors such as screen resolution and scale". [D] https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html
  - GTK 4 never uses LCD AA ("our compositing does not have component alpha"). [D] https://blog.gtk.org/2024/03/07/on-fractional-scales-fonts-and-hinting/
- **egui:**
  - egui replaced ab_glyph with skrifa + vello_cpu and turned on hinting (PR #7694, merged 2025-12-06). emilk called the readability gain "pretty big"; a reviewer said hinting "on normal DPI platforms just makes a massive difference"; nicoburns found stem darkening a further improvement on macOS. WASM size grew from 10,478,576 B to 11,080,156 B. [D]/[M] https://github.com/emilk/egui/pull/7694
  - egui 0.37's default is `Smooth { light: false, symmetric_rendering: true, preserve_linear_metrics: true }`, and layout uses shaper advances, not hinted ones. [S] `epaint/src/text/font_tweak.rs`
- **Chrome:**
  - Chrome replaced FreeType with skrifa for web fonts (Chrome 133 on Linux, Android and ChromeOS), then for **system fonts on Linux and Android**. "In Chrome 145 we removed FreeType from Blink." It was validated by pixel comparison with FreeType across AA and hinting modes (the "fauntlet" tool). [D] https://developer.chrome.com/blog/memory-safety-fonts (2025-03-19, note dated 2026-04-24)
  - On Windows and macOS Chrome still rasterises system fonts with the OS. [D] same post
- **Zed on Windows** shows the limits:
  1. **Grayscale with gamma and contrast** (#37167, merged 2025-09-01) used DirectWrite rasterisation with grid fitting re-enabled. [D] https://github.com/zed-industries/zed/pull/37167
  2. **y-subpixel positioning made grid-fitted text blurry**: "text was now grid-fitted, but Zed sometimes draws text at a fractional vertical offset". The fix (#38440, 2025-09-22; Linux #38959) snaps y, at the cost that text "slightly lags behind scroll until it reaches the next pixel". [D] https://github.com/zed-industries/zed/issues/37460
  3. **Subpixel (ClearType-style) rendering on Windows and Linux, on by default** (#45423, merged 2026-01-06): "improves the legibility of text on standard DPI displays". BGR layouts followed (#55174, 2026-04-29). [D] https://github.com/zed-industries/zed/pull/45423
     - GPUI uses it only for opaque windows (`background_appearance() != Opaque → false`) and when the platform supports it. [S] `crates/gpui/src/window.rs` `should_use_subpixel_rendering` at `a5c4ca8`
  4. Users of the long-running LoDPI issue (#7992, 193 comments) called Zed 1.x "night and day" (May 2026). [D] https://github.com/zed-industries/zed/issues/7992
  5. **An open bug** (#65664, 2026-10-15): all text is vertically blurred when the client height is odd. The reporter measured sharpness 14.2–15.7 at odd heights against 18.8–20.7 at even ones, and suspects a half-pixel offset. [D]/[M] https://github.com/zed-industries/zed/issues/65664
- **Verdict** [I]:
  - Vertical-only hinting, integral baselines, x subpixel positions and DirectWrite gamma/contrast is the right default. It is what GTK ships and what Zed shipped first.
  - It is **not quite** what Windows users compare against (ClearType LCD in Explorer, VS Code and Chrome). The strongest data point is that Zed, after a year of grayscale tuning, made LCD the Windows default.
  - Keep LCD out of M1 but design the atlas, primitive and layer flags so a dual-source LCD path for opaque text layers can be added. Decide by side-by-side screenshots on a 96-DPI panel.
  - Also add the x-only AA mode (§2.1), which is the cheaper way to get closer to DirectWrite NATURAL.

### 2.3 Hard rules that come from §2.2 [I]

1. Glyph baselines land on **integral device pixels**: parley `quantize: true`, or round baselines at paint time. Scroll offsets of text layers are integral device pixels. Text in a layer animated at fractional offsets (transitions, zoom) is either re-snapped every frame or, during the animation, drawn unhinted from a separate cache key.
2. The viewport transform maps pixel centres exactly. Odd window sizes must not produce half-pixel offsets (Zed #65664). Add a test that renders at odd sizes.
3. Advances are never hinted (skrifa reports rounded advances for hinted draws; ignore them). Layout is then independent of hinting and DPI, as in egui and GTK automatic.
4. Hint only at scale < ~1.75. At HiDPI and on macOS, draw unhinted (gpu-2d-text.md §2.5).

### 2.4 Rasteriser choice

| Option | Notes |
|---|---|
| **Own accumulation rasteriser** (font-rs design: "very similar in design to libart", drawn straight into a dense buffer, "exact subpixel area computation and a winding-number integration step") | About 300–500 lines. We control x-only AA, gamma-free linear coverage and determinism (scalar f32, no FMA). [D] https://github.com/raphlinus/font-rs README; [I] size estimate. |
| **zeno 0.3.3** (2025-05-08; swash's rasteriser) | Mature and small. Last release 17 months ago; no x-only mode. [D] crates.io |
| **vello_cpu 0.3.0** (2026-10-02) via **glifo 0.4.0** | egui 0.37's choice: it handles COLRv1 gradients and bitmaps ("Render colored emojis on native", egui #8492, 2026-09-05). Heavy, and it pins skrifa 0.44. glifo is "experimental and under rapid development". [D] docs.rs/glifo 0.4.0; [S] `epaint/src/text/font_face.rs` |
| swash 0.2.10 scaler | Has its own hinter, not skrifa's. Zed Linux uses `ScaleContext … .hint(true)` with `Render::new(sources).format(Format::Alpha)`. [S] `gpui_wgpu/src/cosmic_text_system.rs` |
| ab_glyph 0.2.32, fontdue 0.9.5 | No hinting (fontdue's README never mentions it). [D] |

- **Speed:** no current head-to-head numbers were found. egui #7694 gives none; font-rs claims "extremely promising performance compared with Freetype" without numbers. [D]
- At 12–13 px a glyph is about 10×14 px, and every glyph is rasterised once per (glyph, size, subpixel bin, mode), so raster speed matters only for the first frame and for zoom. A cold UI screen of about 300 unique glyphs × 4 bins is the case to measure. [I]
- **Recommendation:** our own rasteriser for monochrome coverage, skrifa `PaintSink` for COLR, and PNG decode for sbix and CBDT. If COLRv1 gradients need a real painter, use vello_cpu behind a feature, or draw the COLRv1 layers ourselves (solid layers plus linear, radial and sweep gradients over clip paths). [I]

### 2.5 macOS specifics (risk)

- **CoreGraphics font smoothing is a luminance-dependent dilation.** Zed replicates it:
  - level = `clamp(floor(4·L + 0.5), 0, 4)` with `L = 0.2126 r + 0.7152 g + 0.0722 b`, and the level is part of the glyph cache key;
  - it is rasterised with `set_should_smooth_fonts(true)` and gray fill `level·0.25`;
  - it is skipped when `AppleFontSmoothing = 0`.
  - [S] `crates/gpui_apple/src/text_system.rs` at `a5c4ca8`
- **A skrifa rasteriser therefore draws lighter text than native macOS** unless tessel emboldens outlines by a luminance-dependent amount. nicoburns' stem-darkening note is in egui #7694. The CG dilation amounts are not public. [I]
- **PingFang (the system CJK font) can contain `hvgl` outlines** (Apple's Hierarchical Variation Fonts, https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hvgl.html). fontations has no `hvgl` support (no match in the tree at `cf5676b`).
  - fontique works around it by mapping Hani/Hans to "Heiti SC" and Hant to "Heiti TC" when PingFang is unusable. [S] `fontique/src/backend/coretext.rs`; [D] parley CHANGELOG 0.9.0 "CJK text on macOS when PingFangUI contains hvgl outlines".
- **Recommendation:** skrifa on macOS for M1 (it is HiDPI there, so the differences are small) plus a dilation emulation. Keep the `GlyphRasterizer` trait so a CoreText backend can come later, as §6.5 already says. [I]

### 2.6 Version skew (risk)

- parley 0.12 brings skrifa 0.44 and read-fonts 0.41. The DirectWrite-matching `raster_policy` and the ClearType "enhanced" interpreter landed in skrifa 0.49 (`13b7c17`, 2026-10-19). [S]
- **Options** [I]:
  - (a) take skrifa 0.49 for rasterisation alongside parley's 0.44 for shaping. That gives two read-fonts copies in the binary. Font bytes, face index, glyph ids and normalised coords cross the boundary as plain data, so it works.
  - (b) wait for a parley release on read-fonts 0.46.
  - (c) on 0.44, use the `HintingInstance` API with `Target::Smooth { mode: Light, symmetric_rendering: false, preserve_linear_metrics: true }` and compute the gasp policy ourselves from read-fonts. That is about 30 lines, copying `RasterPolicy::resolve`.
- **Recommendation: (c) now, (a) when the version gap matters.**

---

## 3. Gamma and contrast correction

### 3.1 The exact DirectWrite grayscale formula (Windows Terminal AtlasEngine)

[S] `src/renderer/atlas/dwrite_helpers.hlsl`, `shader_ps.hlsl` and `dwrite_helpers.cpp` at `98c8c79` (MIT). Zed's WGSL port is in `crates/gpui_wgpu/src/shaders.wgsl` at `a5c4ca8`, "adapted from … microsoft/terminal …/dwrite.hlsl".

```wgsl
// a: linear coverage from the R8 atlas (rasterised with gamma 1, contrast 0).
// c: text colour, straight (non-premultiplied) sRGB-encoded RGB in [0,1].
// k: grayscale enhanced contrast (DirectWrite default 1.0; ClearType default 0.5).
// g: gamma ratios for the user's gamma (1.8 default) = (0.148054421, -0.894594550, 1.47590804, -0.324668258).
fn dwrite_grayscale_alpha(a: f32, c: vec3<f32>, k: f32, g: vec4<f32>) -> f32 {
    let lightness = dot(c, vec3(0.30, 0.59, 0.11));              // Rec.601
    let k_eff = k * saturate(4.0 * (0.75 - lightness));           // no boost for light text
    let ac = a * (k_eff + 1.0) / (a * k_eff + 1.0);               // enhanced contrast
    let f = dot(c, vec3(0.25, 0.5, 0.25));                        // "intensity"
    return ac + ac * (1.0 - ac) * ((g.x * f + g.y) * ac + (g.z * f + g.w));
}
// output (premultiplied): vec4(c * a', a') * text_alpha; blend ONE, ONE_MINUS_SRC_ALPHA.
```

- **Gamma ratios:** WT's `DWrite_GetGammaRatios` has a 13-row table, `gammaIncorrectTargetRatios`, for gamma 1.0–2.2 in steps of 0.1. The 1.8 row is `{0.1469, -0.8911, 1.4644, -0.3234}/4`, normalised by `norm13 = 0x10000/(255·255)·4` for entries 0 and 2 and `norm24 = 0x100/255·4` for entries 1 and 3. [S]
  - I recomputed the 1.8 row: 0.148054, -0.894595, 1.475908, -0.324668. [I]
- **Inputs:** `IDWriteRenderingParams1::GetGamma`, `GetEnhancedContrast` and `GetGrayscaleEnhancedContrast` from `CreateRenderingParams`, cached for the app's lifetime ("applications aren't expected to refresh the rendering params after startup"). [S] `dwrite_helpers.cpp`
  - Gamma is valid in (0, 256]; enhanced contrast is ≥ 0, "typically ranges from 0 to 1". [D] https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nf-dwrite-idwriterenderingparams-getgamma and …-getenhancedcontrast
  - The values are per monitor (`CreateMonitorRenderingParams`). [D] https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nf-dwrite-idwritefactory-createrenderingparams
  - The atlas is rasterised with custom params gamma 1.0, contrast 0, so the correction is not applied twice. [S]
- **Thin fonts:** a `isThinFont` +0.5 contrast boost applies to "Courier New", "Fixed Miriam Transparent", "Miriam Fixed", "Rod", "Rod Transparent" and "Simplified Arabic Fixed". [S] `dwrite_helpers.cpp`
- **Zed's defaults on Linux and wgpu:** gamma 1.8 (env `ZED_FONTS_GAMMA`, clamped 1.0–2.2), grayscale contrast 1.0, subpixel contrast 0.5. [S] `gpui_wgpu/src/wgpu_renderer.rs`
- **Chrome** made text contrast 1.0 its Windows default in Chrome 132 because it "closely matched … pre-Chromium Edge". [D] https://developer.chrome.com/blog/better-text-rendering-in-chromium-based-browsers-on-windows (2025-02-12)

**What it does** [I], computed from the formula above at gamma 1.8 and k = 1. Table entries are output alpha.

| coverage | black text | mid-grey | white text |
|---|---|---|---|
| 0.25 | 0.236 | 0.420 | 0.431 |
| 0.50 | 0.462 | 0.637 | 0.694 |
| 0.75 | 0.723 | 0.822 | 0.861 |

- Light-on-dark text gets more coverage, which counters the thinning that light text suffers when blended in gamma space.
- Dark-on-light text stays near identity: the enhanced contrast offsets the gamma term's thinning.
- **So one shader serves both light and dark themes without per-theme transfer curves.**
- egui instead switches per theme between `Off` (light) and `2c − c²` (dark), applied when writing the atlas. [S] `epaint/src/image.rs` `FontColorTransferFunction`
- Because the formula takes the text colour as input, it needs no background colour. [I]

### 3.2 Linear vs sRGB blending in wgpu

- **The DirectWrite constants assume blending in sRGB-encoded (gamma) space.** The table is literally named `gammaIncorrectTargetRatios`. [S]
  - WT's swap chain is `DXGI_FORMAT_B8G8R8A8_UNORM` (non-sRGB). [S] `BackendD3D.cpp`
  - Zed's wgpu renderer prefers `Bgra8Unorm` and `Rgba8Unorm`, then any format `!has_srgb_suffix()`. [S] `wgpu_renderer.rs`
- **Linear blending makes dark text thin and light text bold:**
  - Ghostty: `linear` "makes dark text look much thinner than normal and light text much thicker". Its default off macOS is `linear-corrected`, which needs the **background luminance per pixel**: `a' = (lin(unlin(fg_l)·a + unlin(bg_l)·(1−a)) − bg_l)/(fg_l − bg_l)`. [D] https://ghostty.org/docs/config/reference#alpha-blending; [S] `shaders.metal` at `cec8b92`
  - kitty's `text_composition_strategy` exposes gamma (default 1.0 Linux, 1.7 macOS) and contrast (0 Linux, 30 macOS). [D] https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.text_composition_strategy
  - egui blends in gamma space after finding "proper anti-aliasing of fonts was very difficult to accomplish in linear space". [D] https://github.com/emilk/egui/pull/2071
- **Recommendation for tessel** [I]:
  - Use **non-sRGB surface and intermediate formats** (`Bgra8Unorm`/`Rgba8Unorm`) and blend in sRGB-encoded space everywhere.
  - That matches how browsers and CSS composite, which svelters content expects. It makes the DirectWrite formula exact, and it needs no background colour (which a general toolkit, unlike a terminal, does not know).
  - 3D viewports render linear or HDR internally and encode to sRGB before being composited as a layer.
  - The R8 atlas is `R8Unorm` holding linear coverage.
  - Colour emoji go in an `Rgba8Unorm` premultiplied atlas, blended without correction (Zed #37167: premultiplied, because straight alpha "is not associative").

### 3.3 Per platform [I]

- **Windows:** read the gamma and contrasts from DirectWrite at startup and on display change (`WM_SETTINGCHANGE`, monitor move). Defaults are 1.8 / 1.0.
- **Linux:** 1.8 / 1.0 (Zed's values); later, read `Xft.*`/fontconfig hinting preferences.
- **macOS:** contrast 0, gamma ratio for 1.0 (identity), plus the dilation emulation of §2.5. Honour `AppleFontSmoothing = 0`.
- Make it a setting (Zed's `ZED_FONTS_*` precedent) and a theme token, not a constant.

---

## 4. Fonts: system enumeration, fallback, colour and variable fonts

### 4.1 fontique 0.12

**Backends** [S] `fontique/src/backend/` at `2133d78`:
- `dwrite.rs`: enumerates the system collection, with fallback through `IDWriteFontFallback::MapCharacters` given text and locale.
- `coretext.rs`: CoreText enumeration plus a directory scan of the `Library/Fonts` paths (0.8). Fallback through `create_fallback_font_for_text(script.sample(), locale)`, with the PingFang→Heiti hack.
- `fontconfig.rs`: a fontconfig pattern with `FC_LANG` from the locale and `FC_CHARSET` from a per-script charset. `fontconfig-dlopen` is optional.
- `android.rs`: reads the `fonts.xml` config.
- HarmonyOS gets a dummy backend (#913, main).

**Other API** [D]/[S]:
- `CollectionOptions { shared, system_fonts }`. `system_fonts: false` gives a hermetic collection for tests. [S] `collection/mod.rs`
- `load_system_fonts` and `load_fonts_from_paths` (0.8).
- `FallbackKey` is (script, locale). A `FallbackMap` caches per script, and per locale within a script. Main also caches system fallback misses (#900). [S] `fallback.rs`; git log.

**Gap** [I]: fallback is keyed by **script**, using a sample string for that script. Characters in the Common script (Zyyy: arrows, math, box drawing, ⌘, ✓, technical symbols, which are common in CAD UIs and logs) and the emoji presentation sequences resolved by `parley_emoji` need an explicit symbol chain. Per OS:
- Windows: Segoe UI Symbol, Segoe UI Emoji, Cambria Math.
- macOS: Apple Symbols, Apple Color Emoji, STIX Two Math.
- Linux: Noto Sans Symbols 2, Noto Color Emoji, Noto Sans Math.
- The cosmic-text lists are a precedent: [S] `cosmic-text/src/font/fallback/*.rs`.

### 4.2 CJK fallback per OS [I], from §4.1 and §2.5

- **Windows:** DirectWrite `MapCharacters` with the locale picks Yu Gothic UI (ja), Microsoft YaHei UI (zh-Hans), Microsoft JhengHei UI (zh-Hant) or Malgun Gothic (ko). Pass the UI locale, so Han text gets the right regional glyphs.
  - parley 0.12 "now honours script and region subtags for correct handling of, for example, zh-Hans and zh-Hant". [D] CHANGELOG 0.12.0
- **macOS:** PingFang, but it may be unreadable (`hvgl`), so expect Heiti. **Test CJK on macOS 15/26 early.**
- **Linux:** Noto Sans CJK through fontconfig `FC_LANG`.

### 4.3 Colour fonts and emoji

- **skrifa** reads COLRv0/v1 (v1 with variations) through `PaintSink`, and EBDT, CBDT and sbix as masks, BGRA or PNG. [D] skrifa README at 0.49
- **What each OS ships:**
  - Apple Color Emoji is sbix (PNG). [I] (well known; not re-verified here)
  - Windows 11 Segoe UI Emoji is a "hybrid" font with COLRv1, COLRv0 and monochrome glyphs; COLRv1 support was added to DirectWrite and Direct2D. [D] https://forest.watch.impress.co.jp/docs/news/1572154.html, https://blog.yuo.be/2026/03/26/im-not-a-fan-of-how-direct2d-renders-windows-11-emojis/
  - Noto Color Emoji on Linux is CBDT or COLRv1, depending on the distribution's package. [I]
- **Path** [I]:
  - `parley_emoji` resolves presentation and drives font selection (it is "internal to Parley"; [D] `parley_emoji/README.md`).
  - Colour glyphs are rasterised once per (glyph, ppem bucket), premultiplied into the RGBA atlas. Bitmap strikes are scaled from the nearest strike, so bitmap emoji are cached at a few sizes and scaled with linear filtering.
  - Emoji are not tinted and skip the gamma correction (egui: "colour glyphs (e.g. emoji) must skip it"; [S] `glyph_atlas.rs`).
- **Precedents:** GPUI keeps `MonochromeSprite` (R8, tintable) and `PolychromeSprite` (RGBA) as separate primitives (gpu-2d-text.md §2.8). parley's examples render "bitmap emoji, COLR emoji, Arabic (bidi)" through swash, tiny-skia or vello_cpu. [S] `parley/examples/common/src/lib.rs`

### 4.4 Variable fonts [D]/[S]

- parley carries normalised coordinates per run (`Run::normalized_coords() -> &[NormalizedCoord]`, 0.12).
- fontique synthesises bold only when the weight difference is > 200 (0.12), and selects the right variable weight in synthesis.
- skrifa hints `glyf` variable fonts (TrueType interpreter or autohint) and CFF2 (PostScript hints).
- Atlas keys must include a hash of the coordinates. [I]
- **Segoe UI Variable (Windows 11) and SF Pro (macOS) are variable fonts**, so this matters for the default UI font. [I]

---

## 5. Large-text performance and the editor

### 5.1 What others do

**Zed / GPUI**
- `LineLayoutCache` has `previous_frame` and `current_frame` maps. Used keys move forward each frame and the rest are dropped. There are separate maps for unwrapped and wrapped lines, plus content-addressable maps keyed by a caller-provided text hash. A font-generation counter clears both. [S] `crates/gpui/src/text_system/line_layout.rs` at `a5c4ca8`
- Wrapping uses **per-char advance caches** (`cached_ascii_char_widths: [Option<Pixels>; 128]` plus a `HashMap<char, Pixels>`), not shaped widths. [S] `line_wrapper.rs`
- Glyph origins are quantised to 4 x-bins (`SUBPIXEL_VARIANTS_X`) and snapped in y. [S] `window.rs`

**VS Code GPU renderer** (experimental)
- It is a **cell grid**: one rasterised glyph per grapheme segment (`Intl.Segmenter`).
- Glyphs are drawn by Canvas2D `fillText`. Sub-pixel x offsets are binned in tenths ("0 = 0.0px x offset, 9 = 0.9px"). Glyphs are LCD on non-mac (`isMacintosh ? 'greyscale' : 'subpixel'`).
- The atlas uses slab and shelf allocators over multiple pages, and is warmed up with printable ASCII for each token colour on idle callbacks.
- There are full-file and viewport upload strategies, and a hard column cap.
- [S] `src/vs/editor/browser/gpu/{atlas,raster,renderStrategy}/*.ts` at `53e8ffb`
- So it skips shaping (no ligatures) for speed. [I]

**xi-editor**
- The async split made the line cache and wrap-on-resize hard; "monolithic was better". [D] gpu-2d-text.md §2.7 (Raph Levien 2020-06-27)

**cosmic-text**
- A per-`BufferLine` shape and layout cache, invalidated per line; `shape_until_scroll` for the viewport. [D]

**quire**
- Already proven: viewport-only layout, a per-line cache, an ASCII template fast path and incremental line widths and heights (§1.4).

### 5.2 Editor recommendation [I]

- **Shape per logical line, viewport plus overscan only.** quire-text's `LineEdit`s and splices name the lines to reshape; nothing else is reshaped on an edit.
- **Two shaping paths:**
  1. **ASCII monospace fast path:** a printable-ASCII line (plus tabs) in the editor font with colour-only token runs needs no shaper. Glyph id and advance come from a 95-entry table per (font, ppem, coords); x = column × advance + tab stops; the glyph key = (glyph id, x-bin). quire measured this path as roughly halving fling cost (§1.4). It also covers the gutter.
  2. **General path:** `parley_engine::{Analyzer, Shaper}` on the line text. That gives bidi levels, graphemes, emoji presentation, fallback through our `select_font` callback (fontique) and harfrust shaping, with ligatures off or on per setting. Layout (breaking and positioning) stays ours: quire's VS Code wrap algorithm over cluster advances.
- **Do not use `parley::Layout` per editor line:** it carries web line-box, whitespace and alignment machinery, a per-layout style table and builder overhead that an editor line does not need. [I]
- **Cache:** the key is (line-content hash, token-style-run hash, font-set generation, ppem bits, variation-coords hash, feature set). The value is the shaped glyph run (glyph ids, advances, cluster map), **not** atlas coordinates or wrap rows.
  - Wrapped rows are a separate cache keyed by (shaped-run id, wrap width bits, indent).
  - Eviction is GPUI's two-generation scheme plus an LRU byte cap, e.g. 16 MB, for lines scrolled away. The sixteenth-at-a-time eviction is quire's lesson.
- **Long lines:** a 1M-char line is laid out in windows around the viewport. quire's E3 F2 "long-line prefixes" already handles this (E3 1M-char line 0.48 ms; [M] quire STATUS QE4).
- **Ligatures and kerning across token boundaries:** shape the whole line with colour-only style runs, so token colours do not split shaping, as egui #8031 "Use harfrust for better kerning and ligatures" and parley 0.12 #740 ("Shaping across style boundaries now uses the surrounding context") both do.

### 5.3 UI text caching design (tessel-text) [I]

1. **Fonts:** one `FontContext` (fontique `Collection` + `SourceCache`) per process, `shared: true` only if a text thread is added. A font-set generation counter is bumped on font load or system font change. Fallback is memoised per (script, locale, family stack), with per-codepoint memos for Common-script symbols.
2. **Shaped paragraphs:**
   - Key: (text hash, style-runs hash, font-set generation, scale, quantize, OpenType features and variations). Value: `Arc<parley::Layout<Brush=u32 style id>>`, shaped once.
   - The brush is a **style id**, not a colour, so theme and colour changes and hover tints do not reshape. Colour is resolved at paint time.
   - Eviction is two-generation (GPUI) plus an LRU byte budget.
3. **Breaks:** each layout keeps its last `break_all_lines(width)` result. `calculate_content_widths()` is memoised per layout for taffy's min/max-content query. Height-for-width keeps the last 2–3 widths (taffy asks for several during one pass).
4. **Ellipsis:** computed after breaking from cluster advances, with a pre-shaped "…" per style; no reshaping.
5. **Glyph raster cache:**
   - Key: (font blob id + index, coords hash, glyph id, ppem in 26.6, x-bin 0–3, hint mode, AA mode {xy, x-only}, dilation level 0–4).
   - R8 atlas pages of 1024² (1 MB each; grow to 2048²), etagere 0.3.0 allocator ([D] crates.io 2026-03-18). RGBA premultiplied pages for colour.
   - Eviction is per page by frame generation, xterm.js-style multi-page (gpu-2d-text.md §2.1). Never evict a glyph used in the current frame. Hinting instances are cached per (font, ppem, coords, options), as glifo's `HintCache` does ([D] docs.rs glifo 0.4.0).
   - **Budget estimate:** 95 ASCII × 4 bins × 4 faces (UI regular and bold, mono regular and bold) ≈ 1.5k entries of about 10×14 px, roughly 0.25 MB. A full UI with CJK fits in 1–2 pages. [I]
6. **Display list:** a text node records glyph instances (integer device position plus bin, glyph key, style id) once per layout and break. Atlas coordinates are resolved per frame through a cheap key → slot map with an atlas-generation check. This avoids quire's "cached galleys point into the old atlas" problem.

---

## 6. Canvas text (PCB, schematic, footprint and 3D labels)

### 6.1 Slug

- The patent (US 10,373,352) has been dedicated to the public domain since 2026-03-17. [D] https://terathon.com/blog/decade-slug.html
- The reference `SlugVertexShader.hlsl` and `SlugPixelShader.hlsl` are MIT/Apache, and distributed software "must give credit". [D] https://github.com/EricLengyel/Slug
- **Data:**
  - A curve texture holds quadratic Bézier control points in RGBA16F, endpoints shared between consecutive curves.
  - A band texture (RG16UI) holds curves assigned to horizontal and vertical bands with an epsilon of about 1/1024 em, sorted by descending max x or y. Straight lines along a band axis are excluded; lines are encoded as quadratics with a duplicated endpoint.
  - The README also suggests sizing so cap height lands on whole pixels, and states that hinting is ignored. [D] README
- **2026 changes:**
  - The band split was removed: it "introduced some divergence … that could hurt performance … at a small size".
  - Supersampling was removed: it mattered "only for text so small that it was barely readable".
  - **Dynamic dilation:** the vertex shader expands each glyph quad by half a pixel in viewport space.
  - [D] decade-slug post
- **Rust ecosystem: immature** [D] crates.io 2026-10-25:
  - `sanscale` 0.1.0 (2026-09-23, 14 downloads);
  - `sluggrs_skylines` 0.3.4 (a fork of sluggrs, 49 downloads);
  - `bevy_slugtext` 0.1.1 (2026-07-12);
  - `ash-slug` 0.1.5;
  - `sigilbuzz-gpu` 0.3.0;
  - `wgpu-font-renderer` 1.1.0, a JCGT-2017 implementation with 1.5k downloads.
- **Recommendation:** our own WGSL port, a few hundred lines of shader plus a CPU band builder over skrifa's `PathSink` (cubics from CFF split into quadratics). Credit Lengyel. [I]
- **Fit** [I]:
  - It is exact at any zoom, rotation or perspective, needs no atlas or regeneration on zoom, and costs one quad per glyph. Its per-pixel cost grows with the curves in a band, so it is cheapest when glyphs are small on screen, which is the common zoomed-out case.
  - It suits net labels, reference designators, pin names, dimension text and 3D PMI labels.
  - **Weak below about 6–8 px on screen** (no hinting, aliasing). There, switch to atlas glyphs at the snapped size, or greek the label (draw a bar) as the canvas LOD rule. EDA tools greek tiny text anyway.

### 6.2 Stroke fonts

- KiCad's default PCB and schematic font is a **stroke font of line segments** (`newstroke_font[]`, "an array of strokes for Unicode glyphs"), with a separate `OUTLINE_FONT` class for TrueType. [D] https://docs.kicad.org/doxygen/newstroke__font_8h_source.html, https://docs.kicad.org/doxygen/classKIFONT_1_1OUTLINE__FONT.html
- Stroke text with a stroke width is **geometry**, which has to match the manufactured silkscreen. It belongs in the canvas's capsule and line instance primitives, not in a text renderer. [I]
- So tessel needs **Slug only for TrueType and OpenType canvas labels.** Imported Altium stroke and TrueType texts, from the ecadabra lane, follow the same split. [I]

### 6.3 MSDF

- The libraries are `fdsm` 0.8.0 (2025-10-02, pure-Rust MSDF, 443k downloads) and `msdf` 0.2.1 (2022, msdfgen bindings). [D] crates.io
- MSDF needs per-glyph field generation and an atlas. Its corners stay sharp only up to a magnification set by the field resolution, and it shows artifacts at large sizes (Qt doc, gpu-2d-text.md §2.4). PCB zoom covers about 10⁴×, so MSDF would need regeneration or mip chains. **Do not use it.** [I]

---

## 7. Deterministic rendering for snapshot tests vs native look

- **Determinism** [I]:
  - The TrueType interpreter and the autohinter work in fixed point (ports of FreeType), so hinted outlines are bit-stable for the same font bytes and skrifa version.
  - Our rasteriser in scalar f32 without FMA contraction is IEEE-deterministic across x86-64 and AArch64. Avoid SIMD paths with different reductions; vello_cpu dispatches per-ISA kernels through fearless_simd.
  - Shaping (harfrust, integer font units) is deterministic.
- **The GPU stage is not bit-exact across vendors:**
  - quire records snapshots on macOS only, "as other backends (lavapipe, GL) rasterise differently". [S] quire `crates/egui_quire/Cargo.toml` comment
  - egui_kittest 0.37's default per-pixel threshold is 0.6 (weighted squared YIQ distance), with `max_failed_pixels` 0. [D] https://docs.rs/egui_kittest/0.37.0/egui_kittest/struct.SnapshotOptions.html
- **Recommendation** [I]:
  1. Tests use bundled fonts only (`CollectionOptions { system_fonts: false }`, e.g. Inter or Noto Sans plus JetBrains Mono plus Noto Color Emoji COLRv1), fixed gamma and contrast constants, and the skrifa rasteriser.
  2. **Exact goldens on CPU-side artefacts:** the shaped glyph-run list (ids, positions, bins), the atlas tile bytes for a glyph set, and the text display list. These are what agents should diff.
  3. **Tolerance goldens on GPU pixels** (kittest threshold), recorded on one reference backend (lavapipe pinned at Mesa 26.1.3, as `scripts/xrun.sh` already does).
  4. Native look at runtime (DirectWrite parameters, macOS dilation, future CoreText or LCD paths) is a *configuration* that tests turn off. Production may differ from goldens only along those switches.
- **Trade-off:** skrifa everywhere is near-native on Linux (Chrome and GTK use the same FreeType-equivalent stack), close on Windows with x-only AA and DirectWrite gamma (minus ClearType LCD), and lighter than native on macOS without dilation. [I]

---

## 8. Open risks

1. **Grayscale-only may disappoint Windows 96-DPI users.** Zed moved to default-on LCD in Jan 2026 (§2.2). Mitigation: x-only AA at ≤20 ppem, DirectWrite parameters, and a planned LCD path for opaque layers (wgpu `Features::DUAL_SOURCE_BLENDING`, supported on Vulkan, Metal, DX12 and WebGPU per gpu-2d-text.md §2.2).
2. **Fontations version skew:** parley 0.12 is on skrifa 0.44 and read-fonts 0.41, while the current releases are skrifa 0.49 and harfrust 0.15 (§2.6). egui 0.37 is blocked the same way, so cadatomic's egui island and tessel should move together.
3. **parley churn and gaps:** breaking releases every 1–2 months; no ellipsis (#304), no tabs (#302), no shaped-output cache; reshaping inside ligatures is still in progress; `PlainEditor` relayouts the whole buffer.
4. **No published parley performance numbers.** An M1 gate must measure it: µs per label, per 200-word paragraph and per taffy measure pass, plus bytes per `Layout`.
5. **The sRGB-space blending decision** affects all of tessel-render, not only text (§3.2). Choosing `*Srgb` surfaces would force a Ghostty-style background-aware correction or emboldening.
6. **macOS fidelity:** no CoreGraphics dilation emulation yet, and PingFang `hvgl` outlines are unsupported, falling back to Heiti (§2.5).
7. **Fallback for Common-script symbols** needs explicit per-OS chains (§4.1).
8. **COLRv1 painting** needs a gradient-capable painter (vello_cpu or our own).
9. **Slug:** our own port (no mature crate), no hinting at tiny sizes (needs the LOD switch), and an attribution requirement.
10. **ICU4X data size:** parley compiles in `icu_segmenter` and `icu_properties` data. The `complex-scripts` dictionaries add more, and can be loaded dynamically (#881). Measure binary size; the egui precedent added about 600 kB in WASM for skrifa and vello_cpu.
11. **Pixel-grid bugs:** y-subpixel blur (Zed #37460) and odd-size half-pixel blur (Zed #65664). Add explicit tests at odd window sizes and fractional scroll.

## Appendix: sources (accessed 2026-10-25)

- parley 0.12.0: https://docs.rs/parley/0.12.0/parley/ ; https://github.com/linebender/parley (CHANGELOG, `parley_engine` README, issues #304, #302, #386)
- Linebender 2026 Q1: https://linebender.org/blog/tmil-25/
- skrifa 0.49.0 README and source: https://github.com/googlefonts/fontations/tree/main/skrifa
- harfrust 0.15.0: https://github.com/harfbuzz/harfrust
- cosmic-text 0.19.0: https://github.com/pop-os/cosmic-text
- glifo 0.4.0: https://docs.rs/glifo/latest/glifo/
- egui PR #7694: https://github.com/emilk/egui/pull/7694 ; egui PR #2071: https://github.com/emilk/egui/pull/2071
- Chrome, "Memory safety for web fonts": https://developer.chrome.com/blog/memory-safety-fonts
- Chrome, "Better text rendering … on Windows": https://developer.chrome.com/blog/better-text-rendering-in-chromium-based-browsers-on-windows
- GTK: https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html ; https://blog.gtk.org/2024/03/07/on-fractional-scales-fonts-and-hinting/ ; `gtk/gtkwidget.c` at `c36202f`
- DirectWrite: https://learn.microsoft.com/en-us/windows/win32/api/dwrite/ne-dwrite-dwrite_rendering_mode ; …/nf-dwrite-idwriterenderingparams-getgamma ; …/nf-dwrite-idwriterenderingparams-getenhancedcontrast ; …/nf-dwrite-idwritefactory-createrenderingparams
- Windows Terminal AtlasEngine: https://github.com/microsoft/terminal/tree/main/src/renderer/atlas (`dwrite_helpers.hlsl`, `dwrite_helpers.cpp`, `shader_ps.hlsl`, `BackendD3D.cpp`)
- Zed: PRs #37167, #37622, #38440, #38959, #45423, #55174; issues #7992, #37460, #65664 (github.com/zed-industries/zed)
- Ghostty alpha-blending: https://ghostty.org/docs/config/reference#alpha-blending
- kitty text_composition_strategy: https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.text_composition_strategy
- WezTerm freetype_load_target: https://wezterm.org/config/lua/config/freetype_load_target.html
- Alacritty text shader (dual-source, no gamma): https://github.com/alacritty/alacritty/blob/master/alacritty/res/glsl3/text.f.glsl (crossfont 0.8.1)
- Behdad, State of Text Rendering 2024: https://behdad.org/text2024/
- Slug: https://terathon.com/blog/decade-slug.html ; https://github.com/EricLengyel/Slug
- Apple `hvgl`: https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hvgl.html
- KiCad fonts: https://docs.kicad.org/doxygen/newstroke__font_8h_source.html
- egui_kittest SnapshotOptions: https://docs.rs/egui_kittest/0.37.0/egui_kittest/struct.SnapshotOptions.html
- font-rs: https://github.com/raphlinus/font-rs
- crates.io API (versions and dates for every crate named), queried 2026-10-25
