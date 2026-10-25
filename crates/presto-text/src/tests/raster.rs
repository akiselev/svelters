//! FontRasterizer: determinism, AA modes, subpixel bins, synthesis, colour glyphs, the gasp policy.

use super::{first_key, lay, rasterizer, system};
use crate::raster::raster_policy;
use crate::test_fonts;
use presto_base::text::{AaMode, GlyphFormat, GlyphRasterizer, HintMode};
use skrifa::FontRef;

#[test]
fn masks_are_byte_identical_across_rasterizers() {
    let mut ts = system();
    let key = first_key(&mut ts, "g", 13.0);
    let a = rasterizer(&ts).rasterize(&key).expect("g has an outline");
    let b = rasterizer(&ts).rasterize(&key).expect("g has an outline");
    assert_eq!(a, b);
    assert_eq!(a.format, GlyphFormat::Mask);
    assert_eq!(a.data.len(), (a.width * a.height) as usize);
    assert!(a.top < 0 && a.top + a.height as i32 > 0, "g descends below the baseline: {a:?}");
    assert!(a.data.contains(&255) || a.data.iter().any(|v| *v > 200));
}

#[test]
fn horizontal_only_aa_differs_from_grayscale() {
    let mut ts = system();
    let mut key = first_key(&mut ts, "o", 13.0);
    let r = rasterizer(&ts);
    let gray = r.rasterize(&key).unwrap();
    key.aa = AaMode::HorizontalOnly;
    let xonly = r.rasterize(&key).unwrap();
    assert_ne!(gray.data, xonly.data);
    // The top and bottom of an "o" are horizontal edges: grayscale shades them, x-only does not
    // produce more partially covered pixels than grayscale.
    let partial = |d: &[u8]| d.iter().filter(|v| **v > 0 && **v < 255).count();
    assert!(partial(&xonly.data) < partial(&gray.data), "{} vs {}", partial(&xonly.data), partial(&gray.data));
}

#[test]
fn subpixel_bins_shift_coverage() {
    let mut ts = system();
    let mut key = first_key(&mut ts, "l", 13.0);
    let r = rasterizer(&ts);
    let images: Vec<_> = (0..4)
        .map(|b| {
            key.x_bin = b;
            r.rasterize(&key).unwrap()
        })
        .collect();
    for w in images.windows(2) {
        assert_ne!(w[0].data, w[1].data);
    }
}

#[test]
fn hinting_changes_the_outline() {
    let mut ts = system();
    let mut key = first_key(&mut ts, "e", 12.0);
    let r = rasterizer(&ts);
    let hinted = r.rasterize(&key).unwrap();
    key.hint = HintMode::None;
    let unhinted = r.rasterize(&key).unwrap();
    assert_ne!(hinted, unhinted);
}

#[test]
fn space_is_empty() {
    let mut ts = system();
    let l = lay(&mut ts, "a b", 13.0, None);
    let run = &l.runs()[0].run;
    let mut key = first_key(&mut ts, "a", 13.0);
    key.glyph = run.glyphs[1].id;
    assert!(rasterizer(&ts).rasterize(&key).is_none());
}

#[test]
fn embolden_and_dilation_thicken() {
    let mut ts = system();
    let mut key = first_key(&mut ts, "l", 16.0);
    let fonts = ts.fonts().clone();
    let plain = crate::FontRasterizer::new(fonts.clone()).rasterize(&key).unwrap();
    key.embolden_64 = 64;
    let bold = crate::FontRasterizer::new(fonts.clone()).rasterize(&key).unwrap();
    let ink = |d: &[u8]| d.iter().map(|v| *v as u32).sum::<u32>();
    assert!(ink(&bold.data) > ink(&plain.data));
    // Dilation without a configured table draws undilated; with one it thickens.
    key.embolden_64 = 0;
    key.dilation = 2;
    assert_eq!(crate::FontRasterizer::new(fonts.clone()).rasterize(&key).unwrap(), plain);
    let cfg = crate::RasterConfig { dilation_px: vec![0.25, 0.5, 0.75, 1.0] };
    let dilated = crate::FontRasterizer::with_config(fonts, cfg).rasterize(&key).unwrap();
    assert!(ink(&dilated.data) > ink(&plain.data));
}

#[test]
fn colr_emoji_rasterises_in_colour() {
    let mut ts = system();
    let l = lay(&mut ts, "\u{1F600}", 13.0, None);
    assert_eq!(l.runs().len(), 1, "{:?}", l.runs());
    let mut key = first_key(&mut ts, "\u{1F600}", 32.0);
    key.glyph = l.runs()[0].run.glyphs[0].id;
    let img = rasterizer(&ts).rasterize(&key).expect("emoji paints");
    assert_eq!(img.format, GlyphFormat::Color);
    assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    assert!(img.width >= 24 && img.height >= 24, "{}x{}", img.width, img.height);
    // Premultiplied: no channel exceeds alpha; some pixels are opaque and coloured.
    assert!(img.data.chunks_exact(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
    assert!(img.data.chunks_exact(4).any(|p| p[3] == 255 && (p[0] != p[1] || p[1] != p[2])));
    let again = rasterizer(&ts).rasterize(&key).unwrap();
    assert_eq!(img, again);
}

#[test]
fn gasp_policy_follows_the_table() {
    // Noto Sans has a version 1 gasp table with one range asking for symmetric smoothing at every size.
    let font = FontRef::new(test_fonts::SANS_REGULAR).unwrap();
    for ppem in [9, 12, 13, 20, 40] {
        let p = raster_policy(&font, ppem);
        assert!(p.symmetric_smoothing && p.grid_fit, "{ppem}: {p:?}");
    }
    let mut ts = system();
    let key = first_key(&mut ts, "a", 13.0);
    assert_eq!(rasterizer(&ts).preferred_aa(key.font, 13.0), AaMode::Grayscale);
}
