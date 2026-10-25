//! The pixel tier's comparison, on images made in the test (no GPU): a missing golden fails and writes
//! `.new.png`, the same image passes, a changed one fails with a diff unless the failed-pixel budget
//! allows it, a size change fails.

use skelter_test::{Image, SnapshotError, SnapshotOptions, try_image_snapshot_options};
use std::path::PathBuf;

fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Image {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&f(x, y));
        }
    }
    Image {
        width: w,
        height: h,
        rgba,
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("skelter-test-snapshot-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    d
}

fn base() -> Image {
    image(32, 16, |x, _| if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })
}

#[test]
fn a_golden_round_trip() {
    let out = dir("round-trip");
    let opts = SnapshotOptions::new().output_path(&out);
    let img = base();
    match try_image_snapshot_options(&img, "frame", &opts) {
        Err(SnapshotError::OpenSnapshot { .. }) => {}
        other => panic!("a missing golden fails: {other:?}"),
    }
    let new = out.join("frame.new.png");
    assert!(new.exists(), "the run wrote frame.new.png");
    std::fs::rename(&new, out.join("frame.png")).unwrap();

    try_image_snapshot_options(&img, "frame", &opts).expect("the same image passes");
    assert!(!out.join("frame.new.png").exists());

    let changed = image(32, 16, |x, y| {
        if x < 16 && y < 8 { [0, 255, 0, 255] } else if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }
    });
    match try_image_snapshot_options(&changed, "frame", &opts) {
        Err(SnapshotError::Diff { diff, diff_path, .. }) => {
            assert!(diff > 0);
            assert!(diff_path.exists(), "the diff image is written");
        }
        other => panic!("a changed image fails: {other:?}"),
    }
    let lenient = opts.clone().failed_pixel_count_threshold(32 * 16);
    try_image_snapshot_options(&changed, "frame", &lenient).expect("within the failed-pixel budget");

    let bigger = image(33, 16, |_, _| [255, 0, 0, 255]);
    assert!(matches!(
        try_image_snapshot_options(&bigger, "frame", &opts),
        Err(SnapshotError::SizeMismatch { .. })
    ));
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn premultiplied_frames_compare_unpremultiplied() {
    let half = image(4, 4, |_, _| [64, 0, 0, 128]);
    let rgba = skelter_test::snapshot::to_rgba_image(&half);
    assert_eq!(rgba.get_pixel(0, 0).0, [128, 0, 0, 128]);
}
