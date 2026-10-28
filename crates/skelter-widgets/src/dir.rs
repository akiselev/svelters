//! Mirroring for `rtl` (POL4-DIR-WIDGETS). A kind whose geometry is laid out left to right in one
//! box (a table, a tab strip, a calendar) mirrors it in that box where it meets the world: the
//! pointer goes in mirrored ([`flip_x`]), and the recording comes out mirrored — text through
//! [`MirrorText`] (each string drawn in its mirrored rect, start-aligned to the right, its glyphs in
//! reading order), every other primitive by [`mirror_items`] (rects, lines, clips and corner radii
//! mirrored; glyph runs left where [`MirrorText`] put them). Shapes keep their own orientation: a
//! check mark, a number or an icon is moved, never flipped; a kind that wants a chevron to point the
//! other way draws it so.

use crate::table::shim::{HAlign, TextPainter, TextStyle};
use skelter_base::display::{DisplayList, Prim};
use skelter_base::geom::{Corners, Insets, Point, Rect, Size};

/// `x` mirrored in a box `0..w` wide.
pub fn flip_x(x: f32, w: f32) -> f32 {
    w - x
}

/// `r` mirrored in a box `0..w` wide.
pub fn flip_rect(r: Rect, w: f32) -> Rect {
    Rect::new(w - r.max.x, r.min.y, w - r.min.x, r.max.y)
}

fn flip_pt(p: Point, w: f32) -> Point {
    Point::new(w - p.x, p.y)
}

fn flip_corners(c: Corners) -> Corners {
    Corners { tl: c.tr, tr: c.tl, br: c.bl, bl: c.br }
}

fn flip_insets(i: Insets) -> Insets {
    Insets { left: i.right, right: i.left, ..i }
}

/// The start/end alignment swapped (`rtl`: start is the right).
pub fn flip_align(a: HAlign) -> HAlign {
    match a {
        HAlign::Start => HAlign::End,
        HAlign::End => HAlign::Start,
        a => a,
    }
}

/// Mirrors `items` in a box `0..w` wide, except glyph runs (placed by [`MirrorText`]) and vector
/// paths (their transform places a shape whose width the list does not carry).
pub fn mirror_items(items: &mut [Prim], w: f32) {
    for p in items {
        match p {
            Prim::Quad(q) => {
                q.rect = flip_rect(q.rect, w);
                q.radii = flip_corners(q.radii);
                q.border_widths = flip_insets(q.border_widths);
            }
            Prim::Shadow(s) => {
                s.rect = flip_rect(s.rect, w);
                s.radii = flip_corners(s.radii);
                s.offset.x = -s.offset.x;
            }
            Prim::Line(l) => {
                l.from = flip_pt(l.from, w);
                l.to = flip_pt(l.to, w);
            }
            Prim::Icon(i) => i.rect = flip_rect(i.rect, w),
            Prim::Image(i) => i.rect = flip_rect(i.rect, w),
            Prim::PushClip(c) => {
                c.rect = flip_rect(c.rect, w);
                c.radii = flip_corners(c.radii);
            }
            _ => {}
        }
    }
}

/// Records `f` into `list`, mirrored in a box `0..w` wide when `rtl`: `f` draws left to right and
/// its text through the painter it is given.
pub fn record_mirrored(list: &mut DisplayList, text: &mut dyn TextPainter, rtl: bool, w: f32, f: impl FnOnce(&mut dyn TextPainter, &mut DisplayList)) {
    if !rtl {
        f(text, list);
        return;
    }
    let from = list.items.len();
    let mut m = MirrorText { inner: text, width: w };
    f(&mut m, list);
    mirror_items(&mut list.items[from..], w);
    list.bounds = flip_rect(list.bounds, w).union(&list.bounds);
}

/// A text painter that draws each string in its rect mirrored in a box `width` wide, start and end
/// swapped: what a left-to-right recording gives in `rtl` once [`mirror_items`] mirrors the rest.
pub struct MirrorText<'a> {
    pub inner: &'a mut dyn TextPainter,
    pub width: f32,
}

impl TextPainter for MirrorText<'_> {
    fn measure(&mut self, text: &str, style: &TextStyle) -> Size {
        self.inner.measure(text, style)
    }
    fn draw(&mut self, list: &mut DisplayList, text: &str, style: &TextStyle, rect: Rect, align: HAlign, elide: bool) {
        // the run lands mirrored; mark where it starts so the shapes pass leaves it alone, and
        // pre-mirror any non-glyph prims the painter adds (an elision clip) so they end up here
        let from = list.items.len();
        self.inner.draw(list, text, style, flip_rect(rect, self.width), flip_align(align), elide);
        mirror_items(&mut list.items[from..], self.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::shim::testing::FixedText;
    use skelter_base::color::Color;

    #[test]
    fn a_mirrored_recording_moves_shapes_and_puts_text_at_the_right() {
        let mut list = DisplayList::new();
        let mut text = FixedText::default();
        record_mirrored(&mut list, &mut text, true, 100.0, |t, l| {
            crate::table::shim::fill(l, Rect::new(10.0, 0.0, 30.0, 10.0), Color::BLACK, 0.0);
            crate::table::shim::line(l, Point::new(0.0, 5.0), Point::new(20.0, 5.0), 1.0, Color::BLACK);
            t.draw(l, "ab", &TextStyle::default(), Rect::new(0.0, 0.0, 50.0, 10.0), HAlign::Start, false);
        });
        let Prim::Quad(q) = &list.items[0] else { panic!() };
        assert_eq!((q.rect.min.x, q.rect.max.x), (70.0, 90.0));
        let Prim::Line(l) = &list.items[1] else { panic!() };
        assert_eq!((l.from.x, l.to.x), (100.0, 80.0));
        assert_eq!(text.drawn[0].1, Rect::new(50.0, 0.0, 100.0, 10.0), "the text's rect mirrored");
        assert_eq!(text.drawn[0].2, HAlign::End, "start-aligned to the right");
    }

    #[test]
    fn ltr_records_as_is() {
        let mut list = DisplayList::new();
        let mut text = FixedText::default();
        record_mirrored(&mut list, &mut text, false, 100.0, |_, l| crate::table::shim::fill(l, Rect::new(10.0, 0.0, 30.0, 10.0), Color::BLACK, 0.0));
        let Prim::Quad(q) = &list.items[0] else { panic!() };
        assert_eq!(q.rect.min.x, 10.0);
    }
}
