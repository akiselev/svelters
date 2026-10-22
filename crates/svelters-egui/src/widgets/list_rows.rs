//! The rows of `virtual-list` and `tree-view`, svelters' own (OW5a; ARCH-WIDGETS §6.1, DESIGN R6,
//! R8, R10, R14, WIDGET-PARITY §2.18). Lists stay `W::VirtualList` payloads (the virtualiser,
//! row pool and M5 protocol of `kinds/virtuallist.rs`); what they paint and how they scroll is
//! this module's and OW2's Scrollable, from CSS and UA tables, with no egui `Visuals`.
//!
//! **Rows are `::part(row)`** of the list (coordinator, 2026-10-22), one style per pooled row:
//! [`Scene::part_at`](crate::scene::Scene::part_at) matches the list's `::part(row)` rules with the
//! host's state replaced by the row's ([`RowState::bits`]) at the row's logical position (P1), so
//!
//! ```css
//! virtual-list::part(row):hover { … }                               /* hover fill */
//! virtual-list:focus-within::part(row):selected { … }               /* active selection */
//! virtual-list:not(:focus-within)::part(row):selected { … }         /* inactive selection */
//! virtual-list::part(row):focus-visible { outline: 1px solid … }    /* the cursor, keyboard */
//! virtual-list::part(row):nth-child(even) { … }                     /* stripes, by position */
//! tree-view::part(row):has-children:expanded { … }
//! ```
//!
//! work on rows that are recycled. The row nodes carry the same bits (`:selected`, `:current`,
//! `:expanded`, `:has-children`, `:drop-target`), so their contents restyle too
//! (`virtual-list:focus-within > :selected { color: … }`, base.css).
//!
//! R6: rows are flat (no shadow, no lip) with four distinct states: `list-hover`, `list-sel`
//! (+ `list-sel-fg`), `list-sel-inactive`, and R8's 1px `focus` outline at offset −1 on the
//! cursor row, keyboard only (a click does not show it). R10: `cursor: pointer`. Parts:
//! `row` (P, repeated), `twistie` (P, tree rows; R14 chevron), `placeholder` (P, a position not
//! bound yet), `drop-line` (P, the insertion line of a drag).

