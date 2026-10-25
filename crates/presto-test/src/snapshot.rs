//! The pixel tier (architecture §16.1): compare a rendered frame with a checked-in PNG, egui_kittest's
//! `snapshot` API and rules, so tests and goldens move over unchanged.
//!
//! - The golden is `{output_path}/{name}.png`; a failing run writes `{name}.new.png` and, past the
//!   threshold, `{name}.diff.png`.
//! - The comparison is dify's (kittest's): a pixel fails when its weighted YIQ distance exceeds
//!   [`SnapshotOptions::threshold`] (anti-aliased pixels detected and forgiven); the snapshot fails when
//!   more than [`SnapshotOptions::max_failed_pixels`] pixels fail.
//! - `UPDATE_SNAPSHOTS=1` (or `true`, `yes`, `on`) rewrites failing goldens, `UPDATE_SNAPSHOTS=force`
//!   every golden; the previous one is kept as `{name}.old.png`.
//!
//! presto renders premultiplied colour; snapshots are written and compared unpremultiplied (PNG's alpha).
//! kittest's `kittest.toml` is not read: options come from [`SnapshotOptions`] and the harness builder.

use crate::render::{Image, RenderError};
use std::path::PathBuf;

/// kittest's `SnapshotOptions`, with its defaults (threshold 0.6, no failed pixels, `tests/snapshots`).
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotOptions {
    /// How far one pixel may differ before it counts as failing (dify's weighted squared YIQ distance).
    pub threshold: f32,
    /// How many pixels may fail before the snapshot fails.
    pub max_failed_pixels: usize,
    /// Where goldens live, relative to the test's working directory (the crate root under cargo).
    pub output_path: PathBuf,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        SnapshotOptions {
            threshold: 0.6,
            max_failed_pixels: 0,
            output_path: PathBuf::from("tests/snapshots"),
        }
    }
}

impl SnapshotOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }
    pub fn failed_pixel_count_threshold(mut self, n: usize) -> Self {
        self.max_failed_pixels = n;
        self
    }
    pub fn max_failed_pixels(self, n: usize) -> Self {
        self.failed_pixel_count_threshold(n)
    }
    pub fn output_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_path = path.into();
        self
    }
}

#[derive(Debug)]
pub enum SnapshotError {
    /// No golden (a new test): `{name}.new.png` was written.
    OpenSnapshot { path: PathBuf, err: String },
    SizeMismatch {
        name: String,
        expected: (u32, u32),
        actual: (u32, u32),
    },
    /// `diff` pixels failed the threshold.
    Diff {
        name: String,
        diff: i32,
        diff_path: PathBuf,
    },
    WriteSnapshot { path: PathBuf, err: String },
    Render(RenderError),
}

const HOW_TO_UPDATE: &str = "Run with `UPDATE_SNAPSHOTS=1` to update the snapshots.";

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::OpenSnapshot { path, err } => write!(
                f,
                "missing snapshot {}: {err}. {HOW_TO_UPDATE}",
                path.display()
            ),
            SnapshotError::SizeMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "{name}: the image is {actual:?}, the snapshot {expected:?}. {HOW_TO_UPDATE}"
            ),
            SnapshotError::Diff {
                name,
                diff,
                diff_path,
            } => write!(
                f,
                "{name}: {diff} pixels differ (diff: {}). {HOW_TO_UPDATE}",
                diff_path.display()
            ),
            SnapshotError::WriteSnapshot { path, err } => {
                write!(f, "cannot write {}: {err}", path.display())
            }
            SnapshotError::Render(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Test,
    UpdateFailing,
    UpdateAll,
}

impl Mode {
    #[track_caller]
    fn from_env() -> Self {
        match std::env::var("UPDATE_SNAPSHOTS").as_deref() {
            Err(_) | Ok("false" | "0" | "no" | "off") => Mode::Test,
            Ok("true" | "1" | "yes" | "on") => Mode::UpdateFailing,
            Ok("force") => Mode::UpdateAll,
            Ok(other) => panic!("Unsupported value for UPDATE_SNAPSHOTS: {other:?}"),
        }
    }
}

