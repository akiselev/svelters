//! Glyph rasterisation: [`FontRasterizer`] turns a [`GlyphKey`] into a [`GlyphImage`] on the
//! compositor thread.
//!
//! - **Outlines** come from skrifa (the version parley pins), drawn at the key's ppem with the
//!   subpixel x offset `x_bin / 4` px. With [`HintMode::Vertical`] they are hinted vertically only:
//!   `Engine::AutoFallback` with `Target::Smooth { mode: Light, symmetric_rendering: false,
//!   preserve_linear_metrics: true }` (the font's instructions where worth running, else the
//!   autohinter; points move in y only, advances stay linear). Hinting instances are cached per
//!   (font, ppem, coordinates).
//! - **Coverage** is our own deterministic rasteriser ([`coverage`]) in grayscale or horizontal-only
//!   mode ([`AaMode::HorizontalOnly`]); [`AaMode::Lcd`] is not implemented and rasterises as
//!   grayscale (architecture §9.4).
//! - **Policy**: [`raster_policy`] is DirectWrite's `gasp` rule set; [`FontRasterizer::preferred_aa`]
//!   applies it to choose the AA mode a renderer should put in its keys.
//! - **Synthesis**: emboldening offsets the flattened outline by `embolden_64 / 64` px; skew shears
//!   it.
//! - **Dilation** (the macOS look) emboldens by a per-level amount from [`RasterConfig::dilation_px`];
//!   the amounts CoreGraphics uses are not public, so the default table is empty and a dilation
//!   level then draws undilated.
//! - **Colour glyphs** ([`color`]): COLRv0/v1 painted into premultiplied RGBA, sbix/CBDT/EBDT
//!   bitmaps decoded and scaled to the ppem.

pub mod color;
pub mod coverage;

use crate::fonts::FontStore;
use coverage::{Accumulator, Cmd, Pt};
use skelter_base::ids::FontId;
use skelter_base::text::{AaMode, GlyphFormat, GlyphImage, GlyphKey, GlyphRasterizer, HintMode};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target};
use skrifa::raw::tables::gasp::GaspRangeBehavior;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::sync::{Arc, Mutex};

/// Hinting instances kept (per font, ppem, coordinates).
const HINT_CACHE: usize = 64;

/// The smoothing and grid fitting a font requests at a size (the `gasp` table, as DirectWrite
/// reads it).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RasterPolicy {
    /// Antialias in both axes; `false` = horizontally only.
    pub symmetric_smoothing: bool,
    /// Apply TrueType grid fitting.
    pub grid_fit: bool,
}

/// DirectWrite's raster policy for `font` at `ppem`.
///
/// Copied from skrifa 0.49 `outline/glyf/hint/cleartype.rs` (`RasterPolicy::fallback` and
/// `RasterPolicy::resolve`, googlefonts/fontations, MIT OR Apache-2.0; modifications by the svelters
/// authors), because the skrifa parley 0.12 pins (0.44) predates it: without a version 1 `gasp`
/// range covering the size, a font with glyph instructions gets horizontal-only smoothing through
/// 20 ppem and grid fitting; everything else is symmetric.
pub fn raster_policy(font: &FontRef<'_>, ppem: u16) -> RasterPolicy {
    let has_glyph_instructions =
        font.maxp().map(|maxp| maxp.max_size_of_instructions().unwrap_or_default() != 0).unwrap_or_default();
    let mut policy =
        RasterPolicy { symmetric_smoothing: !has_glyph_instructions || ppem > 20, grid_fit: has_glyph_instructions };
    if let Ok(gasp) = font.gasp()
        && gasp.version() >= 1
            && let Some(range) = gasp.gasp_ranges().iter().find(|range| ppem <= range.range_max_ppem()) {
                let behavior = range.range_gasp_behavior();
                policy.symmetric_smoothing = behavior.contains(GaspRangeBehavior::GASP_SYMMETRIC_SMOOTHING);
                policy.grid_fit = behavior.contains(GaspRangeBehavior::GASP_SYMMETRIC_GRIDFIT);
            }
    policy
}

/// Rasteriser settings that are not part of a glyph key.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RasterConfig {
    /// Emboldening in pixels for dilation levels 1..=4 (index 0 = level 1). Empty = dilation off.
    pub dilation_px: Vec<f32>,
}

type HintKey = (FontId, u32, u64);

pub struct FontRasterizer {
    fonts: Arc<FontStore>,
    config: RasterConfig,
    hinters: Mutex<Vec<(HintKey, Arc<HintingInstance>)>>,
}

impl std::fmt::Debug for FontRasterizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontRasterizer").field("config", &self.config).finish()
    }
}

impl FontRasterizer {
    pub fn new(fonts: Arc<FontStore>) -> Self {
        Self::with_config(fonts, RasterConfig::default())
    }

    pub fn with_config(fonts: Arc<FontStore>, config: RasterConfig) -> Self {
        FontRasterizer { fonts, config, hinters: Mutex::new(Vec::new()) }
    }

