//! What a drag draws: the ghost label that follows the pointer (the source's `::part(ghost)`: a
//! floating surface at `elev-3`, 0.9 opacity, recorded into the window's drag layer, architecture
//! §6.3 and §14) and the active target's outline (`::part(drop-line)`: a 2-point `focus` stroke
//! inside the target's box).

use crate::table::shim::{HAlign, TextPainter, TextStyle};
use skelter_base::color::{Color, Fill};
use skelter_base::display::{BorderStyle, DisplayList, Prim, Quad, Shadow};
use skelter_base::geom::{Corners, Insets, Point, Rect, Vec2};

/// The ghost's look (from `::part(ghost)` over its UA defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct GhostLook {
    pub text: TextStyle,
    pub background: Color,
    pub border: Color,
    pub border_width: f32,
    pub radius: f32,
    pub shadow: Color,
    pub shadow_blur: f32,
    pub shadow_offset: Vec2,
    pub padding: Insets,
    /// The ghost's layer opacity (set on the drag layer's effect node, not baked into colours).
    pub opacity: f32,
    /// Where the ghost sits relative to the pointer.
    pub offset: Vec2,
}

impl Default for GhostLook {
    fn default() -> Self {
        GhostLook {
            text: TextStyle::default(),
            background: Color::WHITE,
            border: Color::from_rgba8(200, 200, 200, 255),
            border_width: 1.0,
            radius: 4.0,
            shadow: Color::from_rgba8(0, 0, 0, 60),
            shadow_blur: 8.0,
            shadow_offset: Vec2::new(0.0, 2.0),
            padding: Insets { top: 3.0, right: 6.0, bottom: 3.0, left: 6.0 },
            opacity: 0.9,
            offset: Vec2::new(14.0, 14.0),
        }
    }
}

/// The text a ghost shows: the source's text content, else its drag type.
pub fn ghost_text<'a>(source_text: &'a str, drag_type: &'a str) -> &'a str {
    if source_text.is_empty() { drag_type } else { source_text }
}

/// Records the ghost for a pointer at `pointer` (drag-layer coordinates) and returns its box.
pub fn record_ghost(text: &str, pointer: Point, look: &GhostLook, painter: &mut dyn TextPainter, list: &mut DisplayList) -> Rect {
    let size = painter.measure(text, &look.text);
    let p = &look.padding;
    let min = pointer + look.offset;
    let rect = Rect::new(min.x, min.y, min.x + size.w + p.left + p.right, min.y + size.h + p.top + p.bottom);
    let radii = Corners::all(look.radius);
    if look.shadow.a > 0.0 {
        let ink = rect.translate(look.shadow_offset).inflate(look.shadow_blur);
        list.push(Prim::Shadow(Shadow { rect, radii, color: look.shadow, blur: look.shadow_blur, spread: 0.0, offset: look.shadow_offset, inset: false }), ink);
    }
    let q = Quad { rect, radii, fill: Fill::Solid(look.background), border_widths: Insets::all(look.border_width), border_color: look.border, border_style: BorderStyle::Solid };
    list.push(Prim::Quad(q), rect);
    painter.draw(list, text, &look.text, rect.inset(look.padding), HAlign::Start, false);
    rect
}

/// Records the active target's outline: a `width` stroke inside `rect` (target-local), radius 2.
pub fn record_drop_line(rect: Rect, color: Color, width: f32, list: &mut DisplayList) {
    let w = width.max(1.0);
    let q = Quad { rect, radii: Corners::all(2.0), fill: Fill::Solid(Color::TRANSPARENT), border_widths: Insets::all(w), border_color: color, border_style: BorderStyle::Solid };
    list.push(Prim::Quad(q), rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::shim::testing::FixedText;

    #[test]
    fn the_ghost_follows_the_pointer_with_its_padding() {
        let mut text = FixedText::default();
        let mut list = DisplayList::new();
        let look = GhostLook::default();
        let r = record_ghost(ghost_text("", "cad/part"), Point::new(100.0, 50.0), &look, &mut text, &mut list);
        assert_eq!(r.min, Point::new(114.0, 64.0));
        let w = "cad/part".len() as f32 * FixedText::advance(&look.text) + 12.0;
        assert!((r.width() - w).abs() < 1e-4);
        assert_eq!(text.texts(), vec!["cad/part"]);
        assert!(matches!(list.items[0], Prim::Shadow(_)));
        assert!(matches!(list.items[1], Prim::Quad(_)));
        assert_eq!(ghost_text("R1", "cad/part"), "R1");
        let mut outline = DisplayList::new();
        record_drop_line(Rect::new(0.0, 0.0, 50.0, 20.0), Color::BLACK, 2.0, &mut outline);
        assert_eq!(outline.items.len(), 1);
    }
}
