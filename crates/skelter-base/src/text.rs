//! The text boundaries: shaped glyph runs (made by `skelter-text`, drawn by `skelter-render`), the glyph
//! rasteriser the renderer calls, and the measurement interface layout calls.

use crate::geom::Size;
use crate::ids::FontId;

/// One positioned glyph, relative to its run's origin (points; y down, baseline at 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum HintMode {
    /// No hinting.
    None,
    /// Vertical-only hinting, unhinted advances (the default, architecture §9.1).
    #[default]
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AaMode {
    /// Grayscale coverage in both axes.
    #[default]
    Grayscale,
    /// Antialias horizontally only (DirectWrite NATURAL-style, skrifa `raster_policy`).
    HorizontalOnly,
    /// LCD subpixel (later, gated: architecture §9.4).
    Lcd,
}

/// A run of glyphs in one font at one size, with its metrics. Shaped once; drawn many times.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontId,
    /// Font size in points.
    pub size: f32,
    /// Variation coordinates hash (0 = default instance).
    pub variations: u64,
    pub glyphs: Vec<Glyph>,
    pub ascent: f32,
    pub descent: f32,
    pub advance: f32,
    pub hint: HintMode,
    pub aa: AaMode,
    /// Synthetic emboldening in points (0 = none) and skew in degrees (0 = none).
    pub embolden: f32,
    pub skew_deg: f32,
}

/// The key of one rasterised glyph image (architecture §9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font: FontId,
    pub variations: u64,
    pub glyph: u32,
    /// Pixels per em in 26.6 fixed point (device pixels).
    pub ppem_26_6: u32,
    /// Horizontal subpixel bin, 0..4 (4×1 positions).
    pub x_bin: u8,
    pub hint: HintMode,
    pub aa: AaMode,
    /// Dilation level 0..=4 (macOS-style colour-dependent thickening; 0 = none).
    pub dilation: u8,
    /// Synthetic emboldening in 1/64 px and skew in whole degrees.
    pub embolden_64: u16,
    pub skew_deg: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphFormat {
    /// 8-bit linear coverage.
    Mask,
    /// Premultiplied RGBA8 (colour emoji, COLR).
    Color,
}

/// A rasterised glyph: `left`/`top` place the bitmap relative to the pen position on the baseline
/// (device pixels, y down: `top` is negative above the baseline).
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphImage {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub format: GlyphFormat,
    pub data: Vec<u8>,
}

/// Implemented by `skelter-text`; called by the renderer on the compositor thread when a glyph is not
/// in the atlas yet.
pub trait GlyphRasterizer: Send + Sync {
    /// `None` for an empty glyph (space) or a glyph the font lacks.
    fn rasterize(&self, key: &GlyphKey) -> Option<GlyphImage>;
}

/// What layout needs from text: intrinsic sizes of a styled string (architecture §5).
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpec<'a> {
    pub text: &'a str,
    /// CSS `font-family` list, e.g. `"Inter, system-ui, sans-serif"`.
    pub family: &'a str,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    /// CSS `line-height` as a multiple of the size (`None` = the font's normal).
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
    pub wrap: bool,
}

pub trait TextMeasure {
    /// Size of `spec` laid out at `max_width` (`None` = no wrapping width).
    fn measure(&mut self, spec: &TextSpec<'_>, max_width: Option<f32>) -> Size;
    /// (min-content, max-content) widths.
    fn content_widths(&mut self, spec: &TextSpec<'_>) -> (f32, f32);
}

/// Inline direction (CSS `direction`, HTML `dir`): where a line, a `row` and the inline start are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

impl Direction {
    pub fn is_rtl(self) -> bool {
        self == Direction::Rtl
    }
    /// `ltr` / `rtl`.
    pub fn name(self) -> &'static str {
        match self {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        }
    }
    /// The other direction.
    pub fn flipped(self) -> Direction {
        match self {
            Direction::Ltr => Direction::Rtl,
            Direction::Rtl => Direction::Ltr,
        }
    }
}

/// The direction of `text`'s first strong character (Unicode bidi classes L, R and AL), as HTML's
/// `dir=auto` resolves it; `None` when it has none (digits, punctuation and marks are not strong).
/// The R/AL ranges are the right-to-left scripts' blocks without their marks and Arabic digits.
pub fn first_strong(text: &str) -> Option<Direction> {
    text.chars().find_map(strong)
}

/// A character's strong direction, if it has one.
pub fn strong(c: char) -> Option<Direction> {
    let u = c as u32;
    if u < 0x80 {
        return c.is_ascii_alphabetic().then_some(Direction::Ltr);
    }
    let rtl_block = matches!(u, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF);
    if rtl_block {
        // not strong: Hebrew points, Arabic marks, Arabic-Indic digits and number signs
        let weak = matches!(
            u,
            0x0591..=0x05BD
                | 0x05BF
                | 0x05C1..=0x05C2
                | 0x05C4..=0x05C5
                | 0x05C7
                | 0x0600..=0x0605
                | 0x0610..=0x061A
                | 0x064B..=0x066C
                | 0x0670
                | 0x06D6..=0x06ED
                | 0x06F0..=0x06F9
                | 0x0711
                | 0x0730..=0x074A
                | 0x07A6..=0x07B0
                | 0x07EB..=0x07F3
                | 0x0816..=0x082D
                | 0x0859..=0x085B
                | 0x0890..=0x089F
                | 0x08CA..=0x08FF
                | 0xFB1E
                | 0xFE00..=0xFE0F
                | 0xFE20..=0xFE2F
        );
        return (!weak).then_some(Direction::Rtl);
    }
    c.is_alphabetic().then_some(Direction::Ltr)
}
