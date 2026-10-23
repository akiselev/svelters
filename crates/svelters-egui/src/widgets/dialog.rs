//! OW7: svelters' own `modal` and `inner-window` (ARCH-WIDGETS §3.2, §6.1; WIDGET-PARITY §2.21;
//! DESIGN R7, R10). No egui `Modal`, `Window` or `Frame`: egui supplies the Area, picking, focus
//! memory and the modal layer.
//!
//! - **modal**: one `egui::Area` at `Order::Foreground` over the whole viewport. Its first part is
//!   the backdrop (`::part(backdrop)`, sensing clicks, so nothing below is reached; egui's modal
//!   layer also keeps keyboard focus and hover above it). The dialog box sits centred on it in
//!   R7's `elev-4` look (`surface-raised`, a 1px stroke, `window-radius`, `window-shadow`), with an
//!   optional `::part(title)` above the content. Escape or a press on the backdrop closes it when
//!   `dismissable` (Escape is checked after the content ran, so a select or menu inside that
//!   takes it closes first). `resizable` adds the right and bottom edges and a `::part(grip)`
//!   (Qt's `sizeGripEnabled`). AccessKit `Dialog`, modal, named by its title.
//! - **inner-window**: an Area at `Order::Middle` (above the window's content, below popups),
//!   with `::part(titlebar)` (drag to move; double click collapses when `collapsible`),
//!   `::part(collapse)`, `::part(title)`, `::part(close)` and, when `resizable`, eight
//!   `::part(edge-*)` handles driven by OW2's [`Resizable`]. A press brings it to the top;
//!   `:backdrop` is on while another inner window is above it. AccessKit `Window`.
//!
//! Both are EV3 dialog focus scopes (`crate::overlays::draw_content`): focus moves in when they
//! open (`autofocus`, else the first field), Tab stays inside, Enter and Escape reach the scope's
//! `default` and `cancel` buttons, and focus goes back to the opener when they close.
//!
//! Writer lane: OW7.

use super::glyph::{self, Glyph};
use super::popup::{css, dark, part_css};
use crate::interact::anchor;
use crate::interact::dismiss::{DismissParams, Dismissable, Outside, Reason};
use crate::interact::{self, Intents, PartResponses, Resizable, Resize, ResizeParams, Unit};
use crate::overlays::OverlayW;
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::Arena;
use crate::walk::Out;
use egui::mutex::Mutex;
use egui::{Color32, CursorIcon, Id, LayerId, Order, Pos2, Rect, Sense, Ui, Vec2, vec2};
use std::sync::Arc;

/// A dialog or floating window at rest (DESIGN R7, `elev-4`): `surface-raised` (`menu-bg`), a 1px
/// `menu-border-color` stroke, `window-radius`, `window-shadow`. The padding is around the content
/// (the node's own `padding` prop is taffy's, inside it).
pub const DIALOG_UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("window-radius")),
    ("box-shadow", Token("window-shadow")),
    ("padding", Px(12.0)),
    ("color", Token("fg")),
])]);

/// An inner window: [`DIALOG_UA`] with a smaller content inset (the title bar takes the top).
pub const WINDOW_UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("window-radius")),
    ("box-shadow", Token("window-shadow")),
    ("padding", Px(8.0)),
    ("color", Token("fg")),
])]);

/// `modal::part(backdrop)`: the dim over the viewport (VS Code's dialog block, `rgba(0,0,0,.3)`).
pub const BACKDROP_UA: Ua = Ua(&[UaRule::base(&[("background-color", Color(Color32::from_rgba_premultiplied(0, 0, 0, 77)))])]);

/// `::part(title)`: the dialog's or window's caption (semibold, `fg-strong`).
pub const TITLE_UA: Ua = Ua(&[UaRule::base(&[("font-weight", Number(600.0)), ("color", Token("fg-strong")), ("padding", Px(12.0)), ("padding-bottom", Px(0.0))])]);

