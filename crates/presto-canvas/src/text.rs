//! Canvas fonts: the font files labels are set in, their simple line layout (character map and
//! advances, no shaping) and the cache of their glyphs as [`SlugGlyph`]s, shared by the threads that
//! build chunks and the renderer that uploads the glyphs.
//!
//! Stroke fonts (KiCad's newstroke) are not fonts here: they are segments a chunk draws.

use crate::slug::{OutlineBuilder, SlugGlyph};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A font registered with [`CanvasFonts::register`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontKey(pub u32);

/// One glyph of one canvas font.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyphKey {
    pub font: FontKey,
    pub glyph: u32,
}

/// A laid-out line in ems.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub glyphs: Vec<PlacedGlyph>,
    /// Total advance in ems.
    pub advance: f64,
    /// Ascent and descent (positive) in ems.
    pub ascent: f64,
    pub descent: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub key: GlyphKey,
    /// Pen position in ems from the line start.
    pub x: f64,
}

struct Font {
    data: Arc<[u8]>,
    index: u32,
    upem: f32,
    ascent: f64,
    descent: f64,
}

/// The fonts canvases use. `Send + Sync`, shared as `Arc`.
#[derive(Default)]
pub struct CanvasFonts {
    fonts: Mutex<Vec<Font>>,
    glyphs: Mutex<HashMap<GlyphKey, Arc<SlugGlyph>>>,
}

impl std::fmt::Debug for CanvasFonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.fonts.lock().map(|v| v.len()).unwrap_or(0);
        f.debug_struct("CanvasFonts").field("fonts", &n).finish()
    }
}

impl CanvasFonts {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Registers face `index` of a font file (TrueType, OpenType, a collection); `None` when the data
    /// is not a font.
    pub fn register(&self, data: Arc<[u8]>, index: u32) -> Option<FontKey> {
        let font = FontRef::from_index(&data, index).ok()?;
        let upem = font.head().ok()?.units_per_em() as f32;
        let m = font.metrics(Size::unscaled(), LocationRef::default());
        let (ascent, descent) = (m.ascent as f64 / upem as f64, -m.descent as f64 / upem as f64);
        let mut fonts = self.fonts.lock().unwrap_or_else(|e| e.into_inner());
        fonts.push(Font { data: data.clone(), index, upem, ascent, descent });
        Some(FontKey(fonts.len() as u32 - 1))
    }

    /// Lays `text` out on one line: each character through the font's character map, advanced by its
    /// horizontal metric. `None` for an unknown font.
    pub fn layout(&self, key: FontKey, text: &str) -> Option<Line> {
        let fonts = self.fonts.lock().unwrap_or_else(|e| e.into_inner());
        let f = fonts.get(key.0 as usize)?;
        let font = FontRef::from_index(&f.data, f.index).ok()?;
        let charmap = font.charmap();
        let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let scale = 1.0 / f.upem as f64;
        let mut line = Line { ascent: f.ascent, descent: f.descent, ..Default::default() };
        for ch in text.chars() {
            let Some(gid) = charmap.map(ch) else { continue };
            line.glyphs.push(PlacedGlyph { key: GlyphKey { font: key, glyph: gid.to_u32() }, x: line.advance });
            line.advance += metrics.advance_width(gid).unwrap_or(0.0) as f64 * scale;
        }
        Some(line)
    }

    /// The glyph's outline as Slug curves and bands, built once.
    pub fn glyph(&self, key: GlyphKey) -> Option<Arc<SlugGlyph>> {
        if let Some(g) = self.glyphs.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return Some(g.clone());
        }
        let g = {
            let fonts = self.fonts.lock().unwrap_or_else(|e| e.into_inner());
            let f = fonts.get(key.font.0 as usize)?;
            let font = FontRef::from_index(&f.data, f.index).ok()?;
            let mut pen = Pen(OutlineBuilder::new(f.upem));
            if let Some(outline) = font.outline_glyphs().get(GlyphId::new(key.glyph)) {
                // A malformed outline draws what it produced before the error.
                let _ = outline.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen);
            }
            Arc::new(pen.0.finish())
        };
        self.glyphs.lock().unwrap_or_else(|e| e.into_inner()).insert(key, g.clone());
        Some(g)
    }
}

struct Pen(OutlineBuilder);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}
