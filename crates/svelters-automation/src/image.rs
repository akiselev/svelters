//! [`Image`]: a captured frame (`capture.screenshot`), RGBA8 without premultiplied alpha.

use crate::geom::Rect;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    /// Width and height in pixels.
    pub size: [usize; 2],
    /// `size[0] * size[1]` pixels, row by row, 4 bytes each (R, G, B, A; alpha not premultiplied).
    pub rgba: Vec<u8>,
}

impl Image {
    /// An image from unpremultiplied RGBA8 rows. Panics when `rgba` doesn't have `4 * w * h` bytes.
    pub fn from_rgba_unmultiplied(size: [usize; 2], rgba: &[u8]) -> Image {
        assert_eq!(rgba.len(), 4 * size[0] * size[1], "rgba holds 4 bytes per pixel");
        Image { size, rgba: rgba.to_vec() }
    }

    pub fn width(&self) -> usize {
        self.size[0]
    }

    pub fn height(&self) -> usize {
        self.size[1]
    }

    /// The pixel at `(x, y)`.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = 4 * (y * self.size[0] + x);
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    /// The part of the image under `rect` (in points; `pixels_per_point`: `None` is 1), clamped to
    /// the image (egui's `ColorImage::region`).
    pub fn region(&self, rect: &Rect, pixels_per_point: Option<f32>) -> Image {
        let ppp = pixels_per_point.unwrap_or(1.0);
        let clamp = |v: f32, max: usize| ((v * ppp).round().max(0.0) as usize).min(max);
        let (x0, x1) = (clamp(rect.min.x, self.size[0]), clamp(rect.max.x, self.size[0]));
        let (y0, y1) = (clamp(rect.min.y, self.size[1]), clamp(rect.max.y, self.size[1]));
        let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
        let mut rgba = Vec::with_capacity(4 * w * h);
        for y in y0..y0 + h {
            let row = 4 * (y * self.size[0]);
            rgba.extend_from_slice(&self.rgba[row + 4 * x0..row + 4 * (x0 + w)]);
        }
        Image { size: [w, h], rgba }
    }
}

#[cfg(feature = "egui")]
impl From<egui::ColorImage> for Image {
    fn from(c: egui::ColorImage) -> Image {
        let rgba = c.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
        Image { size: c.size, rgba }
    }
}

#[cfg(feature = "egui")]
impl From<Image> for egui::ColorImage {
    fn from(i: Image) -> egui::ColorImage {
        egui::ColorImage::from_rgba_unmultiplied(i.size, &i.rgba)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pos2;

    #[test]
    fn region_crops_in_pixels() {
        let px: Vec<u8> = (0..4 * 4 * 3).map(|i| i as u8).collect();
        let img = Image::from_rgba_unmultiplied([4, 3], &px);
        let r = img.region(&Rect::from_min_max(pos2(0.5, 0.5), pos2(1.5, 1.5)), Some(2.0));
        assert_eq!(r.size, [2, 2]);
        assert_eq!(r.pixel(0, 0), img.pixel(1, 1));
        assert_eq!(r.pixel(1, 1), img.pixel(2, 2));
        let out = img.region(&Rect::from_min_max(pos2(3.0, 2.0), pos2(9.0, 9.0)), None);
        assert_eq!(out.size, [1, 1]);
    }
}
