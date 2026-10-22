//! IC1 (plans/milestones/IC1-icon-toolchain.md): icon SVGs through egui_extras' `SvgLoader` (resvg 0.45),
//! compared with committed PNGs, so a loader, resvg or usvg-options change shows up here.
//!
//! The SVGs in `tests/icons/build/` are `tools/icons/icons.py build` output for a few of IC0's style samples
//! (`tools/icons/samples`, plans/research/ICON-STYLE.md). They cover the subject tone and the solid arrowhead
//! (`mcad.extrude`, also at 12, 14 and 20 with its 1.25px stroke), a clip-path hatch (`mcad.section`), the
//! results fringe in a clip-path (`sim.result`), a composed badge whose mask clears the corner cell
//! (`mcad.extrude+remove`), and an `accent` part with a knocked-out mark (`chrome.warning`).
//! `tools/icons/test_icons.py` checks that they match the tool's current output.
//!
//! The PNGs in `tests/icons/png/` are this test's own renders of the white (tint-ready) copies.
//! Regenerate them with `UPDATE=1`, look at them, and commit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::load::{ImagePoll, SizeHint};
use egui::{Color32, ColorImage};

/// (id, master, pixels): the optical sizes at 1x, and 16@2x.
const CASES: &[(&str, u32, u32)] = &[
    ("mcad.extrude", 12, 12),
    ("mcad.extrude", 14, 14),
    ("mcad.extrude", 16, 16),
    ("mcad.extrude", 16, 32),
    ("mcad.extrude", 20, 20),
    ("mcad.extrude", 24, 24),
    ("mcad.section", 16, 16),
    ("mcad.section", 24, 24),
    ("sim.result", 16, 16),
    ("sim.result", 24, 24),
    ("mcad.extrude+remove", 16, 16),
    ("mcad.extrude+remove", 16, 32),
    ("mcad.extrude+remove", 24, 24),
    ("chrome.warning", 12, 12),
    ("chrome.warning", 16, 16),
];

/// Per-channel tolerance (premultiplied): rounding through the PNG's unmultiplied RGBA.
const TOLERANCE: u8 = 2;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/icons")
}