/// `inner-window::part(titlebar)`: 30 tall; the title starts 8 in. `:backdrop` (not the top
/// window) mutes the title.
pub const TITLEBAR_UA: Ua = Ua(&[
    UaRule::base(&[("height", Px(30.0)), ("padding-left", Px(8.0)), ("padding-right", Px(4.0)), ("cursor", Keyword("default"))]),
    UaRule::when(StateBits::BACKDROP, &[("color", Token("fg-muted"))]),
]);

/// `::part(close)` and `::part(collapse)` (and a toast's close): 22px toolbar buttons (R3 hover,
/// R2 press, R10 pointer, R8 a keyboard-only 1px `focus` ring at -1 as VS Code's toolbar actions).
pub const TOOL_UA: Ua = Ua(&[
    UaRule::base(&[("width", Px(22.0)), ("height", Px(22.0)), ("border-radius", Px(4.0)), ("font-size", Px(16.0)), ("cursor", Keyword("pointer")), ("color", Token("fg"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("toolbar-active"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid")), ("outline-width", Px(1.0)), ("outline-color", Token("focus")), ("outline-offset", Px(-1.0))]),
]);

/// `modal::part(grip)`: the size grip in the bottom-right corner (12px, muted).
pub const GRIP_UA: Ua = Ua(&[UaRule::base(&[("width", Px(12.0)), ("height", Px(12.0)), ("color", Token("fg-muted"))])]);

/// The resize handles: (part, the (axis, sign) of each [`Resizable`] it drives, cursor).
type EdgeDef = (&'static str, [Option<(usize, f32)>; 2], CursorIcon);
const EDGES: [EdgeDef; 8] = [
    ("edge-right", [Some((0, 1.0)), None], CursorIcon::ResizeHorizontal),
    ("edge-bottom", [Some((1, 1.0)), None], CursorIcon::ResizeVertical),
    ("edge-bottom-right", [Some((0, 1.0)), Some((1, 1.0))], CursorIcon::ResizeNwSe),
    ("edge-left", [Some((0, -1.0)), None], CursorIcon::ResizeHorizontal),
    ("edge-top", [Some((1, -1.0)), None], CursorIcon::ResizeVertical),
    ("edge-bottom-left", [Some((0, -1.0)), Some((1, 1.0))], CursorIcon::ResizeNeSw),
    ("edge-top-right", [Some((0, 1.0)), Some((1, -1.0))], CursorIcon::ResizeNeSw),
    ("edge-top-left", [Some((0, -1.0)), Some((1, -1.0))], CursorIcon::ResizeNwSe),
];
/// The modal's handles (Qt's size grip: right, bottom and the corner).
const MODAL_EDGES: [usize; 3] = [0, 1, 2];
/// The hit thickness of an edge (straddling the border) and the side of a corner.
const EDGE_HIT: f32 = 6.0;
const CORNER: f32 = 12.0;
/// The smallest a user resize makes the box.
const MIN_SIZE: Vec2 = vec2(120.0, 60.0);

#[derive(Debug, Default)]
struct Edge {
    units: [Resizable; 2],
    state: StateBits,
}

/// What a dialog or window keeps between passes (not a prop): where the user put it, its size
/// after a user resize, collapsed, the handles' units.
#[derive(Debug, Default)]
pub(crate) struct Run {
    pos: Option<Pos2>,
    size: Option<Vec2>,
    collapsed: bool,
    edges: [Edge; 8],
    close: StateBits,
    collapse: StateBits,
}

/// [`Run`] in the overlay's payload (shared by its clones; never part of prop equality).
#[derive(Clone, Default)]
pub struct DialogRun(Arc<Mutex<Run>>);

impl std::fmt::Debug for DialogRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DialogRun")
    }
}

impl PartialEq for DialogRun {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The rect of handle `k` of `frame`.
fn edge_rect(frame: Rect, k: usize) -> Rect {
    let h = EDGE_HIT / 2.0;
    let (l, r, t, b) = (frame.left(), frame.right(), frame.top(), frame.bottom());
    match EDGES[k].0 {
        "edge-right" => Rect::from_min_max(egui::pos2(r - h, t + CORNER), egui::pos2(r + h, b - CORNER)),
        "edge-left" => Rect::from_min_max(egui::pos2(l - h, t + CORNER), egui::pos2(l + h, b - CORNER)),
        "edge-bottom" => Rect::from_min_max(egui::pos2(l + CORNER, b - h), egui::pos2(r - CORNER, b + h)),
        "edge-top" => Rect::from_min_max(egui::pos2(l + CORNER, t - h), egui::pos2(r - CORNER, t + h)),
        "edge-bottom-right" => Rect::from_min_max(egui::pos2(r - CORNER, b - CORNER), egui::pos2(r + h, b + h)),
        "edge-bottom-left" => Rect::from_min_max(egui::pos2(l - h, b - CORNER), egui::pos2(l + CORNER, b + h)),
        "edge-top-right" => Rect::from_min_max(egui::pos2(r - CORNER, t - h), egui::pos2(r + h, t + CORNER)),
        _ => Rect::from_min_max(egui::pos2(l - h, t - h), egui::pos2(l + CORNER, t + CORNER)),
    }
}

/// Runs the resize handles `which` of `frame` (OW2's [`Resizable`], one per axis a handle moves);
/// a left or top handle keeps the opposite edge still. Returns the frame after this pass's
/// resize, if any.
fn edges(ui: &Ui, id: Id, run: &mut Run, frame: Rect, bounds: Rect, which: &[usize]) -> Option<Rect> {
    let mut f = frame;
    let mut moved = false;
    for &k in which {
        let (name, axes, cursor) = EDGES[k];
        let resp = ui.interact(edge_rect(frame, k), id.with(name), Sense::CLICK | Sense::DRAG);
        let e = &mut run.edges[k];
        let units = &mut e.units;
        let mut fired: Vec<(usize, f32, Resize)> = Vec::new();
        interact::run(ui, &resp, &mut e.state, &PartResponses::default(), |cx| {
            for (j, a) in axes.iter().enumerate() {
                let Some((axis, sign)) = *a else { continue };
                let unit = &mut units[j];
                unit.set_size(frame.size()[axis]);
                let p = ResizeParams { axis, sign, min: MIN_SIZE[axis], max: bounds.size()[axis], step: 10.0, collapsible: false, double_click: false };
                let mut out = Intents::default();
                unit.input(&p, cx, &mut out);
                fired.extend(out.iter().map(|r| (axis, sign, *r)));
            }
        });
        if resp.hovered() || resp.dragged() || units.iter().any(|u| u.dragging()) {
            ui.ctx().set_cursor_icon(cursor);
        }
        for (axis, sign, r) in fired {
            let (Resize::Resize { size } | Resize::Cancel { size }) = r else { continue };
            if sign < 0.0 {
                // the far edge stays where it is
                let far = f.max[axis];
                f.min[axis] = far - size;
            } else {
                f.max[axis] = f.min[axis] + size;
            }
            moved = true;
        }
    }
    if moved {
        run.pos = Some(f.min);
        run.size = Some(f.size());
        ui.ctx().request_repaint();
        Some(f)
    } else {
        None
    }
}

/// `r` moved into `bounds` as far as it fits (its top-left wins when it is bigger).
fn clamp_into(r: Rect, bounds: Rect) -> Rect {
    let mut d = Vec2::ZERO;
    for a in 0..2 {
        if r.max[a] > bounds.max[a] {
            d[a] = bounds.max[a] - r.max[a];
        }
        if r.min[a] + d[a] < bounds.min[a] {
            d[a] = bounds.min[a] - r.min[a];
        }
    }
    r.translate(d)
}

/// How a dialog or window was closed: its `closed` result (`"reject"` for a user dismissal).
pub(crate) const REJECT: &str = "reject";

/// The content inset of a box styled `s` over `ua`: (lead: padding + border at the top-left,
/// extra: both sides).
fn inset(s: &StyleView<'_>, ua: Ua, state: StateBits) -> (Vec2, Vec2) {
    let u = StyleView::ua_only(ua, state, s.is_dark());
    let [pl, pr, pt, pb] = u.sides("padding");
    let [bl, br, bt, bb] = s.sides("border-width");
    (vec2(pl + bl, pt + bt), vec2(pl + pr + bl + br, pt + pb + bt + bb))
}

/// Paints open `modal` overlay `i` (see the module docs). Returns the close result when it closes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn modal(arena: &mut Arena, i: usize, origin: Vec2, o: &OverlayW, enabled: bool, ui: &mut Ui, out: &mut Vec<Out>) -> Option<&'static str> {
    let ctx = ui.ctx().clone();
    let node = arena.slots.id_at(i)?;
    let id = arena.core[i].egui_id.with("own-modal");
    let layer = LayerId::new(Order::Foreground, id);
    let bounds = anchor::bounds(&ctx);
    let mut state = arena.core[i].state;
    state.set(StateBits::OPEN, true);
    state.set(StateBits::DISABLED, !enabled);
    let dk = dark(&ctx);
    let sheet = css(arena, node);
    let title_css = part_css(arena, node, "title");
    let s = StyleView::new(sheet.as_deref(), DIALOG_UA, state, dk);
    let (lead, extra) = inset(&s, DIALOG_UA, state);
    let ts = s.part(title_css.as_deref(), TITLE_UA);
    let title = (!o.title.is_empty()).then(|| paint::text_galley(&ctx, &ts, &o.title, Some(bounds.width() * 0.8)));
    let [tl, tr, tt, tb] = ts.sides("padding");
    let (title_w, title_h) = title.as_ref().map_or((0.0, 0.0), |g| (g.size().x + tl + tr, g.size().y + tt + tb));
    let top = vec2(0.0, title_h);
    let run_arc = o.run.0.clone();
    let mut run = run_arc.lock();
    let user_size = if o.resizable { run.size } else { None };
    let definite = user_size.map(|sz| {
        let c = sz - extra - top;
        [Some(c.x.max(0.0)), Some(c.y.max(0.0))]
    });
    let guess = bounds.center();
    let size = definite.unwrap_or([None, None]);
    let content = crate::overlays::lay_out(arena, i, origin, &ctx, guess + lead + top, size);
    let fsize = user_size.unwrap_or(vec2(content.x.max(title_w) + extra.x, content.y + extra.y + title_h)).round();
    let at = match run.pos {
        Some(p) if o.resizable => p,
        _ => bounds.center() - fsize / 2.0,
    };
    let frame = clamp_into(Rect::from_min_size(at.round(), fsize), bounds);
    if frame.min != guess {
        crate::overlays::lay_out(arena, i, origin, &ctx, frame.min + lead + top, size);
    }
    // a press on the backdrop (this layer, outside the box) dismisses it
    let dp = DismissParams { inside: vec![frame], on_outside: if o.dismissable { Outside::Close } else { Outside::Ignore }, escape: false };
    if let Some(d) = ctx.input(|inp| Dismissable::check(&dp, inp))
        && d.reason == Reason::OutsidePress
        && d.at.is_some_and(|at| ctx.layer_id_at(at) == Some(layer))
    {
        arena.core[i].drawn = Rect::NOTHING;
        return Some(REJECT);
    }
    if state != arena.core[i].state {
        crate::style::set_own_state(arena, i, state);
    }
    let backdrop_css = part_css(arena, node, "backdrop");
    let grip_css = part_css(arena, node, "grip");
    let mut resized = None;
    super::popup::area(&ctx, id, Order::Foreground, bounds, |aui| {
        if !enabled {
            aui.disable();
        }
        // the backdrop takes every press that is not the dialog's
        aui.interact(bounds, id.with("backdrop"), Sense::click());
        let s = StyleView::new(sheet.as_deref(), DIALOG_UA, state, dark(aui.ctx()));
        let mut pc = paint::PaintCx::for_node(aui.painter(), node.to_raw());
        pc.tag = "modal";
        let bs = s.part(backdrop_css.as_deref(), BACKDROP_UA);
        paint::paint_box(&mut pc.part("backdrop"), &bs, bounds);
        paint::paint_box(&mut pc, &s, frame);
        // AccessKit: a modal Dialog named by its title
        let surface = aui.interact(frame, id.with("surface"), Sense::hover());
        surface.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Dialog, enabled, o.title.as_str()));
        aui.ctx().accesskit_node_builder(surface.id, |n| n.set_modal());
        if let Some(g) = &title {
            let r = Rect::from_min_size(frame.min + vec2(tl, tt), g.size());
            let ts = s.part(title_css.as_deref(), TITLE_UA);
            paint::paint_text(&mut pc.part("title"), &ts, g, r, Align::Start);
        }
        if o.resizable {
            let g = Rect::from_min_max(frame.max - vec2(14.0, 14.0), frame.max - vec2(2.0, 2.0));
            let gs = s.part(grip_css.as_deref(), GRIP_UA);
            paint_grip(&mut pc.part("grip"), &gs, g);
            resized = edges(aui, id, &mut run, frame, bounds, &MODAL_EDGES);
        }
        drop(pc);
        crate::overlays::draw_content(arena, i, origin, aui, out, Rect::from_min_max(frame.min + lead + top, frame.max - (extra - lead)));
    });
    drop(run);
    arena.core[i].drawn = frame;
    ctx.memory_mut(|m| m.set_modal_layer(layer));
    // Escape last: a select, menu or popup inside took it first if it was theirs
    if o.dismissable && ctx.input(|inp| inp.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Escape, pressed: true, modifiers, .. } if !modifiers.any()))) {
        return Some(REJECT);
    }
    if resized.is_some() {
        ctx.request_repaint();
    }
    None
}