/// A rendered frame as straight-alpha RGBA, the form PNGs and dify take.
pub fn to_rgba_image(img: &Image) -> image::RgbaImage {
    let mut px = img.rgba.clone();
    for p in px.chunks_exact_mut(4) {
        let a = p[3];
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
            }
        }
    }
    image::RgbaImage::from_raw(img.width, img.height, px).expect("width × height × 4 bytes")
}

/// Compare `img` with the golden `name` under `options` (kittest's `try_image_snapshot_options`).
pub fn try_image_snapshot_options(
    img: &Image,
    name: impl Into<String>,
    options: &SnapshotOptions,
) -> Result<(), SnapshotError> {
    compare(&to_rgba_image(img), name.into(), options)
}

/// [`try_image_snapshot_options`] with the default options.
pub fn try_image_snapshot(img: &Image, name: impl Into<String>) -> Result<(), SnapshotError> {
    try_image_snapshot_options(img, name, &SnapshotOptions::default())
}

/// Panics on a failed comparison (kittest's `image_snapshot`).
#[track_caller]
pub fn image_snapshot(img: &Image, name: impl Into<String>) {
    if let Err(e) = try_image_snapshot(img, name) {
        panic!("{e}");
    }
}

fn compare(new: &image::RgbaImage, name: String, options: &SnapshotOptions) -> Result<(), SnapshotError> {
    let mode = Mode::from_env();
    let dir = match PathBuf::from(&name).parent() {
        Some(p) => options.output_path.join(p),
        None => options.output_path.clone(),
    };
    std::fs::create_dir_all(dir).ok();
    let golden = options.output_path.join(format!("{name}.png"));
    let diff_path = options.output_path.join(format!("{name}.diff.png"));
    let old_path = options.output_path.join(format!("{name}.old.png"));
    let new_path = options.output_path.join(format!("{name}.new.png"));
    for p in [&diff_path, &old_path, &new_path] {
        std::fs::remove_file(p).ok();
    }

    let save = |path: &PathBuf, img: &image::RgbaImage| {
        img.save(path).map_err(|e| SnapshotError::WriteSnapshot {
            path: path.clone(),
            err: e.to_string(),
        })
    };
    let update = || {
        std::fs::rename(&golden, &old_path).ok();
        save(&golden, new)?;
        println!("Updated snapshot: {}", golden.display());
        Ok(())
    };

    let previous = match image::open(&golden) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            if mode != Mode::Test {
                return update();
            }
            save(&new_path, new)?;
            return Err(SnapshotError::OpenSnapshot {
                path: golden,
                err: e.to_string(),
            });
        }
    };
    if previous.dimensions() != new.dimensions() {
        if mode != Mode::Test {
            return update();
        }
        save(&new_path, new)?;
        return Err(SnapshotError::SizeMismatch {
            name,
            expected: previous.dimensions(),
            actual: new.dimensions(),
        });
    }
    let threshold = if mode == Mode::UpdateAll {
        0.0
    } else {
        options.threshold
    };
    let Some((wrong, diff)) =
        dify::diff::get_results(previous, new.clone(), threshold, true, None, &None, &None)
    else {
        return Ok(());
    };
    let within = i64::from(wrong) <= options.max_failed_pixels as i64;
    if !within {
        save(&diff_path, &diff)?;
    }
    match mode {
        Mode::Test if within => Ok(()),
        Mode::Test => {
            save(&new_path, new)?;
            Err(SnapshotError::Diff {
                name,
                diff: wrong,
                diff_path,
            })
        }
        Mode::UpdateFailing if within => Ok(()),
        Mode::UpdateFailing | Mode::UpdateAll => update(),
    }
}