fn read_svg(sub: &str, id: &str, master: u32) -> Vec<u8> {
    let p = dir().join("build").join(sub).join(master.to_string()).join(format!("{id}.svg"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    ctx
}

/// Rasterise through the loader exactly as `egui::Image` does: bytes:// URI, `SizeHint::Size`.
fn load(ctx: &egui::Context, uri: String, bytes: Vec<u8>, px: u32) -> Arc<ColorImage> {
    ctx.include_bytes(uri.clone(), bytes);
    let hint = SizeHint::Size { width: px, height: px, maintain_aspect_ratio: true };
    match ctx.try_load_image(&uri, hint) {
        Ok(ImagePoll::Ready { image }) => image,
        Ok(ImagePoll::Pending { .. }) => panic!("{uri}: the SVG loader is synchronous, but it returned Pending"),
        Err(e) => panic!("{uri}: {e}"),
    }
}

fn to_png(img: &ColorImage) -> image::RgbaImage {
    let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, raw).expect("size")
}

fn compare(name: &str, got: &ColorImage) -> Result<(), String> {
    let path = dir().join("png").join(format!("{name}.png"));
    if std::env::var_os("UPDATE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        to_png(got).save(&path).unwrap();
        return Ok(());
    }
    let want = image::open(&path)
        .map_err(|e| format!("{}: {e} (run with UPDATE=1 to record it)", path.display()))?
        .to_rgba8();
    if [want.width() as usize, want.height() as usize] != got.size {
        return Err(format!("{name}: size {:?}, snapshot {}x{}", got.size, want.width(), want.height()));
    }
    let mut bad = 0;
    for (w, g) in want.pixels().zip(&got.pixels) {
        let w = Color32::from_rgba_unmultiplied(w[0], w[1], w[2], w[3]);
        if w.to_array().iter().zip(g.to_array()).any(|(a, b)| a.abs_diff(b) > TOLERANCE) {
            bad += 1;
        }
    }
    if bad > 0 {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.actual.png"));
        to_png(got).save(&out).unwrap();
        return Err(format!("{name}: {bad} pixels differ from {} (actual: {})", path.display(), out.display()));
    }
    Ok(())
}

#[test]
fn icon_svgs_render_through_egui_extras_like_the_snapshots() {
    let ctx = context();
    let mut failures = Vec::new();
    for &(id, master, px) in CASES {
        let img = load(&ctx, format!("bytes://ic1/white/{master}/{id}@{px}.svg"), read_svg("white", id, master), px);
        assert_eq!(img.size, [px as usize, px as usize], "{id}");
        // A white copy tints cleanly: every pixel is white at some coverage (premultiplied r = g = b = a).
        let off = img.pixels.iter().filter(|c| {
            let [r, g, b, a] = c.to_array();
            [r, g, b].iter().any(|v| v.abs_diff(a) > 1)
        });
        assert_eq!(off.count(), 0, "{id}@{px}: the white copy rendered non-white pixels");
        assert!(img.pixels.iter().any(|c| c.a() == 255), "{id}@{px}: no solid pixel");
        if let Err(e) = compare(&format!("{id}@{master}-{px}"), &img) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Why build writes white copies: the loader's default `usvg::Options` draw `currentColor` black, so an
/// `egui::Image::tint` of the source SVG stays black. When this starts failing, egui_extras resolves
/// `currentColor` itself and the white copies can go (ICONS.md §5.3, "Before AS1").
#[test]
fn the_loader_draws_current_color_black() {
    let ctx = context();
    let img = load(&ctx, "bytes://ic1/src/mcad.extrude.svg".into(), read_svg(".", "mcad.extrude", 16), 16);
    let inked: Vec<_> = img.pixels.iter().filter(|c| c.a() > 0).collect();
    assert!(!inked.is_empty());
    assert!(inked.iter().all(|c| c.r() == 0 && c.g() == 0 && c.b() == 0), "currentColor is no longer black");
}

/// The badge knockout is a `<mask>` that clears the bottom-right 8x8 cell (ICON-STYLE §7.2): in
/// `mcad.extrude+remove` the base's slab is inked there alone, and the composed icon keeps only the minus.
#[test]
fn the_badge_knockout_mask_clears_the_cell() {
    let ctx = context();
    let base = load(&ctx, "bytes://ic1/k/extrude.svg".into(), read_svg("white", "mcad.extrude", 16), 16);
    let composed =
        load(&ctx, "bytes://ic1/k/extrude+remove.svg".into(), read_svg("white", "mcad.extrude+remove", 16), 16);
    let at = |img: &ColorImage, x: usize, y: usize| img.pixels[y * 16 + x].a();
    // The minus glyph is the 1px line at y = 12.5: row 12. Every other row of the cell is cleared.
    let cell: Vec<(usize, usize)> =
        (8..16).flat_map(|y| (8..16).map(move |x| (x, y))).filter(|&(_, y)| y != 12).collect();
    assert!(cell.iter().any(|&(x, y)| at(&base, x, y) > 0), "the base inks the cell");
    // (x, y, alpha): a seam on the cell's edge row and column is the mask rects inheriting a stroke.
    let left: Vec<_> = cell
        .iter()
        .map(|&(x, y)| (x, y, at(&composed, x, y)))
        .filter(|&(_, _, a)| a > 0)
        .collect();
    assert!(left.is_empty(), "the knockout left base pixels in the cell (x, y, alpha): {left:?}");
    assert_eq!(at(&composed, 12, 12), 255, "the minus is drawn");
    assert_eq!(at(&composed, 7, 1), at(&base, 7, 1), "outside the cell the base is untouched");
}
