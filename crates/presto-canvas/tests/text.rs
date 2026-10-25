//! Canvas labels with a real font (presto-text's bundled Noto Sans subset): layout, Slug coverage of
//! real outlines, and label level of detail.

use presto_base::color::Color;
use presto_canvas::slug::coverage;
use presto_canvas::{CanvasFonts, ChunkBuilder, DVec2, GlyphKey, Label};
use std::sync::Arc;

const NOTO: &[u8] = include_bytes!("../../presto-text/fonts/NotoSans-Regular.subset.ttf");

fn fonts() -> (Arc<CanvasFonts>, presto_canvas::FontKey) {
    let f = CanvasFonts::new();
    let key = f.register(Arc::from(NOTO), 0).expect("a font");
    (f, key)
}

fn glyph(fonts: &CanvasFonts, font: presto_canvas::FontKey, ch: char) -> GlyphKey {
    let line = fonts.layout(font, &ch.to_string()).expect("layout");
    line.glyphs[0].key
}

#[test]
fn layout_advances_along_the_line() {
    let (f, key) = fonts();
    let line = f.layout(key, "R12").unwrap();
    assert_eq!(line.glyphs.len(), 3);
    assert_eq!(line.glyphs[0].x, 0.0);
    assert!(line.glyphs[1].x > 0.3 && line.glyphs[2].x > line.glyphs[1].x);
    assert!(line.ascent > 0.5 && line.descent > 0.0);
}

#[test]
fn real_outlines_are_covered_inside_and_open_in_counters() {
    let (f, key) = fonts();
    let ppem = [64.0, 64.0];
    let l = f.glyph(glyph(&f, key, 'l')).unwrap();
    let mid = [(l.bbox[0] + l.bbox[2]) * 0.5, (l.bbox[1] + l.bbox[3]) * 0.5];
    assert_eq!(coverage(&l, mid, ppem), 1.0, "the stem of l");
    assert_eq!(coverage(&l, [l.bbox[2] + 0.2, mid[1]], ppem), 0.0, "right of l");

    let o = f.glyph(glyph(&f, key, 'o')).unwrap();
    let c = [(o.bbox[0] + o.bbox[2]) * 0.5, (o.bbox[1] + o.bbox[3]) * 0.5];
    assert_eq!(coverage(&o, c, ppem), 0.0, "the counter of o");
    assert_eq!(coverage(&o, [o.bbox[0] + 0.02, c[1]], ppem), 1.0, "the left bowl of o");

    let space = f.glyph(glyph(&f, key, ' ')).unwrap();
    assert!(space.is_empty());
}

#[test]
fn labels_split_into_glyphs_bars_and_nothing_by_size() {
    let (f, key) = fonts();
    let mut b = ChunkBuilder::new(DVec2::ZERO);
    for (i, size) in [10.0, 1.0, 0.1].into_iter().enumerate() {
        b.label(
            &f,
            &Label { text: "U1", font: key, pos: DVec2::new(0.0, i as f64 * 20.0), size, rotation: 0.0, color: Color::BLACK, align: 0.0 },
        );
    }
    let c = b.build();
    assert_eq!(c.labels.len(), 3);
    assert_eq!(c.glyphs.len(), 6);
    // At 1 device pixel per world unit: the 10-unit label as glyphs, the 1-unit one... too small for a bar
    // (1.5 px), the 0.1 one hidden. At 2 px per unit: 10 → glyphs, 1 → bar (2 px), 0.1 → hidden.
    let at = |ppu: f32| c.label_lod(6.0 / ppu, 1.5 / ppu);
    let one = at(1.0);
    assert_eq!((one.glyphs, one.greek_start, one.greek_end), (2, 1, 1));
    let two = at(2.0);
    assert_eq!((two.glyphs, two.greek_start, two.greek_end), (2, 1, 2));
    assert_eq!(c.greek.len(), 3);
}
