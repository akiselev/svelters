//! Exact CPU-side goldens (architecture §9.6): the shaped glyph runs of a small corpus and digests of
//! their rasterised masks. `UPDATE=1` rewrites `goldens/*.txt`; review the diff like any other code.

use super::{rasterizer, system};
use crate::{RichText, Span, TextStyle};
use presto_base::text::{AaMode, GlyphKey, GlyphRasterizer, HintMode};
use std::fmt::Write;
use std::path::PathBuf;

fn corpus() -> Vec<(&'static str, RichText, Option<f32>)> {
    let st = |size: f32| TextStyle { size, ..TextStyle::default() };
    let mut mixed = RichText::plain("Bold and plain", st(13.0));
    mixed.spans.push(Span { range: 0..4, style: TextStyle { weight: 700, paint: 1, ..st(13.0) } });
    let mut ell = RichText::plain("Ellipsis cuts this label", TextStyle { wrap: false, ..st(12.0) });
    ell.ellipsis = true;
    vec![
        ("latin", RichText::plain("The quick brown fox", st(13.0)), None),
        ("kerning", RichText::plain("AVATAR To Ty", st(16.0)), None),
        ("ligature", RichText::plain("office fi fl", st(13.0)), None),
        ("wrapped", RichText::plain("one two three four", st(13.0)), Some(50.0)),
        ("hebrew", RichText::plain("\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd}", st(13.0)), None),
        ("bidi", RichText::plain("abc \u{5e9}\u{5dc}\u{5d5}\u{5dd} def", st(13.0)), None),
        ("spans", mixed, None),
        ("ellipsis", ell, Some(90.0)),
        ("emoji", RichText::plain("ok \u{1F44D}", st(13.0)), None),
    ]
}

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/goldens")
}

fn check(name: &str, actual: &str) {
    let path = goldens_dir().join(name);
    if std::env::var_os("UPDATE").is_some() {
        std::fs::create_dir_all(goldens_dir()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {}; run with UPDATE=1", path.display()));
    if expected != actual {
        let first = expected.lines().zip(actual.lines()).position(|(a, b)| a != b);
        panic!("golden {name} differs (first differing line {first:?}); run with UPDATE=1 and review\n--- actual\n{actual}");
    }
}

/// FNV-1a over the mask bytes.
fn digest(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[test]
fn glyph_runs_golden() {
    let mut ts = system();
    let mut out = String::new();
    for (name, text, width) in corpus() {
        let l = ts.layout(&text, width);
        writeln!(out, "# {name} size={:.3}x{:.3} lines={}", l.size().w, l.size().h, l.lines().len()).unwrap();
        for r in l.runs() {
            write!(
                out,
                "run font={} size={} origin=({:.3},{:.3}) style={} range={:?} adv={:.3}:",
                r.run.font.0, r.run.size, r.origin.x, r.origin.y, r.style, r.range, r.run.advance
            )
            .unwrap();
            for g in &r.run.glyphs {
                write!(out, " {}@{:.3},{:.3}", g.id, g.x, g.y).unwrap();
            }
            out.push('\n');
        }
    }
    check("runs.txt", &out);
}

#[test]
fn glyph_masks_golden() {
    let mut ts = system();
    let r = rasterizer(&ts);
    let mut out = String::new();
    for (name, text, width) in corpus() {
        let l = ts.layout(&text, width);
        for (aa, x_bin) in [(AaMode::Grayscale, 0u8), (AaMode::Grayscale, 2), (AaMode::HorizontalOnly, 1)] {
            write!(out, "{name} {aa:?} bin{x_bin}:").unwrap();
            for run in l.runs() {
                for g in &run.run.glyphs {
                    let key = GlyphKey {
                        font: run.run.font,
                        variations: run.run.variations,
                        glyph: g.id,
                        ppem_26_6: (run.run.size * 64.0) as u32,
                        x_bin,
                        hint: HintMode::Vertical,
                        aa,
                        dilation: 0,
                        embolden_64: 0,
                        skew_deg: 0,
                    };
                    match r.rasterize(&key) {
                        Some(img) => write!(
                            out,
                            " {}:{}x{}@{},{}#{:016x}",
                            g.id,
                            img.width,
                            img.height,
                            img.left,
                            img.top,
                            digest(&img.data)
                        )
                        .unwrap(),
                        None => write!(out, " {}:empty", g.id).unwrap(),
                    }
                }
            }
            out.push('\n');
        }
    }
    check("masks.txt", &out);
}