/// The size grip: three diagonal dot rows in the part's `color`.
fn paint_grip(cx: &mut paint::PaintCx<'_>, s: &StyleView<'_>, r: Rect) {
    if s.keyword("display") == Some("none") {
        return;
    }
    let c = s.color("color").unwrap_or(Color32::GRAY);
    let step = r.width() / 3.0;
    for row in 0..3 {
        for col in (2 - row)..3 {
            let p = r.min + vec2(step * (col as f32 + 0.5), step * (row as f32 + 0.5));
            cx.painter.circle_filled(p, 1.0, c);
        }
    }
    cx.record(paint::TraceKind::Path, r, c, egui::Stroke::NONE, r.width());
}

/// A small button part (`close`, `collapse`; a toast's close): hover, press and focus-visible bits
/// (kept in `bits` between passes: focus from a press shows no ring, R8), its glyph or `text`, its
/// AccessKit button named `label`; true when clicked (pointer, Enter or Space, AccessKit).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tool(ui: &Ui, pc: &mut paint::PaintCx<'_>, host: &StyleView<'_>, css: Option<&svelters_style::ComputedStyle>, part: &'static str, bits: &mut StateBits, r: Rect, id: Id, g: Glyph, label: &str, expanded: Option<bool>) -> bool {
    tool_with(ui, pc, host.part(css, TOOL_UA), part, bits, r, id, Some(g), None, label, expanded)
}

