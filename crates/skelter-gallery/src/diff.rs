//! `--diff <before.png>`: a picture of what changed between a stored image and a new one, for
//! looking only (contact sheets are not goldens; nothing fails on a difference).
//!
//! The diff image is three panels side by side on a grey ground: **before**, **after**, and the
//! **changes**: the after image faded to grey, every pixel that differs painted magenta, and the
//! changed region framed in red. Images of different sizes are compared over the larger size; a
//! pixel only one of them has counts as changed.

use skelter_test::Image;
use std::path::Path;

/// The gap between panels and around them, in pixels.
const GAP: u32 = 8;
const GROUND: [u8; 4] = [96, 96, 96, 255];
const CHANGED: [u8; 4] = [255, 0, 255, 255];
const FRAME: [u8; 4] = [255, 32, 32, 255];

/// What [`diff`] found.
#[derive(Clone, Debug)]
pub struct Diff {
    /// The three-panel picture (opaque, so premultiplied and straight alpha agree).
    pub image: Image,
    /// Pixels that differ (any channel), over the larger of the two sizes.
    pub changed: u64,
    pub total: u64,
    /// The smallest rectangle holding every changed pixel: `[x, y, w, h]` in the images' pixels.
    pub bounds: Option<[u32; 4]>,
    pub before_size: [u32; 2],
    pub after_size: [u32; 2],
}

/// Reads a PNG as straight RGBA8.
pub fn load_png(path: &Path) -> Result<Image, String> {
    let img = image::ImageReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let img = img.with_guessed_format().map_err(|e| format!("{}: {e}", path.display()))?.decode().map_err(|e| format!("{}: {e}", path.display()))?.into_rgba8();
    Ok(Image { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

/// `image` (premultiplied, as the renderer gives it) with straight alpha, as its PNG holds it.
pub fn straight(image: &Image) -> Image {
    let mut rgba = image.rgba.clone();
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
            }
        }
    }
    Image { width: image.width, height: image.height, rgba }
}

fn px(img: &Image, x: u32, y: u32) -> Option<[u8; 4]> {
    (x < img.width && y < img.height).then(|| {
        let i = ((y * img.width + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
    })
}

/// Over `GROUND`, opaque.
fn flat(p: [u8; 4]) -> [u8; 4] {
    let a = u32::from(p[3]);
    let mix = |c: u8, g: u8| ((u32::from(c) * a + u32::from(g) * (255 - a) + 127) / 255) as u8;
    [mix(p[0], GROUND[0]), mix(p[1], GROUND[1]), mix(p[2], GROUND[2]), 255]
}

/// Compares `before` and `after` (both straight RGBA).
pub fn diff(before: &Image, after: &Image) -> Diff {
    let (w, h) = (before.width.max(after.width), before.height.max(after.height));
    let mut out = Image { width: GAP * 4 + w * 3, height: GAP * 2 + h, rgba: GROUND.repeat(((GAP * 4 + w * 3) * (GAP * 2 + h)) as usize) };
    let mut put = |out: &mut Image, x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * out.width + x) * 4) as usize;
        out.rgba[i..i + 4].copy_from_slice(&c);
    };
    let (mut changed, mut lo, mut hi) = (0u64, (u32::MAX, u32::MAX), (0u32, 0u32));
    for y in 0..h {
        for x in 0..w {
            let (b, a) = (px(before, x, y), px(after, x, y));
            if let Some(b) = b {
                put(&mut out, GAP + x, GAP + y, flat(b));
            }
            if let Some(a) = a {
                put(&mut out, GAP * 2 + w + x, GAP + y, flat(a));
            }
            let differs = b != a;
            let c = if differs {
                changed += 1;
                lo = (lo.0.min(x), lo.1.min(y));
                hi = (hi.0.max(x), hi.1.max(y));
                CHANGED
            } else {
                // faded to grey, so the magenta reads on any theme
                let f = flat(a.unwrap_or(GROUND));
                let l = ((u32::from(f[0]) * 3 + u32::from(f[1]) * 6 + u32::from(f[2])) / 10) as u8;
                let g = (u32::from(l) / 3 + 120) as u8;
                [g, g, g, 255]
            };
            put(&mut out, GAP * 3 + w * 2 + x, GAP + y, c);
        }
    }
    let bounds = (changed > 0).then(|| [lo.0, lo.1, hi.0 - lo.0 + 1, hi.1 - lo.1 + 1]);
    if let Some([bx, by, bw, bh]) = bounds {
        // a frame two pixels outside the changed region (clipped to the panel)
        let (x0, y0) = (bx.saturating_sub(2), by.saturating_sub(2));
        let (x1, y1) = ((bx + bw + 1).min(w - 1), (by + bh + 1).min(h - 1));
        let ox = GAP * 3 + w * 2;
        for x in x0..=x1 {
            for y in [y0, y1] {
                put(&mut out, ox + x, GAP + y, FRAME);
            }
        }
        for y in y0..=y1 {
            for x in [x0, x1] {
                put(&mut out, ox + x, GAP + y, FRAME);
            }
        }
    }
    Diff { image: out, changed, total: u64::from(w) * u64::from(h), bounds, before_size: [before.width, before.height], after_size: [after.width, after.height] }
}

/// The diff image's path for an output `out`: `<out stem>.diff.png` beside it.
pub fn diff_path(out: &Path) -> std::path::PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    out.with_file_name(format!("{stem}.diff.png"))
}

impl Diff {
    /// `"312 of 30720 pixels changed, in 8,8 40x16"` (or `"no change"`).
    pub fn summary(&self) -> String {
        let size = if self.before_size != self.after_size {
            format!(" (size {}x{} → {}x{})", self.before_size[0], self.before_size[1], self.after_size[0], self.after_size[1])
        } else {
            String::new()
        };
        match self.bounds {
            None => format!("no change{size}"),
            Some([x, y, w, h]) => format!("{} of {} pixels changed, in {x},{y} {w}x{h}{size}", self.changed, self.total),
        }
    }

    pub fn json(&self, file: &Path) -> serde_json::Value {
        serde_json::json!({ "file": file, "changed": self.changed, "total": self.total, "bounds": self.bounds,
                            "before_size": self.before_size, "after_size": self.after_size })
    }
}
