//! presto's text stack (`plans/research/presto/architecture.md` §9).
//!
//! | Module | What |
//! |---|---|
//! | [`fonts`] | [`FontStore`]: the process's font faces (fontique), face and variation ids, the font-set generation |
//! | [`style`] | [`RichText`], [`Span`], [`TextStyle`]: the input; paint ids are kept out of shaping |
//! | [`system`] | [`TextSystem`]: shaping (parley), breaking, placement, ellipsis, tab stops, caches; the [`TextMeasure`](presto_base::text::TextMeasure) impl |
//! | [`layout`] | [`TextLayout`]: placed glyph runs and the hit-test, caret and selection geometry |
//! | [`raster`] | [`FontRasterizer`]: the [`GlyphRasterizer`](presto_base::text::GlyphRasterizer) impl (skrifa outlines, our coverage rasteriser, colour glyphs) |
//! | [`test_fonts`] | the bundled Noto subsets for deterministic tests |
//!
//! Threads: a [`FontStore`] is shared (`Arc`) by the UI thread's [`TextSystem`] and the
//! compositor thread's [`FontRasterizer`]; shaped runs cross as `Arc<GlyphRun>` naming faces by
//! [`FontId`](presto_base::ids::FontId).

pub mod fonts;
pub mod layout;
pub mod raster;
pub mod style;
pub mod system;
pub mod test_fonts;

pub use fonts::{FontConfig, FontStore};
pub use layout::{Affinity, Cursor, LineInfo, PlacedRun, TextLayout};
pub use raster::{FontRasterizer, RasterConfig, RasterPolicy};
pub use style::{Align, RichText, Span, TextStyle};
pub use system::{TextStats, TextSystem};

#[cfg(test)]
mod tests;
