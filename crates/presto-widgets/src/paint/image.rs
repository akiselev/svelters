//! The image primitive: an image fitted into a rect (`object-fit`: `contain`, `cover`, `fill`, `none`),
//! with `border-radius` corners. Decoding and the image store are presto-core's resources; this only
//! records. `opacity` is the node's effect (the compositor's).

use super::{PaintCx, StyleView};
use presto_base::display::{Image, Prim, Sampling};
use presto_base::geom::{Corners, Point, Rect, Size};
use presto_base::ids::ImageId;

/// The rect an image of natural `size` takes in `rect` under `object-fit` (centred; `contain` default).
pub fn fit_rect(fit: &str, size: Size, rect: Rect) -> Rect {
    let (sx, sy) = (rect.width() / size.w, rect.height() / size.h);
    let shown = match fit {
        "fill" => rect.size(),
        "none" => size,
        "cover" => Size::new(size.w * sx.max(sy), size.h * sx.max(sy)),
        "scale-down" => {
            let k = sx.min(sy).min(1.0);
            Size::new(size.w * k, size.h * k)
        }
        _ => Size::new(size.w * sx.min(sy), size.h * sx.min(sy)),
    };
    let c = Point::new((rect.min.x + rect.max.x) / 2.0, (rect.min.y + rect.max.y) / 2.0);
    Rect::from_origin_size(Point::new(c.x - shown.w / 2.0, c.y - shown.h / 2.0), shown)
}

/// Records image `image` of natural `size` (points) in `rect` under `s`. A `cover` or `none` fit that
/// overflows `rect` is cropped with a source rectangle, so the image never paints outside its box.
pub fn paint_image(cx: &mut PaintCx, s: &StyleView, image: ImageId, size: Size, rect: Rect) {
    if size.w <= 0.0 || size.h <= 0.0 || rect.is_empty() {
        return;
    }
    let fit = s.keyword("object-fit").unwrap_or("contain");
    let r = fit_rect(fit, size, rect);
    let shown = r.intersect(&rect);
    if shown.is_empty() {
        return;
    }
    // the visible part in the image's own units (points of its natural size)
    let (kx, ky) = (size.w / r.width(), size.h / r.height());
    let src = (shown != r).then(|| Rect::new((shown.min.x - r.min.x) * kx, (shown.min.y - r.min.y) * ky, (shown.max.x - r.min.x) * kx, (shown.max.y - r.min.y) * ky));
    let [tl, tr, br, bl] = s.radii();
    let sampling = match s.keyword("image-rendering") {
        Some("pixelated" | "crisp-edges") => Sampling::Nearest,
        _ => Sampling::Linear,
    };
    cx.push(Prim::Image(Image { image, rect: shown, src, radii: Corners { tl, tr, br, bl }, opacity: 1.0, sampling }), shown);
}
