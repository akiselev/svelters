//! presto-text tests. All run on the bundled fonts with system fonts off ([`FontConfig::testing`]).

mod coverage;
mod geometry;
mod golden;
mod layout;
mod raster;

use crate::{FontConfig, FontRasterizer, FontStore, RichText, TextLayout, TextStyle, TextSystem};
use presto_base::text::{AaMode, GlyphKey, HintMode};
use std::sync::Arc;

pub(crate) fn fonts() -> Arc<FontStore> {
    FontStore::new(FontConfig::testing())
}

pub(crate) fn system() -> TextSystem {
    TextSystem::new(fonts())
}

pub(crate) fn style(size: f32) -> TextStyle {
    TextStyle { size, ..TextStyle::default() }
}

pub(crate) fn plain(text: &str, size: f32) -> RichText {
    RichText::plain(text, style(size))
}

pub(crate) fn lay(ts: &mut TextSystem, text: &str, size: f32, max_width: Option<f32>) -> Arc<TextLayout> {
    ts.layout(&plain(text, size), max_width)
}

/// The key of the first glyph of `text` laid out at `size` pt, rasterised at `ppem` px.
pub(crate) fn first_key(ts: &mut TextSystem, text: &str, ppem: f32) -> GlyphKey {
    let l = lay(ts, text, 13.0, None);
    let run = &l.runs()[0].run;
    GlyphKey {
        font: run.font,
        variations: run.variations,
        glyph: run.glyphs[0].id,
        ppem_26_6: (ppem * 64.0) as u32,
        x_bin: 0,
        hint: HintMode::Vertical,
        aa: AaMode::Grayscale,
        dilation: 0,
        embolden_64: 0,
        skew_deg: 0,
    }
}

pub(crate) fn rasterizer(ts: &TextSystem) -> FontRasterizer {
    FontRasterizer::new(ts.fonts().clone())
}