use super::scroll_area::{THUMB_UA, TRACK_UA};
use crate::interact::{BarGeom, ScrollParams, Scrollable, Unit};
use crate::native::Scrollbars;
use crate::paint::{self, PaintCx, StyleView, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::{NodeId, Scene};
use egui::{Color32, Rect, Stroke};

const SEL_ACTIVE: StateBits = StateBits(StateBits::SELECTED.0 | StateBits::FOCUS_WITHIN.0);

/// `::part(row)`: DESIGN R6's four states, R8's row focus, R10's pointer.
pub const ROW: Ua = Ua(&[
    UaRule::base(&[
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
        ("cursor", Keyword("pointer")),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("list-sel-inactive"))]),
    UaRule::when(SEL_ACTIVE, &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg"))]),
    UaRule::when(StateBits::DROP_TARGET, &[("background-color", Token("list-drop"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("cursor", Keyword("default"))]),
]);

/// `::part(twistie)`: a tree row's chevron (R14), VS Code's 16 px box.
pub const TWISTIE: Ua = Ua(&[UaRule::base(&[("width", Token("twistie-w")), ("color", Token("icon"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// `::part(placeholder)`: a position in view whose row is not bound yet (one frame).
pub const PLACEHOLDER: Ua = Ua(&[UaRule::base(&[("background-color", Token("list-hover")), ("border-radius", Px(2.0))])]);

/// `::part(drop-line)`: where a dragged row would land (between rows; around the list when it
/// lands at the end).
pub const DROP_LINE: Ua = Ua(&[UaRule::base(&[("background-color", Token("focus")), ("height", Px(2.0))])]);

/// One row's state, from the list's selection, cursor, focus and pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowState {
    pub hovered: bool,
    pub pressed: bool,
    pub selected: bool,
    /// The cursor row (`currentIndex`): `:current`.
    pub current: bool,
    /// The list has keyboard focus (the active selection colours).
    pub list_focused: bool,
    /// The list's focus came from the keyboard (R8: only then the cursor row shows its outline).
    pub keyboard: bool,
    /// A drag would drop into this row.
    pub drop_into: bool,
    /// Tree rows: `:has-children` (P5, bit 23) and `:expanded`.
    pub has_children: bool,
    pub expanded: bool,
    pub disabled: bool,
}

impl RowState {
    /// The bits `::part(row)` matches with (the host's, replaced).
    pub fn bits(&self) -> StateBits {
        let mut b = StateBits::NONE;
        b.set(StateBits::HOVERED, self.hovered);
        b.set(StateBits::PRESSED, self.pressed);
        b.set(StateBits::SELECTED, self.selected);
        b.set(StateBits::CURRENT, self.current);
        b.set(StateBits::FOCUS_WITHIN, self.list_focused);
        b.set(StateBits::FOCUSED, self.list_focused && self.current);
        b.set(StateBits::FOCUS_VISIBLE, self.list_focused && self.current && self.keyboard);
        b.set(StateBits::DROP_TARGET, self.drop_into);
        b.set(StateBits::HAS_CHILDREN, self.has_children);
        b.set(StateBits::EXPANDED, self.has_children && self.expanded);
        b.set(StateBits::DISABLED, self.disabled);
        b
    }
}

/// The bits a row node mirrors from its row (its contents' selectors).
const ROW_NODE_BITS: StateBits =
    StateBits(StateBits::SELECTED.0 | StateBits::CURRENT.0 | StateBits::DROP_TARGET.0 | StateBits::HAS_CHILDREN.0 | StateBits::EXPANDED.0);

/// Sets row node `ni`'s mirrored bits (keeping the walk's own: hover, focus).
pub(crate) fn mirror(s: &mut Scene, ni: usize, row: &RowState) {
    let before = s.core[ni].state;
    let want = StateBits((before.0 & !ROW_NODE_BITS.0) | (row.bits().0 & ROW_NODE_BITS.0));
    if want != before {
        crate::style::set_own_state(s, ni, want);
    }
}

fn dark(ui: &egui::Ui) -> bool {
    ui.ctx().theme() == egui::Theme::Dark
}

/// Part `part` of list `list` at `at` = (position, count) in state `row`: the style the row
/// paints with (the CSS part style over `ua`, inheriting from the list).
pub(crate) fn part_style(s: &mut Scene, list: NodeId, part: &'static str, row: StateBits, at: (usize, usize)) -> Option<std::sync::Arc<svelters_style::ComputedStyle>> {
    s.part_at(list, part, row, at)
}

/// What a painted row reports back.
pub(crate) struct Painted {
    /// The row's `cursor` (R10), to set while it is hovered.
    pub cursor: Option<egui::CursorIcon>,
}

/// Paints row `rect`'s box (`::part(row)`) of list `list` (scene index `li`): background, border,
/// shadows from CSS or [`ROW`]. The outline is [`paint_row_outline`], after the contents.
pub(crate) fn paint_row(s: &mut Scene, li: usize, row: &RowState, at: (usize, usize), rect: Rect, ui: &egui::Ui) -> Painted {
    let list = s.slots.id_at(li).expect("live list");
    let bits = row.bits();
    let css = part_style(s, list, "row", bits, at);
    let host_css = s.style.computed(list).cloned();
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), bits, dark(ui));
    let st = host.part(css.as_deref(), ROW);
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    paint::paint_box(&mut pc.part("row"), &st, rect);
    Painted { cursor: st.cursor() }
}

/// The row's outline (R8 focus ring, 1px inside), over its contents.
pub(crate) fn paint_row_outline(s: &mut Scene, li: usize, row: &RowState, at: (usize, usize), rect: Rect, ui: &egui::Ui) {
    let bits = row.bits();
    if !bits.contains(StateBits::FOCUS_VISIBLE) && !s.style.is_active() {
        return;
    }
    let list = s.slots.id_at(li).expect("live list");
    let css = part_style(s, list, "row", bits, at);
    let host_css = s.style.computed(list).cloned();
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), bits, dark(ui));
    let st = host.part(css.as_deref(), ROW);
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    paint::paint_outline(&mut pc.part("row"), &st, rect);
}

/// A tree row's twistie (R14) in `rect`, pointing right or down when `open`.
pub(crate) fn paint_twistie(s: &mut Scene, li: usize, row: &RowState, at: (usize, usize), rect: Rect, ui: &egui::Ui) {
    let list = s.slots.id_at(li).expect("live list");
    let bits = row.bits();
    let row_css = part_style(s, list, "row", bits, at);
    let tw_css = part_style(s, list, "twistie", bits, at);
    let host_css = s.style.computed(list).cloned();
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), bits, dark(ui));
    let row_v = host.part(row_css.as_deref(), ROW);
    let st = row_v.part(tw_css.as_deref(), TWISTIE);
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    let color = st.color("color").unwrap_or(Color32::GRAY);
    super::parts::paint_chevron(&mut pc.part("twistie"), rect, color, row.expanded);
}

/// The twistie's width (`::part(twistie)` `width`, VS Code's 16).
pub(crate) fn twistie_width(s: &Scene, li: usize, ui: &egui::Ui) -> f32 {
    let list = s.slots.id_at(li).expect("live list");
    let host = StyleView::new(s.style.computed(list).map(|c| &**c), Ua(&[]), StateBits::NONE, dark(ui));
    host.part(s.style.part_computed(list, "twistie").map(|c| &**c), TWISTIE).px("width").unwrap_or(16.0)
}

/// A placeholder for an unbound position.
pub(crate) fn paint_placeholder(s: &Scene, li: usize, rect: Rect, ui: &egui::Ui) {
    let list = s.slots.id_at(li).expect("live list");
    let host = StyleView::new(s.style.computed(list).map(|c| &**c), Ua(&[]), StateBits::NONE, dark(ui));
    let st = host.part(s.style.part_computed(list, "placeholder").map(|c| &**c), PLACEHOLDER);
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    paint::paint_box(&mut pc.part("placeholder"), &st, rect.shrink(2.0));
}

/// Where a drag would land: a line above or below `row` (`::part(drop-line)`), or around the
/// list's `area` when `row` is `None`.
pub(crate) fn paint_drop_line(s: &Scene, li: usize, area: Rect, row: Option<Rect>, below: bool, ui: &egui::Ui) {
    let list = s.slots.id_at(li).expect("live list");
    let host = StyleView::new(s.style.computed(list).map(|c| &**c), Ua(&[]), StateBits::NONE, dark(ui));
    let st = host.part(s.style.part_computed(list, "drop-line").map(|c| &**c), DROP_LINE);
    let color = st.color("background-color").unwrap_or(Color32::LIGHT_BLUE);
    let h = st.px("height").unwrap_or(2.0).max(1.0);
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    let mut pc = pc.part("drop-line");
    match row {
        Some(rr) => {
            let y = if below { rr.max.y } else { rr.min.y };
            let r = pc.snap_rect(Rect::from_x_y_ranges(rr.x_range(), (y - h / 2.0)..=(y + h / 2.0)));
            pc.painter.rect_filled(r, 0.0, color);
            pc.record(TraceKind::Box, r, color, Stroke::NONE, 0.0);
        }
        None => {
            let stroke = Stroke::new(pc.snap_width(h), color);
            pc.painter.rect_stroke(area, 0.0, stroke, egui::StrokeKind::Inside);
            pc.record(TraceKind::Box, area, Color32::TRANSPARENT, stroke, 0.0);
        }
    }
}

fn tag_of(s: &Scene, li: usize) -> &'static str {
    match s.w(li) {
        crate::scene::W::VirtualList(v) if v.tree.is_some() => "tree-view",
        _ => "virtual-list",
    }
}

// ---- the scroll bar: OW2's Scrollable (replaces the list's own 8 pt bar) ----

/// A list's vertical bar: OW2's [`Scrollable`] over the virtualiser's offset. The list keeps its
/// own offset ([`VScroll`](crate::scene::virt::VScroll), decided before the rows are bound, R22):
/// the unit gets it each frame and what it moves (thumb drag, track press, the wheel the list did
/// not take, kinetic, AccessKit) goes back as a pending delta. The look is a scroll area's bar
/// (`::part(scrollbar-y)`, `::part(thumb-y)`, overlay with fade per `native::Policy`), and the
/// list gets R15's `:state(sv.overflow-top)`.
#[derive(Debug, Default)]
pub(crate) struct ListBar {
    scroll: Scrollable,
    thumb: StateBits,
}

impl ListBar {
    /// Runs the bar on list `li` (rect `r`, response `resp`) at `offset` of `total` points: returns
    /// the offset change it asks for.
    pub(crate) fn run(&mut self, s: &mut Scene, li: usize, ui: &egui::Ui, resp: &egui::Response, r: Rect, offset: f64, total: f64) -> f64 {
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        let policy = crate::native::policy_of(&ctx);
        let d = dark(ui);
        let content = egui::vec2(r.width(), total as f32);
        // the unit works in f32: its change is measured against the value it was given
        let given = offset as f32;
        self.scroll.set_offset(egui::vec2(0.0, given));
        self.scroll.set_geometry(content, r.size(), [None, None]);
        let bar_w = StyleView::ua_only(TRACK_UA, StateBits::NONE, d).px("width").unwrap_or(10.0);
        let min_thumb = StyleView::ua_only(THUMB_UA, StateBits::NONE, d).px("min-height").unwrap_or(20.0);
        let bars = self.scroll.bar_geometry(r, bar_w, min_thumb, [false, self.scroll.overflows(1)]);
        self.scroll.set_geometry(content, r.size(), bars);
        let mode = crate::interact::scroll::bar_mode(None, policy.scrollbars);
        let alpha = match mode {
            Scrollbars::Overlay => self.scroll.bar_alpha(now, &policy.scroll),
            Scrollbars::Classic => 1.0,
        };
        let enabled = ui.is_enabled();
        let mut parts = crate::interact::PartResponses::default();
        let id = resp.id;
        if let Some(b) = bars[1]
            && enabled
            && (alpha > 0.0 || resp.contains_pointer())
        {
            let tr = ui.interact(b.track, id.with("scrollbar-y"), egui::Sense::click_and_drag());
            let th = ui.interact(b.thumb, id.with("thumb-y"), egui::Sense::drag());
            parts.0.push(("scrollbar-y", crate::interact::Resp::of(&tr)));
            parts.0.push(("thumb-y", crate::interact::Resp::of(&th)));
        }
        let params = ScrollParams { axes: [false, true], chain: true, page_keys: false, arrow_keys: false };
        let mut fired = crate::interact::Intents::default();
        let mut state = s.core[li].state;
        {
            let sc = &mut self.scroll;
            crate::interact::run(ui, resp, &mut state, &parts, |icx| sc.input(&params, icx, &mut fired));
        }
        // WG6 QA9: a drag held near the top or bottom edge scrolls
        if enabled && let Some(a) = crate::kinds::dnd::auto_scroll(&ctx, r) {
            self.scroll.scroll_by(&params, egui::vec2(0.0, a.y));
            ctx.request_repaint();
        }
        crate::style::set_own_state(s, li, state);
        let consumed = self.scroll.consumed();
        if consumed != egui::Vec2::ZERO {
            ctx.input_mut(|i| i.smooth_scroll_delta -= consumed);
        }
        let t = parts.get("thumb-y");
        self.thumb.set(StateBits::HOVERED, t.is_some_and(|t| t.hovered));
        self.thumb.set(StateBits::PRESSED, t.is_some_and(|t| t.down_on));
        self.thumb.set(StateBits::DRAGGING, self.scroll.dragging(1));
        if !fired.is_empty() {
            ctx.request_repaint();
        }
        if let Some(t) = self.scroll.next_frame() {
            ctx.request_repaint_after(t);
        }
        if mode == Scrollbars::Overlay
            && let Some(t) = self.scroll.bar_repaint(now, &policy.scroll)
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(t.max(0.0)));
        }
        self.paint(s, li, ui, &bars, alpha);
        (self.scroll.offset().y - given) as f64
    }

    fn paint(&self, s: &mut Scene, li: usize, ui: &egui::Ui, bars: &[Option<BarGeom>; 2], alpha: f32) {
        let Some(b) = bars[1] else { return };
        let list = s.slots.id_at(li).expect("live list");
        let d = dark(ui);
        // AccessKit: the bar is a `ScrollBar` with its value
        let (v, max) = (self.scroll.offset().y as f64, self.scroll.max_offset().y as f64);
        ui.ctx().accesskit_node_builder(s.core[li].egui_id.with("scrollbar-y"), |n| {
            n.set_role(egui::accesskit::Role::ScrollBar);
            n.set_orientation(egui::accesskit::Orientation::Vertical);
            n.set_numeric_value(v);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(max);
        });
        if alpha <= 0.0 {
            return;
        }
        let track_css = part_style(s, list, "scrollbar-y", StateBits::NONE, (0, 1));
        let thumb_css = part_style(s, list, "thumb-y", self.thumb, (0, 1));
        let host_css = s.style.computed(list).cloned();
        let host = StyleView::new(host_css.as_deref(), Ua(&[]), StateBits::NONE, d);
        let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
        pc.tag = tag_of(s, li);
        let track = host.part(track_css.as_deref(), TRACK_UA);
        let fill = track.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
        let mut tp = pc.part("scrollbar-y");
        if fill.a() > 0 {
            tp.painter.rect_filled(b.track, 0.0, fill);
        }
        tp.record(TraceKind::Box, b.track, fill, Stroke::NONE, 0.0);
        let thumb = host.part(thumb_css.as_deref(), THUMB_UA).in_state(self.thumb);
        let fill = thumb.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
        let radius = thumb.px("border-radius").unwrap_or(0.0);
        let mut tp = pc.part("thumb-y");
        tp.painter.rect_filled(b.thumb, radius, fill);
        tp.record(TraceKind::Box, b.thumb, fill, Stroke::NONE, 0.0);
    }
}

/// R15: the list's inset edge shadows (`:state(sv.overflow-top)`, base.css), over its rows.
pub(crate) fn paint_shadows(s: &Scene, li: usize, r: Rect, ui: &egui::Ui) {
    let list = s.slots.id_at(li).expect("live list");
    let Some(css) = s.style.computed(list) else { return };
    let host = StyleView::new(Some(&**css), Ua(&[]), s.core[li].state, dark(ui));
    let mut pc = PaintCx::for_node(ui.painter(), list.to_raw());
    pc.tag = tag_of(s, li);
    paint::paint_inset_shadows(&mut pc, &host, r);
}

/// EV1: the list's event `name` through the Rust filters and behaviours on its path before the
/// list's default action (moving the selection, expanding, activating); false when one of them
/// prevented it. The JS event itself is emitted by the caller as before.
pub(crate) fn allowed(s: &mut Scene, li: usize, name: &'static str, ctx: &egui::Context, out: &mut Vec<crate::walk::Out>) -> bool {
    let node = s.slots.id_at(li).expect("live list");
    let (o, runs) = crate::events::dispatch_signal(s, node, name);
    for r in runs {
        crate::provider::behavior::apply(s, r, Some(ctx), out);
    }
    !o.prevented()
}
