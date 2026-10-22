//! WIDGET-PARITY P3: paint parts laid out from CSS inside the host's content box.
//!
//! The parts are flex items of the host's content box: the host's `flex-direction` (`row`
//! default, `column`, `row-reverse`, `column-reverse`), `gap` and `justify-content` (`start`,
//! `center`, `end`), each part's `order`, `width`/`height` (else its content size), `margin` and
//! `display: none`. Cross axis: centred. So `button { flex-direction: column }` puts the label
//! under the icon (Qt's TextUnderIcon) and `button::part(label) { display: none }` gives an
//! icon-only button, with no Rust per layout. Node parts (OW1b on) are laid out by taffy instead.

use super::StyleView;
use egui::{Rect, Vec2, pos2, vec2};

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
    let gap = host.px("gap").unwrap_or(0.0);
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&i| parts[i].style.number("order").unwrap_or(0.0) as i32);
    if reverse {
        order.reverse();
    }
    let need = measure_parts(host, parts);
    let (avail, need_main) = if row { (content.width(), need.x) } else { (content.height(), need.y) };
    let start = match host.keyword("justify-content") {
        Some("center") => (avail - need_main) / 2.0,
        Some("end" | "flex-end") => avail - need_main,
        _ => 0.0,
    };
    let mut out = vec![None; parts.len()];
    let mut at = start;
    for i in order {
        let Some((size, m)) = outer(&parts[i]) else { continue };
        let inner = vec2(size.x - m[0] - m[1], size.y - m[2] - m[3]);
        let r = if row {
            let y = content.center().y - inner.y / 2.0 + (m[2] - m[3]) / 2.0;
            Rect::from_min_size(pos2(content.left() + at + m[0], y), inner)
        } else {
            let x = content.center().x - inner.x / 2.0 + (m[0] - m[1]) / 2.0;
            Rect::from_min_size(pos2(x, content.top() + at + m[2]), inner)
        };
        out[i] = Some(r);
        at += if row { size.x } else { size.y } + gap;
    }
    out
}