/// [`tool`] with its look given (`ps`, unstated), a glyph and/or a text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tool_with(ui: &Ui, pc: &mut paint::PaintCx<'_>, ps: StyleView<'_>, part: &'static str, bits: &mut StateBits, r: Rect, id: Id, g: Option<Glyph>, text: Option<&std::sync::Arc<egui::Galley>>, label: &str, expanded: Option<bool>) -> bool {
    let resp = ui.interact(r, id.with(part), Sense::click());
    bits.set(StateBits::HOVERED, resp.hovered());
    bits.set(StateBits::PRESSED, resp.is_pointer_button_down_on());
    bits.set(StateBits::FOCUSED, resp.has_focus());
    if resp.is_pointer_button_down_on() || !resp.has_focus() {
        bits.set(StateBits::FOCUS_VISIBLE, false);
    } else if resp.gained_focus() {
        bits.set(StateBits::FOCUS_VISIBLE, true);
    }
    let ps = ps.in_state(*bits);
    if resp.hovered()
        && let Some(c) = ps.cursor()
    {
        ui.ctx().set_cursor_icon(c);
    }
    let mut cx = pc.part(part);
    paint::paint_box(&mut cx, &ps, r);
    if let Some(g) = g {
        glyph::paint(&mut cx, &ps, g, r);
    }
    if let Some(t) = text {
        paint::paint_text(&mut cx, &ps, t, Rect::from_center_size(r.center(), t.size()), Align::Center);
    }
    paint::paint_outline(&mut cx, &ps, r);
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Button, enabled, label);
        w.selected = expanded;
        w
    });
    resp.clicked()
}

