//! What a drag draws: the ghost label that follows the pointer (the source's `::part(ghost)`: a
//! floating surface at `elev-3`, 0.9 opacity, recorded into the window's drag layer, architecture
//! §6.3 and §14) and the active target's outline (`::part(drop-line)`: a 2-point `focus` stroke
//! inside the target's box). [`install`] makes the ghost the drag image of skelter-core's in-app
//! drags ([`skelter_core::DragPreview`]) whose source kind supplies none; `host::install` installs it.

use crate::shim::text::TextShaper;
use crate::table::shim::{HAlign, TextPainter, TextStyle};
use crate::table::text_painter::{CollectRes, NoShaper, ShaperPainter};
use skelter_core::{Document, DragPreview, common};
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

/// The ghost as a drag image: recorded with the pointer at the list's origin, its glyph runs
/// collected (a run's `Prim::Glyphs` names it by position plus one).
pub fn ghost_preview(text: &str, look: &GhostLook, shaper: &mut dyn TextShaper) -> DragPreview {
    let mut res = CollectRes::default();
    let mut list = DisplayList::new();
    {
        let mut p = ShaperPainter { text: shaper, res: Some(&mut res) };
        record_ghost(text, Point::ZERO, look, &mut p, &mut list);
    }
    DragPreview::List { list, runs: res.runs, offset: Vec2::ZERO }
}

/// Installs the ghost in `look` as the drag image of `doc`'s drags whose source kind supplies none:
/// the source's text content, else its `drag-type`, shaped by the document's text system.
pub fn install(doc: &mut Document, look: GhostLook) {
    doc.set_drag_preview_provider(Box::new(move |doc, n, _at| {
        let v = doc.view(n)?;
        let text = ghost_text(v.text(), v.prop_str(common::DRAG_TYPE).unwrap_or("")).to_owned();
        let mut none = NoShaper;
        let shaper: &mut dyn TextShaper = match doc.service::<skelter_text::TextSystem>() {
            Some(t) => t,
            None => &mut none,
        };
        Some(ghost_preview(&text, &look, shaper))
    }));
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

    #[test]
    fn a_core_drag_carries_the_ghost_as_its_preview() {
        use crate::testdoc::{BOX, child, doc, frame, pointer};
        use skelter_base::event::PointerPhase;
        let (mut d, root) = doc(skelter_base::geom::Size::new(200.0, 100.0), |_| {});
        let src = child(&mut d, root, BOX, "width: 40px; height: 40px");
        d.set_prop(src, common::DRAG_TYPE, skelter_core::Value::from("cad/part")).unwrap();
        install(&mut d, GhostLook::default());
        frame(&mut d);
        d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 0, false));
        d.dispatch(&pointer(PointerPhase::Move, 40.0, 10.0, 10, true));
        let Some(DragPreview::List { list, offset, .. }) = d.drag_preview() else { panic!("the ghost: {:?}", d.drag_preview()) };
        assert_eq!(*offset, Vec2::ZERO);
        let look = GhostLook::default();
        assert!(matches!(list.items[0], Prim::Shadow(_)), "the ghost's shadow");
        let at = Point::new(look.offset.x, look.offset.y);
        assert!(matches!(&list.items[1], Prim::Quad(q) if q.rect.min == at), "its surface, at the pointer plus its offset");
    }
}
