//! The coverage rasteriser on hand-made polygons.

use crate::raster::coverage::{bounds, embolden, flatten, Accumulator, Cmd, Pt};

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Vec<Pt>> {
    flatten(&[
        Cmd::Move(Pt::new(x0, y0)),
        Cmd::Line(Pt::new(x1, y0)),
        Cmd::Line(Pt::new(x1, y1)),
        Cmd::Line(Pt::new(x0, y1)),
        Cmd::Close,
    ])
}

fn mask(polys: &[Vec<Pt>], x_only: bool) -> (u32, u32, Vec<u8>) {
    let (x0, y0, x1, y1) = bounds(polys).unwrap();
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut acc = Accumulator::new(w, h, x0, y0);
    acc.fill(polys, x_only);
    (w, h, acc.to_mask())
}

#[test]
fn pixel_aligned_square_is_fully_covered() {
    let (w, h, m) = mask(&rect(1.0, 1.0, 4.0, 3.0), false);
    assert_eq!((w, h), (3, 2));
    assert!(m.iter().all(|v| *v == 255), "{m:?}");
}

#[test]
fn half_pixel_edges_are_half_covered() {
    let (w, h, m) = mask(&rect(0.5, 0.0, 2.5, 1.0), false);
    assert_eq!((w, h), (3, 1));
    assert_eq!(m, vec![128, 255, 128]);
}

#[test]
fn winding_is_independent_of_orientation() {
    let cw = rect(0.25, 0.25, 3.75, 2.75);
    let mut ccw = cw.clone();
    ccw[0].reverse();
    assert_eq!(mask(&cw, false), mask(&ccw, false));
}

#[test]
fn horizontal_only_mode_samples_row_centres() {
    // Covers y 0.2..0.7 of row 0 (its centre 0.5 is inside) and x 0.5..2.5.
    let polys = rect(0.5, 0.2, 2.5, 0.7);
    let (_, _, sym) = mask(&polys, false);
    let (_, _, xonly) = mask(&polys, true);
    assert_eq!(xonly, vec![128, 255, 128], "x stays antialiased, y is binary");
    assert_eq!(sym, vec![64, 128, 64], "half the row's height is covered");
    // A sliver that misses the centre line draws nothing in x-only mode.
    let (_, _, miss) = mask(&rect(0.0, 0.6, 2.0, 0.9), true);
    assert!(miss.iter().all(|v| *v == 0), "{miss:?}");
}

#[test]
fn curves_flatten_within_bounds() {
    let polys = flatten(&[
        Cmd::Move(Pt::new(0.0, 5.0)),
        Cmd::Quad(Pt::new(5.0, -5.0), Pt::new(10.0, 5.0)),
        Cmd::Cubic(Pt::new(7.0, 8.0), Pt::new(3.0, 8.0), Pt::new(0.0, 5.0)),
        Cmd::Close,
    ]);
    assert_eq!(polys.len(), 1);
    assert!(polys[0].len() > 8);
    let (x0, y0, x1, y1) = bounds(&polys).unwrap();
    assert!(x0 >= 0 && x1 <= 10 && y0 >= -1 && y1 <= 8, "{:?}", (x0, y0, x1, y1));
}

#[test]
fn embolden_grows_a_rect_by_the_strength() {
    let mut polys = rect(2.0, 2.0, 6.0, 6.0);
    embolden(&mut polys, 1.0);
    let (x0, y0, x1, y1) = bounds(&polys).unwrap();
    // Each edge moves out by 0.5 px: 1.5..6.5 covers pixels 1..7.
    assert_eq!((x0, y0, x1, y1), (1, 1, 7, 7));
    let (_, _, m) = mask(&polys, false);
    assert_eq!(m[0], 64, "a corner pixel half covered in x and y");
}

#[test]
fn masks_are_deterministic() {
    let polys = flatten(&[
        Cmd::Move(Pt::new(0.3, 0.7)),
        Cmd::Cubic(Pt::new(4.1, -2.0), Pt::new(9.7, 3.3), Pt::new(6.2, 9.9)),
        Cmd::Line(Pt::new(1.1, 7.7)),
        Cmd::Close,
    ]);
    assert_eq!(mask(&polys, false), mask(&polys, false));
    assert_eq!(mask(&polys, true), mask(&polys, true));
}