/// Paints open `inner-window` overlay `i` (see the module docs). Returns the close result when it
/// closes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn window(arena: &mut Arena, i: usize, origin: Vec2, o: &OverlayW, enabled: bool, ui: &mut Ui, out: &mut Vec<Out>) -> Option<&'static str> {
    let ctx = ui.ctx().clone();
    let node = arena.slots.id_at(i)?;
    let id = arena.core[i].egui_id.with("own-window");
    let layer = LayerId::new(Order::Middle, id);
    let bounds = anchor::bounds(&ctx);
    let top_layer = ctx.memory(|m| m.areas().top_layer_id(Order::Middle));
    let mut state = arena.core[i].state;
    state.set(StateBits::OPEN, true);
    state.set(StateBits::DISABLED, !enabled);
    state.set(StateBits::BACKDROP, top_layer.is_some_and(|t| t != layer));
    let dk = dark(&ctx);
    let sheet = css(arena, node);
    let [bar_css, title_css, close_css, collapse_css] = ["titlebar", "title", "close", "collapse"].map(|p| part_css(arena, node, p));
    let s = StyleView::new(sheet.as_deref(), WINDOW_UA, state, dk);
    let (lead, extra) = inset(&s, WINDOW_UA, state);
    let bs = s.part(bar_css.as_deref(), TITLEBAR_UA);
    let bar_h = bs.px("height").unwrap_or(30.0);
    let top = vec2(0.0, bar_h);
    let run_arc = o.run.0.clone();
    let mut run = run_arc.lock();
    let collapsed = o.collapsible && run.collapsed;
    // the title's width (the window is at least that wide)
    let ts = bs.part(title_css.as_deref(), TITLE_UA);
    let title = paint::text_galley(&ctx, &ts, &o.title, None);
    let [bl, br, _, _] = bs.sides("padding");
    let buttons = 22.0 * (o.closable as u8 + o.collapsible as u8) as f32 + 8.0;
    let title_w = title.size().x + bl + br + buttons;
    let fixed = o.resizable.then(|| run.size.unwrap_or(vec2(o.default_w as f32, o.default_h as f32)));
    let guess = run.pos.unwrap_or(bounds.min);
    let fsize = if collapsed {
        vec2(fixed.map_or(title_w + extra.x, |f| f.x), bar_h + 2.0 * s.sides("border-width")[2])
    } else if let Some(f) = fixed {
        let c = f - extra - top;
        crate::overlays::lay_out(arena, i, origin, &ctx, guess + lead + top, [Some(c.x.max(0.0)), Some(c.y.max(0.0))]);
        f
    } else {
        let c = crate::overlays::lay_out(arena, i, origin, &ctx, guess + lead + top, [None, None]);
        vec2(c.x.max(title_w) + extra.x, c.y + extra.y + bar_h)
    }
    .round();
    let at = run.pos.unwrap_or_else(|| match (o.default_x, o.default_y) {
        (Some(x), Some(y)) => egui::pos2(x as f32, y as f32),
        _ => (bounds.center() - fsize / 2.0).round(),
    });
    let frame = Rect::from_min_size(at, fsize);
    run.pos = Some(at);
    if !collapsed && frame.min != guess {
        let size = fixed.map_or([None, None], |f| {
            let c = f - extra - top;
            [Some(c.x.max(0.0)), Some(c.y.max(0.0))]
        });
        crate::overlays::lay_out(arena, i, origin, &ctx, frame.min + lead + top, size);
    }
    // a press inside brings it to the top (z-order)
    if let Some(p) = ctx.input(|inp| inp.events.iter().find_map(|e| match e {
        egui::Event::PointerButton { pos, pressed: true, .. } => Some(*pos),
        _ => None,
    }))
        && ctx.layer_id_at(p) == Some(layer)
    {
        ctx.move_to_top(layer);
    }
    if state != arena.core[i].state {
        crate::style::set_own_state(arena, i, state);
    }
    let mut closed = false;
    let mut toggle = false;
    let mut drag = Vec2::ZERO;
    let area = frame.expand(EDGE_HIT);
    egui::Area::new(id).order(Order::Middle).fixed_pos(area.min).constrain(false).interactable(true).show(&ctx, |aui| {
        aui.set_max_size(area.size());
        if !enabled {
            aui.disable();
        }
        let s = StyleView::new(sheet.as_deref(), WINDOW_UA, state, dark(aui.ctx()));
        let bs = s.part(bar_css.as_deref(), TITLEBAR_UA);
        let mut pc = paint::PaintCx::for_node(aui.painter(), node.to_raw());
        pc.tag = "inner-window";
        paint::paint_box(&mut pc, &s, frame);
        // AccessKit: a Window named by its title
        let surface = aui.interact(frame, id.with("surface"), Sense::hover());
        surface.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Window, enabled, o.title.as_str()));
        // the title bar: collapse, title, close
        let bar = Rect::from_min_size(frame.min, vec2(frame.width(), bar_h));
        paint::paint_box(&mut pc.part("titlebar"), &bs, bar);
        let mut x = bar.left() + bl;
        let mut right = bar.right() - br;
        let btn = |x: f32| Rect::from_min_size(egui::pos2(x, bar.center().y - 11.0), vec2(22.0, 22.0));
        if o.collapsible {
            let r = btn(x - 4.0);
            x = r.right() + 2.0;
            let g = if collapsed { Glyph::ChevronRight } else { Glyph::ChevronDown };
            toggle |= tool(aui, &mut pc, &bs, collapse_css.as_deref(), "collapse", &mut run.collapse, r, id, g, "Collapse window", Some(!collapsed));
        }
        if o.closable {
            let r = btn(right - 22.0);
            right = r.left();
            closed |= tool(aui, &mut pc, &bs, close_css.as_deref(), "close", &mut run.close, r, id, Glyph::Close, "Close window", None);
        }
        let ts = bs.part(title_css.as_deref(), TITLE_UA);
        let tr = Rect::from_min_max(egui::pos2(x, bar.top()), egui::pos2(right, bar.bottom()));
        let tg = Rect::from_min_size(egui::pos2(x, bar.center().y - title.size().y / 2.0), title.size());
        paint::paint_text(&mut pc.part("title"), &ts, &title, tg.intersect(tr), Align::Start);
        // drag the title bar to move; double click collapses
        let grab = aui.interact(tr, id.with("titlebar"), Sense::CLICK | Sense::DRAG);
        if grab.dragged() {
            drag = grab.drag_delta();
        }
        if grab.double_clicked() && o.collapsible {
            toggle = true;
        }
        if o.resizable && !collapsed {
            let all: Vec<usize> = (0..EDGES.len()).collect();
            edges(aui, id, &mut run, frame, bounds, &all);
        }
        drop(pc);
        if !collapsed {
            crate::overlays::draw_content(arena, i, origin, aui, out, Rect::from_min_max(frame.min + lead + top, frame.max - (extra - lead)));
        } else {
            arena.core[i].drawn = frame;
            crate::focus::record_scope(arena, i, aui);
        }
    });
    if drag != Vec2::ZERO {
        // the title bar stays reachable: at least 40 of it inside the viewport
        let p = run.pos.unwrap_or(frame.min) + drag;
        let x = p.x.clamp(bounds.left() - frame.width() + 40.0, bounds.right() - 40.0);
        let y = p.y.clamp(bounds.top(), (bounds.bottom() - bar_h).max(bounds.top()));
        run.pos = Some(egui::pos2(x, y));
        ctx.request_repaint();
    }
    if toggle {
        run.collapsed = !run.collapsed;
        ctx.request_repaint();
    }
    drop(run);
    arena.core[i].drawn = frame;
    closed.then_some(REJECT)
}
