//! POL1-GALLERY item 4: `--diff`'s picture and numbers (no GPU).

use skelter_gallery::diff::{diff, diff_path, load_png};
use skelter_test::Image;
use std::path::Path;

fn solid(w: u32, h: u32, c: [u8; 4]) -> Image {
    Image { width: w, height: h, rgba: c.repeat((w * h) as usize) }
}

fn set(img: &mut Image, x: u32, y: u32, c: [u8; 4]) {
    let i = ((y * img.width + x) * 4) as usize;
    img.rgba[i..i + 4].copy_from_slice(&c);
}

#[test]
fn diff_counts_and_bounds_the_changed_pixels() {
    let a = solid(20, 10, [10, 20, 30, 255]);
    let same = diff(&a, &a);
    assert_eq!((same.changed, same.total, same.bounds), (0, 200, None));
    assert_eq!(same.summary(), "no change");
    // three panels side by side, 8 px apart
    assert_eq!((same.image.width, same.image.height), (8 * 4 + 20 * 3, 8 * 2 + 10));

    let mut b = a.clone();
    set(&mut b, 3, 2, [255, 255, 255, 255]);
    set(&mut b, 7, 5, [0, 0, 0, 255]);
    let d = diff(&a, &b);
    assert_eq!(d.changed, 2);
    assert_eq!(d.bounds, Some([3, 2, 5, 4]));
    assert_eq!(d.summary(), "2 of 200 pixels changed, in 3,2 5x4");
    // the changes panel paints a changed pixel magenta
    let (ox, oy) = (8 * 3 + 20 * 2, 8);
    let i = (((oy + 2) * d.image.width + ox + 3) * 4) as usize;
    assert_eq!(&d.image.rgba[i..i + 4], &[255, 0, 255, 255]);

    // a size change: the pixels only one image has count as changed
    let c = solid(20, 12, [10, 20, 30, 255]);
    let d = diff(&a, &c);
    assert_eq!((d.changed, d.total), (40, 240));
    assert!(d.summary().ends_with("(size 20x10 → 20x12)"), "{}", d.summary());
}

#[test]
fn the_diff_file_sits_beside_the_output_and_pngs_round_trip() {
    assert_eq!(diff_path(Path::new("/tmp/x/button.png")), Path::new("/tmp/x/button.diff.png"));
    let dir = std::env::temp_dir().join(format!("gallery-diff-{}", std::process::id()));
    let mut img = solid(4, 3, [200, 100, 50, 255]);
    set(&mut img, 1, 1, [0, 0, 0, 0]);
    let f = dir.join("a.png");
    skelter_gallery::save_png(&img, &f).unwrap();
    let back = load_png(&f).unwrap();
    assert_eq!((back.width, back.height), (4, 3));
    assert_eq!(back.rgba, img.rgba);
    std::fs::remove_dir_all(&dir).ok();
}
