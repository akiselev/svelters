//! The image primitive: an image fitted into a rect (`object-fit`: `contain`, `cover`, `fill`, `none`),
//! with `border-radius` corners. Loading is the `image` kind's (`controls/image.rs`) and the pixels
//! are the commit's resources; this only records. `opacity` is the node's effect (the compositor's).

use super::{PaintCx, StyleView};
use skelter_base::display::{Image, Prim, Sampling};
use skelter_base::geom::{Corners, DeviceSize, Point, Rect, Size};
use skelter_base::ids::ImageId;

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

/// Records image `image` of natural `size` (points, its pixels at 1x) in `rect` under `s`
/// (`object-fit`, default `contain`). A `cover` or `none` fit that overflows `rect` is cropped with a
/// source rectangle, so the image never paints outside its box.
pub fn paint_image(cx: &mut PaintCx, s: &StyleView, image: ImageId, size: Size, rect: Rect) {
    let pixels = DeviceSize { w: size.w.round().max(0.0) as u32, h: size.h.round().max(0.0) as u32 };
    paint_image_pixels(cx, s, s.keyword("object-fit").unwrap_or("contain"), image, size, pixels, rect);
}

/// [`paint_image`] with the fit given and the image's own pixel size (`pixels`: an SVG rasterised at
/// the size it is drawn at has more pixels than points), which the source rectangle is in.
pub fn paint_image_pixels(cx: &mut PaintCx, s: &StyleView, fit: &str, image: ImageId, size: Size, pixels: DeviceSize, rect: Rect) {
    if size.w <= 0.0 || size.h <= 0.0 || pixels.w == 0 || pixels.h == 0 || rect.is_empty() {
        return;
    }
    let r = fit_rect(fit, size, rect);
    let shown = r.intersect(&rect);
    if shown.is_empty() {
        return;
    }
    // the visible part in the image's pixels
    let (kx, ky) = (pixels.w as f32 / r.width(), pixels.h as f32 / r.height());
    let src = (shown != r).then(|| Rect::new((shown.min.x - r.min.x) * kx, (shown.min.y - r.min.y) * ky, (shown.max.x - r.min.x) * kx, (shown.max.y - r.min.y) * ky));
    let [tl, tr, br, bl] = s.radii();
    let sampling = match s.keyword("image-rendering") {
        Some("pixelated" | "crisp-edges") => Sampling::Nearest,
        _ => Sampling::Linear,
    };
    cx.push(Prim::Image(Image { image, rect: shown, src, radii: Corners { tl, tr, br, bl }, opacity: 1.0, sampling }), shown);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Ua;
    use crate::interact::StateBits;
    use crate::shim::testing::{MemResources, MonoShaper};
    use skelter_base::display::DisplayList;

    #[test]
    fn fits() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let img = Size::new(20.0, 20.0);
        assert_eq!(fit_rect("contain", img, r), Rect::new(25.0, 0.0, 75.0, 50.0));
        assert_eq!(fit_rect("cover", img, r), Rect::new(0.0, -25.0, 100.0, 75.0));
        assert_eq!(fit_rect("fill", img, r), r);
        assert_eq!(fit_rect("none", img, r), Rect::new(40.0, 15.0, 60.0, 35.0));
        assert_eq!(fit_rect("scale-down", img, r), Rect::new(40.0, 15.0, 60.0, 35.0));
        assert_eq!(fit_rect("scale-down", Size::new(200.0, 200.0), r), Rect::new(25.0, 0.0, 75.0, 50.0));
    }

    #[test]
    fn cover_crops_in_pixels() {
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = MonoShaper;
        let mut cx = PaintCx::new(&mut list, &mut res, &mut text, 1.0);
        let s = StyleView::ua_only(Ua::NONE, StateBits::NONE, true);
        // a 20pt image rasterised at 40 pixels, covering 100x50: the middle half of its rows
        paint_image_pixels(&mut cx, &s, "cover", ImageId(7), Size::new(20.0, 20.0), DeviceSize { w: 40, h: 40 }, Rect::new(0.0, 0.0, 100.0, 50.0));
        let Prim::Image(i) = &list.items[0] else { panic!() };
        assert_eq!(i.rect, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(i.src, Some(Rect::new(0.0, 10.0, 40.0, 30.0)));
        assert_eq!(i.image, ImageId(7));
    }
}
