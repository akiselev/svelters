//! WG6 (plans/milestones/WG6-widget-polish.md): hover affordances shared by every kind.
//!
//! - QA5: a node's `tooltip` shows on disabled widgets too (egui's `on_hover_text` hides there:
//!   [`text`]); a `tooltip` element is a rich tooltip (`crate::overlays`); `status-tip` feeds the
//!   app's status bar ([`status_tip`]).
//! - QA14: the `cursor` prop ([`hover`], [`cursor_icon`]). CSS `cursor` in the style engine is DS2's.
//! - QW13: `elide` on `label` ([`elide`]).
//! - QW9: `combo-box` type-ahead ([`combo_type_ahead`], on WG5a's `KeySelect::type_ahead`).
//! - QA13: `scroll-into-view` ([`scroll_into_view`], applied by the walk: [`reveal`]); a widget that
//!   takes focus scrolls itself into view, and a pass with Tab pressed culls nothing, so focus can
//!   move by Tab to a widget outside the visible part of a scroll area ([`keep`]).
//!
//! Writer lane: WIDGETS.

use crate::scene::{Arena, NodeFlags};
use crate::views::selection::{KeySelect, Mode};
use egui::{Color32, CursorIcon, FontId, Id, Rect, Response, Ui};

/// Shows `tip` on hover whether the widget is enabled or not; also its accessible description.
pub(crate) fn text(resp: Response, tip: &str) -> Response {
    if tip.is_empty() {
        return resp;
    }
    resp.on_hover_text(tip.to_owned()).on_disabled_hover_text(tip.to_owned())
}

/// After node `i` drew `rect` in `ui`: while the pointer is over it (enabled or not), its
/// `status-tip` becomes the status text and its `cursor` the mouse cursor. Children draw later,
/// so the innermost node wins; a child left at `auto` keeps its parent's cursor unless its widget
/// sets one.
pub(crate) fn hover(arena: &Arena, i: usize, ui: &Ui, rect: Rect) {
    let c = arena.common(i);
    if (c.status_tip.is_empty() && c.cursor == "auto") || rect == Rect::NOTHING || !rect.is_positive() {
        return;
    }
    hovered(arena, i, ui.ctx(), ui.rect_contains_pointer(rect));
}

/// [`hover`] for a node drawn by a widget of its own (menu entries): `over`, the pointer is on it.
pub(crate) fn hovered(arena: &Arena, i: usize, ctx: &egui::Context, over: bool) {
    let c = arena.common(i);
    if !over {
        return;
    }
    if !c.status_tip.is_empty() {
        set_status(ctx, &c.status_tip);
    }
    if let Some(icon) = cursor_icon(c.cursor) {
        ctx.set_cursor_icon(icon);
    }
}

#[derive(Clone)]
struct Status {
    pass: u64,
    text: String,
}

fn status_id() -> Id {
    Id::NULL.with("svelters-egui/status-tip")
}

fn set_status(ctx: &egui::Context, text: &str) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(status_id(), Status { pass, text: text.to_owned() }));
    // one more pass after the pointer leaves, so the reader sees the tip go away
    ctx.request_repaint_after(std::time::Duration::from_millis(150));
}

/// QA5: the `status-tip` of the node under the pointer, for the app's status bar (Qt's
/// `QStatusTipEvent`). Set during the pass that drew the node; read it after the pass, or in the
/// next pass before the panels draw (it is kept for one pass).
pub fn status_tip(ctx: &egui::Context) -> Option<String> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Status>(status_id())).filter(|s| s.pass + 1 >= now).map(|s| s.text)
}

/// QA13: marks node `i` to be scrolled into view when the walk next reaches it, and its ancestors
/// so the walk does not cull the way there.
pub(crate) fn scroll_into_view(arena: &mut Arena, i: usize, center: bool) {
    arena.core[i].flags.insert(NodeFlags::WANT_SCROLL);
    arena.core[i].flags.set(NodeFlags::SCROLL_CENTER, center);
    let mut p = arena.core[i].parent.and_then(|p| arena.idx(p));
    while let Some(k) = p {
        arena.core[k].flags.insert(NodeFlags::SCROLL_BELOW);
        p = arena.core[k].parent.and_then(|p| arena.idx(p));
    }
}

/// QA13: whether the walk paints node `i` although `r` is outside `ui`'s clip rect: a
/// `scroll-into-view` is pending below it, or Tab was pressed this pass (egui's focus moves only
/// to widgets drawn in the pass). Clears `SCROLL_BELOW`.
pub(crate) fn keep(arena: &mut Arena, i: usize, ui: &Ui) -> bool {
    let below = arena.core[i].flags.contains(NodeFlags::SCROLL_BELOW);
    arena.core[i].flags.remove(NodeFlags::SCROLL_BELOW);
    below || arena.core[i].flags.contains(NodeFlags::WANT_SCROLL) || ui.input(|i| i.key_pressed(egui::Key::Tab))
}

/// QA13: scrolls to node `i` (drawn at `r`) when `scroll-into-view` asked for it.
pub(crate) fn reveal(arena: &mut Arena, i: usize, ui: &Ui, r: Rect) {
    let f = &mut arena.core[i].flags;
    if f.contains(NodeFlags::WANT_SCROLL) {
        let align = f.contains(NodeFlags::SCROLL_CENTER).then_some(egui::Align::Center);
        f.remove(NodeFlags::WANT_SCROLL | NodeFlags::SCROLL_CENTER);
        ui.scroll_to_rect(r, align);
    }
}