    /// The AA mode DirectWrite would use for `font` at `ppem` (device pixels per em):
    /// horizontal-only where the font's `gasp` policy asks for asymmetric smoothing.
    pub fn preferred_aa(&self, font: FontId, ppem: f32) -> AaMode {
        let Some(data) = self.fonts.face(font) else { return AaMode::Grayscale };
        let Ok(f) = FontRef::from_index(data.data.data(), data.index) else { return AaMode::Grayscale };
        if raster_policy(&f, ppem.round().clamp(0.0, u16::MAX as f32) as u16).symmetric_smoothing {
            AaMode::Grayscale
        } else {
            AaMode::HorizontalOnly
        }
    }

    fn hinter(&self, key: HintKey, font: &FontRef<'_>, ppem: f32, coords: &[NormalizedCoord]) -> Option<Arc<HintingInstance>> {
        let mut cache = self.hinters.lock().unwrap();
        if let Some(i) = cache.iter().position(|(k, _)| *k == key) {
            let entry = cache.remove(i);
            let h = entry.1.clone();
            cache.push(entry);
            return Some(h);
        }
        let outlines = font.outline_glyphs();
        let options = HintingOptions {
            engine: Engine::AutoFallback,
            target: Target::Smooth { mode: SmoothMode::Light, symmetric_rendering: false, preserve_linear_metrics: true },
        };
        let h = Arc::new(HintingInstance::new(&outlines, Size::new(ppem), LocationRef::new(coords), options).ok()?);
        if cache.len() >= HINT_CACHE {
            cache.remove(0);
        }
        cache.push((key, h.clone()));
        Some(h)
    }

    /// The glyph's outline in device pixels (y down, pen at the origin plus the subpixel offset).
    pub(crate) fn outline(&self, key: &GlyphKey, font: &FontRef<'_>, coords: &[NormalizedCoord]) -> Option<Vec<Cmd>> {
        let ppem = key.ppem_26_6 as f32 / 64.0;
        let glyph = font.outline_glyphs().get(GlyphId::new(key.glyph))?;
        let mut pen = DevicePen { cmds: Vec::new(), dx: key.x_bin.min(3) as f32 * 0.25 };
        let hinter = match key.hint {
            HintMode::Vertical => self.hinter((key.font, key.ppem_26_6, key.variations), font, ppem, coords),
            HintMode::None => None,
        };
        let hinted = hinter.as_ref().is_some_and(|h| glyph.draw(DrawSettings::hinted(h, false), &mut pen).is_ok());
        if !hinted {
            pen.cmds.clear();
            glyph.draw(DrawSettings::unhinted(Size::new(ppem), LocationRef::new(coords)), &mut pen).ok()?;
        }
        Some(pen.cmds)
    }

    fn rasterize_outline(&self, key: &GlyphKey, font: &FontRef<'_>, coords: &[NormalizedCoord]) -> Option<GlyphImage> {
        let cmds = self.outline(key, font, coords)?;
        let mut polys = coverage::flatten(&cmds);
        let mut bold = key.embolden_64 as f32 / 64.0;
        if key.dilation > 0 {
            bold += self.config.dilation_px.get(key.dilation as usize - 1).copied().unwrap_or(0.0);
        }
        coverage::embolden(&mut polys, bold);
        coverage::skew(&mut polys, key.skew_deg as f32);
        let (x0, y0, x1, y1) = coverage::bounds(&polys)?;
        let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let mut acc = Accumulator::new(w, h, x0, y0);
        acc.fill(&polys, key.aa == AaMode::HorizontalOnly);
        Some(GlyphImage { left: x0, top: y0, width: w, height: h, format: GlyphFormat::Mask, data: acc.to_mask() })
    }
}

impl GlyphRasterizer for FontRasterizer {
    fn rasterize(&self, key: &GlyphKey) -> Option<GlyphImage> {
        let data = self.fonts.face(key.font)?;
        let font = FontRef::from_index(data.data.data(), data.index).ok()?;
        let coords: Vec<NormalizedCoord> =
            self.fonts.coords(key.variations).iter().map(|c| NormalizedCoord::from_bits(*c)).collect();
        if let Some(img) = color::rasterize_color(self, key, &font, &coords) {
            return Some(img);
        }
        self.rasterize_outline(key, &font, &coords)
    }
}

/// Collects skrifa's outline (y up, pixels) as device-space commands (y down).
struct DevicePen {
    cmds: Vec<Cmd>,
    dx: f32,
}

impl DevicePen {
    fn pt(&self, x: f32, y: f32) -> Pt {
        Pt::new(x + self.dx, -y)
    }
}

impl OutlinePen for DevicePen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cmds.push(Cmd::Move(p));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cmds.push(Cmd::Line(p));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.pt(cx0, cy0), self.pt(x, y));
        self.cmds.push(Cmd::Quad(c, p));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.pt(cx0, cy0), self.pt(cx1, cy1), self.pt(x, y));
        self.cmds.push(Cmd::Cubic(c0, c1, p));
    }
    fn close(&mut self) {
        self.cmds.push(Cmd::Close);
    }
}
