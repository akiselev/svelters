//! Interpolation of animated values (CSS Values 4 "combining values", CSS Transforms 1 §interpolation):
//! numbers and lengths linearly, colours premultiplied in sRGB (the rule for legacy colours in
//! transitions and animations), transform lists per function when both lists have the same function
//! types, otherwise through the 2-D matrix decomposition (translate, rotate, residual, scale).
//!
//! Angles in [`TransformFn`] are radians.

use presto_base::anim::{AnimValue, TransformFn};
use presto_base::color::Color;
use presto_base::geom::{Affine, Point, Vec2};

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The value `t` of the way from `a` to `b` (`t` may leave [0, 1] under overshooting easings).
/// Values of different kinds don't interpolate: they flip at `t = 0.5` (the discrete rule), except
/// numbers and lengths, which mix as plain numbers.
pub fn lerp(a: &AnimValue, b: &AnimValue, t: f32) -> AnimValue {
    use AnimValue::*;
    match (a, b) {
        (Number(x), Number(y)) => Number(mix(*x, *y, t)),
        (Length(x), Length(y)) | (Number(x), Length(y)) | (Length(x), Number(y)) => Length(mix(*x, *y, t)),
        (Color(x), Color(y)) => Color(lerp_color(*x, *y, t)),
        (Vec2(x), Vec2(y)) => Vec2(presto_base::geom::Vec2::new(mix(x.x, y.x, t), mix(x.y, y.y, t))),
        (Rect(x), Rect(y)) => Rect(presto_base::geom::Rect {
            min: Point::new(mix(x.min.x, y.min.x, t), mix(x.min.y, y.min.y, t)),
            max: Point::new(mix(x.max.x, y.max.x, t), mix(x.max.y, y.max.y, t)),
        }),
        (Transform(x), Transform(y)) => Transform(lerp_transform(x, y, t)),
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// Premultiplied interpolation; channels clamp to [0, 1] (CSS clamps out-of-gamut overshoot).
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let pa = a.premultiplied();
    let pb = b.premultiplied();
    let alpha = mix(pa[3], pb[3], t).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    let ch = |i: usize| (mix(pa[i], pb[i], t) / alpha).clamp(0.0, 1.0);
    Color::rgba(ch(0), ch(1), ch(2), alpha)
}

/// The matrix of a transform list: functions apply right to left to a point, as in CSS
/// (`M = F1 · F2 · … · Fn`).
pub fn compose(fns: &[TransformFn]) -> Affine {
    fns.iter().fold(Affine::IDENTITY, |m, f| m.then(&to_matrix(f)))
}

pub fn to_matrix(f: &TransformFn) -> Affine {
    match *f {
        TransformFn::Translate(x, y) => Affine::translate(x, y),
        TransformFn::Scale(x, y) => Affine::scale(x, y),
        TransformFn::Rotate(a) => Affine::rotate(a),
        TransformFn::Skew(ax, ay) => Affine { a: 1.0, b: ay.tan(), c: ax.tan(), d: 1.0, e: 0.0, f: 0.0 },
        TransformFn::Matrix(m) => m,
    }
}

/// Functions that interpolate as themselves: the same type, or the same matrix (Svelte writes the
/// element's computed matrix in front of every keyframe's own functions).
fn same_kind(a: &TransformFn, b: &TransformFn) -> bool {
    match (a, b) {
        (TransformFn::Matrix(x), TransformFn::Matrix(y)) => x == y,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

/// The identity function of `f`'s type (for `none` ⇄ list and lists of different length).
fn identity_of(f: &TransformFn) -> TransformFn {
    match f {
        TransformFn::Translate(..) => TransformFn::Translate(0.0, 0.0),
        TransformFn::Scale(..) => TransformFn::Scale(1.0, 1.0),
        TransformFn::Rotate(_) => TransformFn::Rotate(0.0),
        TransformFn::Skew(..) => TransformFn::Skew(0.0, 0.0),
        TransformFn::Matrix(_) => TransformFn::Matrix(Affine::IDENTITY),
    }
}

fn lerp_fn(a: &TransformFn, b: &TransformFn, t: f32) -> TransformFn {
    match (*a, *b) {
        (TransformFn::Translate(ax, ay), TransformFn::Translate(bx, by)) => TransformFn::Translate(mix(ax, bx, t), mix(ay, by, t)),
        (TransformFn::Scale(ax, ay), TransformFn::Scale(bx, by)) => TransformFn::Scale(mix(ax, bx, t), mix(ay, by, t)),
        (TransformFn::Rotate(x), TransformFn::Rotate(y)) => TransformFn::Rotate(mix(x, y, t)),
        (TransformFn::Skew(ax, ay), TransformFn::Skew(bx, by)) => TransformFn::Skew(mix(ax, bx, t), mix(ay, by, t)),
        (TransformFn::Matrix(x), TransformFn::Matrix(y)) if x == y => TransformFn::Matrix(x),
        _ => TransformFn::Matrix(lerp_matrix(&to_matrix(a), &to_matrix(b), t)),
    }
}

/// CSS Transforms 1 §9: pad the shorter list with identity functions when one list is a prefix
/// type-match of the other, interpolate per function when the types match pairwise, otherwise
/// interpolate the decomposed matrices.
pub fn lerp_transform(a: &[TransformFn], b: &[TransformFn], t: f32) -> Vec<TransformFn> {
    let n = a.len().max(b.len());
    let pairwise = (0..n).all(|i| match (a.get(i), b.get(i)) {
        (Some(x), Some(y)) => same_kind(x, y),
        (Some(x), None) | (None, Some(x)) => !matches!(x, TransformFn::Matrix(_)),
        (None, None) => true,
    });
    if pairwise {
        return (0..n)
            .map(|i| {
                let x = a.get(i).copied().unwrap_or_else(|| identity_of(&b[i]));
                let y = b.get(i).copied().unwrap_or_else(|| identity_of(&a[i]));
                lerp_fn(&x, &y, t)
            })
            .collect();
    }
    // Same leading functions stay per function; the rest goes through one matrix (Transforms 2 §16).
    let common = a.iter().zip(b).take_while(|(x, y)| same_kind(x, y)).count();
    let mut out: Vec<TransformFn> = (0..common).map(|i| lerp_fn(&a[i], &b[i], t)).collect();
    out.push(TransformFn::Matrix(lerp_matrix(&compose(&a[common..]), &compose(&b[common..]), t)));
    out
}

/// A 2-D matrix split as `translate · rotate(angle) · residual · scale(sx, sy)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    pub translate: Vec2,
    pub scale: Vec2,
    /// Radians.
    pub angle: f32,
    /// The remaining 2×2 (skew), column-major `[a, b, c, d]`.
    pub residual: [f32; 4],
}

/// CSS Transforms 1 "Decomposing a 2D matrix" (unmatrix), with columns where the spec has rows.
pub fn decompose(m: &Affine) -> Decomposed {
    let (mut r0x, mut r0y, mut r1x, mut r1y) = (m.a, m.b, m.c, m.d);
    let mut sx = (r0x * r0x + r0y * r0y).sqrt();
    let mut sy = (r1x * r1x + r1y * r1y).sqrt();
    let det = r0x * r1y - r0y * r1x;
    if det < 0.0 {
        if r0x < r1y {
            sx = -sx;
        } else {
            sy = -sy;
        }
    }
    if sx != 0.0 {
        r0x /= sx;
        r0y /= sx;
    }
    if sy != 0.0 {
        r1x /= sy;
        r1y /= sy;
    }
    let angle = r0y.atan2(r0x);
    if angle != 0.0 {
        // Undo the rotation: residual = rotate(−angle) · normalised.
        let (sn, cs) = (-r0y, r0x);
        let (m11, m12, m21, m22) = (r0x, r0y, r1x, r1y);
        r0x = cs * m11 - sn * m12;
        r0y = sn * m11 + cs * m12;
        r1x = cs * m21 - sn * m22;
        r1y = sn * m21 + cs * m22;
    }
    Decomposed { translate: Vec2::new(m.e, m.f), scale: Vec2::new(sx, sy), angle, residual: [r0x, r0y, r1x, r1y] }
}

pub fn recompose(d: &Decomposed) -> Affine {
    let [a, b, c, dd] = d.residual;
    let residual = Affine { a, b, c, d: dd, e: 0.0, f: 0.0 };
    Affine::translate(d.translate.x, d.translate.y)
        .then(&Affine::rotate(d.angle))
        .then(&residual)
        .then(&Affine::scale(d.scale.x, d.scale.y))
}

/// Interpolates decomposed matrices; the angle takes the short way round and a flip on opposite
/// axes becomes a rotation (CSS Transforms 1 "Interpolation of decomposed 2D matrix values").
pub fn lerp_matrix(a: &Affine, b: &Affine, t: f32) -> Affine {
    use std::f32::consts::{PI, TAU};
    let mut da = decompose(a);
    let mut db = decompose(b);
    if (da.scale.x < 0.0 && db.scale.y < 0.0) || (da.scale.y < 0.0 && db.scale.x < 0.0) {
        da.scale = Vec2::new(-da.scale.x, -da.scale.y);
        da.angle += if da.angle < 0.0 { PI } else { -PI };
    }
    if da.angle == 0.0 {
        da.angle = TAU;
    }
    if db.angle == 0.0 {
        db.angle = TAU;
    }
    if (da.angle - db.angle).abs() > PI {
        if da.angle > db.angle {
            da.angle -= TAU;
        } else {
            db.angle -= TAU;
        }
    }
    let r = |i: usize| mix(da.residual[i], db.residual[i], t);
    recompose(&Decomposed {
        translate: Vec2::new(mix(da.translate.x, db.translate.x, t), mix(da.translate.y, db.translate.y, t)),
        scale: Vec2::new(mix(da.scale.x, db.scale.x, t), mix(da.scale.y, db.scale.y, t)),
        angle: mix(da.angle, db.angle, t),
        residual: [r(0), r(1), r(2), r(3)],
    })
}

/// A scalar distance between two values, for settle thresholds (springs): absolute difference for
/// numbers and lengths, Euclidean for vectors, 1 for everything else.
pub fn magnitude(a: &AnimValue, b: &AnimValue) -> f32 {
    use AnimValue::*;
    match (a, b) {
        (Number(x) | Length(x), Number(y) | Length(y)) => (y - x).abs(),
        (Vec2(x), Vec2(y)) => (*y - *x).length(),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Affine, b: &Affine) -> bool {
        let e = 1e-4;
        (a.a - b.a).abs() < e && (a.b - b.b).abs() < e && (a.c - b.c).abs() < e && (a.d - b.d).abs() < e && (a.e - b.e).abs() < e && (a.f - b.f).abs() < e
    }

    #[test]
    fn decompose_round_trips() {
        let ms = [
            Affine::IDENTITY,
            Affine::translate(10.0, -3.0).then(&Affine::rotate(0.7)).then(&Affine::scale(2.0, 0.5)),
            Affine { a: 1.0, b: 0.3, c: 0.5, d: 1.2, e: 4.0, f: 5.0 },
            Affine::scale(-1.0, 1.0),
            Affine::rotate(3.0).then(&Affine::scale(1.0, -2.0)),
        ];
        for m in ms {
            assert!(close(&recompose(&decompose(&m)), &m), "{m:?}");
        }
    }

    #[test]
    fn matrix_lerp_end_points() {
        let a = Affine::translate(0.0, 0.0);
        let b = Affine::translate(100.0, 0.0).then(&Affine::rotate(1.0)).then(&Affine::scale(2.0, 2.0));
        assert!(close(&lerp_matrix(&a, &b, 0.0), &a));
        assert!(close(&lerp_matrix(&a, &b, 1.0), &b));
        let mid = decompose(&lerp_matrix(&a, &b, 0.5));
        assert!((mid.angle - 0.5).abs() < 1e-4);
        assert!((mid.scale.x - 1.5).abs() < 1e-4);
    }

    #[test]
    fn same_lists_interpolate_per_function() {
        let a = [TransformFn::Translate(0.0, 0.0), TransformFn::Scale(1.0, 1.0)];
        let b = [TransformFn::Translate(10.0, 20.0), TransformFn::Scale(0.5, 0.5)];
        assert_eq!(lerp_transform(&a, &b, 0.5), vec![TransformFn::Translate(5.0, 10.0), TransformFn::Scale(0.75, 0.75)]);
        // none → list
        assert_eq!(lerp_transform(&[], &b, 0.5), vec![TransformFn::Translate(5.0, 10.0), TransformFn::Scale(0.75, 0.75)]);
    }

    #[test]
    fn matrix_prefix_from_computed_style() {
        // Svelte's fly: `<matrix from getComputedStyle> translate(x, y)` at every keyframe.
        let m = TransformFn::Matrix(Affine::scale(2.0, 2.0));
        let a = [m, TransformFn::Translate(100.0, 0.0)];
        let b = [m, TransformFn::Translate(0.0, 0.0)];
        let out = lerp_transform(&a, &b, 0.25);
        assert!(close(&compose(&out), &Affine::scale(2.0, 2.0).then(&Affine::translate(75.0, 0.0))));
    }

    #[test]
    fn colors_premultiplied() {
        let red = Color::rgba(1.0, 0.0, 0.0, 1.0);
        let clear = Color::rgba(0.0, 0.0, 1.0, 0.0);
        let mid = lerp_color(red, clear, 0.5);
        assert!((mid.a - 0.5).abs() < 1e-6);
        assert!((mid.r - 1.0).abs() < 1e-6 && mid.b.abs() < 1e-6);
    }
}