/// QW9: the option a focused combo box (`id`, showing `current` of `options`) selects from the text
/// typed this pass, if any.
pub(crate) fn combo_type_ahead(ui: &Ui, id: Id, options: &[String], current: Option<usize>) -> Option<usize> {
    let text: String = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    });
    if text.is_empty() {
        return None;
    }
    let now = ui.input(|i| i.time);
    let key = id.with("type-ahead");
    // WG5a's `KeySelect` (the lists' and tables' rule), over the option indices
    let mut st = ui.ctx().data(|d| d.get_temp::<KeySelect>(key)).unwrap_or_else(|| KeySelect::new(Mode::Single));
    let ids: Vec<u64> = (0..options.len() as u64).collect();
    st.cursor = current.map(|k| k as u64);
    for ch in text.chars() {
        st.type_ahead(&ids, &ch.to_string(), now, |k| options[k as usize].clone());
    }
    let at = st.cursor.map(|k| k as usize);
    ui.ctx().data_mut(|d| d.insert_temp(key, st));
    at.filter(|&k| Some(k) != current)
}

/// QA14: the egui cursor for a CSS cursor name; `None` for `auto` (the widget's own).
pub fn cursor_icon(name: &str) -> Option<CursorIcon> {
    use CursorIcon as C;
    Some(match name {
        "default" => C::Default,
        "none" => C::None,
        "pointer" => C::PointingHand,
        "text" => C::Text,
        "vertical-text" => C::VerticalText,
        "move" => C::Move,
        "grab" => C::Grab,
        "grabbing" => C::Grabbing,
        "not-allowed" => C::NotAllowed,
        "no-drop" => C::NoDrop,
        "crosshair" => C::Crosshair,
        "cell" => C::Cell,
        "wait" => C::Wait,
        "progress" => C::Progress,
        "help" => C::Help,
        "context-menu" => C::ContextMenu,
        "alias" => C::Alias,
        "copy" => C::Copy,
        "all-scroll" => C::AllScroll,
        "zoom-in" => C::ZoomIn,
        "zoom-out" => C::ZoomOut,
        "col-resize" => C::ResizeColumn,
        "row-resize" => C::ResizeRow,
        "ew-resize" => C::ResizeHorizontal,
        "ns-resize" => C::ResizeVertical,
        "nesw-resize" => C::ResizeNeSw,
        "nwse-resize" => C::ResizeNwSe,
        "n-resize" => C::ResizeNorth,
        "e-resize" => C::ResizeEast,
        "s-resize" => C::ResizeSouth,
        "w-resize" => C::ResizeWest,
        "ne-resize" => C::ResizeNorthEast,
        "nw-resize" => C::ResizeNorthWest,
        "se-resize" => C::ResizeSouthEast,
        "sw-resize" => C::ResizeSouthWest,
        _ => return None,
    })
}

/// QW13: `text` cut to fit `width` with `…` at the start, middle or end (`mode`, an `elide`
/// value), measured in `font`. The text itself when it fits.
pub(crate) fn elide(ui: &Ui, text: &str, font: FontId, mode: &str, width: f32) -> String {
    let ctx = ui.ctx();
    elide_with(text, mode, width, |s| ctx.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x))
}

/// [`elide`] over any measure: the longest cut (by characters kept) whose width fits.
pub(crate) fn elide_with(text: &str, mode: &str, width: f32, measure: impl Fn(&str) -> f32) -> String {
    if mode == "none" || measure(text) <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = |n: usize| -> String {
        let (head, tail) = match mode {
            "start" => (0, n),
            "end" => (n, 0),
            _ => (n.div_ceil(2), n / 2),
        };
        let mut s: String = chars[..head].iter().collect();
        s.push('…');
        s.extend(&chars[chars.len() - tail..]);
        s
    };
    // the widest cut that fits: `fits(n)` is monotonic in n
    let (mut lo, mut hi) = (0usize, chars.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if measure(&cut(mid)) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn wg6_elide_modes() {
        let t = "/home/user/projects/part.step";
        assert_eq!(elide_with(t, "middle", 100.0, w), t);
        assert_eq!(elide_with(t, "none", 5.0, w), t);
        assert_eq!(elide_with(t, "middle", 11.0, w), "/home….step");
        assert_eq!(elide_with(t, "end", 6.0, w), "/home…");
        assert_eq!(elide_with(t, "start", 6.0, w), "….step");
        assert_eq!(elide_with(t, "middle", 0.0, w), "…");
        assert_eq!(elide_with("ab", "middle", 1.0, w), "…");
    }

    #[test]
    fn wg6_cursor_names() {
        assert_eq!(cursor_icon("auto"), None);
        assert_eq!(cursor_icon("pointer"), Some(CursorIcon::PointingHand));
        assert_eq!(cursor_icon("ew-resize"), Some(CursorIcon::ResizeHorizontal));
        for n in crate::kinds::CURSOR {
            assert_eq!(cursor_icon(n).is_none(), *n == "auto", "{n}");
        }
    }
}
