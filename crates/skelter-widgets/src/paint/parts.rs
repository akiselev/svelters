//! WIDGET-PARITY P3: paint parts laid out from CSS inside the host's content box.
//!
//! The parts are flex items of the host's content box: the host's `flex-direction` (`row`
//! default, `column`, `row-reverse`, `column-reverse`), `gap` and `justify-content` (`start`,
//! `center`, `end`), each part's `order`, `width`/`height` (else its content size), `margin` and
//! `display: none`. Cross axis: centred. A row runs from the inline start: in `rtl` (the host's
//! `direction`) the first part sits at the right, as a flex row of nodes does. So `button { flex-direction: column }` puts the label
//! under the icon (Qt's TextUnderIcon) and `button::part(label) { display: none }` gives an
//! icon-only button, with no Rust per layout. Node parts are laid out by taffy instead.

use super::StyleView;
use skelter_base::geom::{Point, Rect, Size, Vec2};

fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// One paint part to place: its style (a view whose parent is the host's) and content size.
pub struct PartItem<'v> {
    pub name: &'static str,
    pub style: &'v StyleView<'v>,
    /// The content's natural size (a galley's, an icon's).
    pub content: Vec2,
}

fn axis(host: &StyleView) -> (bool, bool) {
    match host.keyword("flex-direction") {
        Some("column") => (false, false),
        Some("column-reverse") => (false, true),
        Some("row-reverse") => (true, true),
        _ => (true, false),
    }
}

/// The margin box size of `p` (its `width`/`height` or content size, plus margins); `None` when
/// `display: none`.
fn outer(p: &PartItem) -> Option<(Vec2, [f32; 4])> {
    if p.style.is_hidden() {
        return None;
    }
    let w = p.style.px("width").unwrap_or(p.content.x);
    let h = p.style.px("height").unwrap_or(p.content.y);
    let m = p.style.sides("margin");
    Some((vec2(w + m[0] + m[1], h + m[2] + m[3]), m))
}

/// The size the parts need along the host's axis (sum plus gaps) and across it (the largest).
pub fn measure_parts(host: &StyleView, parts: &[PartItem]) -> Vec2 {
    let (row, _) = axis(host);
    let gap = host.px("gap").unwrap_or(0.0);
    let sizes: Vec<Vec2> = parts.iter().filter_map(|p| outer(p).map(|(s, _)| s)).collect();
    let gaps = gap * sizes.len().saturating_sub(1) as f32;
    let (main, cross) = sizes.iter().fold((0.0f32, 0.0f32), |(m, c), s| if row { (m + s.x, c.max(s.y)) } else { (m + s.y, c.max(s.x)) });
    if row { vec2(main + gaps, cross) } else { vec2(cross, main + gaps) }
}

/// Places `parts` in `content`: each part's border box (`None` for a hidden part), in the order
/// of `parts`.
pub fn layout_parts(host: &StyleView, content: Rect, parts: &[PartItem]) -> Vec<Option<Rect>> {
    let (row, reverse) = axis(host);
    let rtl = row && host.is_rtl();
    let gap = host.px("gap").unwrap_or(0.0);
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&i| parts[i].style.number("order").unwrap_or(0.0) as i32);
    if reverse {
        order.reverse();
    }
    let need = measure_parts(host, parts);
    let (avail, mut need_main) = if row { (content.width(), need.x) } else { (content.height(), need.y) };
    // a row narrower than its parts: the label gives up the overflow (the kind elides it in its
    // narrower rect, [`super::fit_layout`]), as a caption is the one part that can shrink
    let mut label_cut = 0.0;
    if row && need_main > avail
        && let Some(l) = parts.iter().find(|p| p.name == "label" && !p.style.is_hidden())
    {
        label_cut = (need_main - avail).min(l.style.px("width").unwrap_or(l.content.x));
        need_main -= label_cut;
    }
    let start = match host.keyword("justify-content") {
        Some("center") => (avail - need_main) / 2.0,
        Some("end" | "flex-end") => avail - need_main,
        _ => 0.0,
    };
    let mut out = vec![None; parts.len()];
    let mut at = start;
    for i in order {
        let Some((mut size, m)) = outer(&parts[i]) else { continue };
        if parts[i].name == "label" {
            size.x -= label_cut;
        }
        let inner = vec2(size.x - m[0] - m[1], size.y - m[2] - m[3]);
        let inner = Size::new(inner.x, inner.y);
        let r = if row {
            let y = (content.min.y + content.max.y) / 2.0 - inner.h / 2.0 + (m[2] - m[3]) / 2.0;
            // `rtl`: the main axis starts at the right; the part's inline-start margin is its right
            let x = if rtl { content.max.x - at - m[1] - inner.w } else { content.min.x + at + m[0] };
            Rect::from_origin_size(Point::new(x, y), inner)
        } else {
            let x = (content.min.x + content.max.x) / 2.0 - inner.w / 2.0 + (m[0] - m[1]) / 2.0;
            Rect::from_origin_size(Point::new(x, content.min.y + at + m[2]), inner)
        };
        out[i] = Some(r);
        at += if row { size.x } else { size.y } + gap;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::StateBits;
    use crate::paint::Ua;

    #[test]
    fn the_label_gives_up_a_rows_overflow() {
        let v = StyleView::ua_only(Ua::NONE, StateBits::NONE, true);
        let parts = [PartItem { name: "icon", style: &v, content: vec2(16.0, 16.0) }, PartItem { name: "label", style: &v, content: vec2(200.0, 18.0) }];
        let wide = layout_parts(&v, Rect::new(0.0, 0.0, 300.0, 20.0), &parts);
        assert_eq!(wide[1].unwrap().width(), 200.0, "room: the label's own width");
        let narrow = layout_parts(&v, Rect::new(0.0, 0.0, 100.0, 20.0), &parts);
        assert_eq!(narrow[0].unwrap().width(), 16.0, "the icon keeps its size");
        assert_eq!(narrow[1].unwrap().min.x, 16.0);
        assert_eq!(narrow[1].unwrap().max.x, 100.0, "the label ends at the content box");
    }

    #[test]
    fn an_rtl_row_runs_from_the_right() {
        use crate::testdoc::{BOX, child, doc, frame};
        let (mut d, root) = doc(Size::new(300.0, 100.0), |_| {});
        let n = child(&mut d, root, BOX, "direction: rtl; width: 200px; height: 20px; gap: 4px");
        frame(&mut d);
        let css = d.view(n).and_then(|v| v.css()).expect("computed");
        let host = StyleView::new(Some(css), Ua::NONE, StateBits::NONE, true);
        assert!(host.is_rtl());
        let v = StyleView::ua_only(Ua::NONE, StateBits::NONE, true);
        let parts = [PartItem { name: "indicator", style: &v, content: vec2(16.0, 16.0) }, PartItem { name: "label", style: &v, content: vec2(50.0, 18.0) }];
        let r = layout_parts(&host, Rect::new(0.0, 0.0, 200.0, 20.0), &parts);
        assert_eq!((r[0].unwrap().min.x, r[0].unwrap().max.x), (184.0, 200.0), "the first part at the right");
        assert_eq!((r[1].unwrap().min.x, r[1].unwrap().max.x), (130.0, 180.0), "the label after the gap, to its left");
    }
}
