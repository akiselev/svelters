//! OW3: svelters' own floating surfaces (ARCH-WIDGETS §3.2): the `popup` overlay, and the pieces
//! menus, the select list and tooltips share: the surface look ([`SURFACE_UA`], DESIGN R7), an
//! `egui::Area` per open surface at `Order::Foreground` (tooltips at `Order::Tooltip`), placement
//! ([`Anchored`](crate::interact::Anchored)), dismissal
//! ([`Dismissable`](crate::interact::Dismissable)) and the keyboard grab of an open menu or list.
//! No egui `Popup`, `Frame` or `Visuals`. Writer lane: OW3.

use crate::interact::anchor::{self, AnchorParams, Placement, Side};
use crate::interact::dismiss::{Dismiss, DismissParams, Dismissable, Outside, Reason};
use crate::overlays::OverlayW;
use crate::paint::{self, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::{Arena, NodeId};
use crate::walk::Out;
use egui::{Id, Order, Pos2, Rect, Ui, Vec2};
use std::sync::Arc;
use svelters_style::ComputedStyle;

/// A floating surface at rest (DESIGN R7): `surface-raised` tone (`menu-bg`), a 1px
/// `border-widget` stroke (`menu-border-color`), `menu-radius` (VIS1 measured 8), `popup-shadow`
/// (`elev-3`), 4px padding, 4px from its anchor (`margin`).
pub const SURFACE_UA: Ua = Ua(&[UaRule::base(SURFACE)]);

/// [`SURFACE_UA`]'s declarations, for tables that extend it (menus, the select list).
pub const SURFACE: &[(&str, crate::paint::UaValue)] = &[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("popup-shadow")),
    ("padding", Px(4.0)),
    ("margin", Px(4.0)),
    ("color", Token("fg")),
];

/// The computed style of `node` (`None` when no sheet reaches it), owned so the scene stays free.
pub(crate) fn css(arena: &Arena, node: NodeId) -> Option<Arc<ComputedStyle>> {
    arena.style.computed(node).cloned()
}

/// The computed style of `node`'s part `name` (`::part(name)`; `None` until OW-CSS computes parts).
pub(crate) fn part_css(arena: &Arena, node: NodeId, name: &str) -> Option<Arc<ComputedStyle>> {
    arena.style.part_computed(node, name).cloned()
}

/// UA tokens resolve in the context's scheme.
pub(crate) fn dark(ctx: &egui::Context) -> bool {
    ctx.theme() == egui::Theme::Dark
}

/// The gap between a surface and its anchor on `side`: the surface's margin facing the anchor.
pub(crate) fn gap(s: &StyleView<'_>, side: Side) -> f32 {
    let [l, r, t, b] = s.sides("margin");
    match side {
        Side::Bottom => t,
        Side::Top => b,
        Side::Right => l,
        Side::Left => r,
    }
}

/// The layer of a new surface `id` in `order`, placed at `rect` (screen). Shown with `add` in the
/// area's `Ui`; pointer input over it does not reach what is below.
pub(crate) fn area(ctx: &egui::Context, id: Id, order: Order, rect: Rect, add: impl FnOnce(&mut Ui)) -> egui::InnerResponse<()> {
    // AX1: svelters placed and sized the surface already; an area egui has not seen would spend
    // its first frame on an invisible sizing pass (no paint, no AccessKit nodes), so a menu just
    // opened would be missing from the tree when automation looks
    let layer = egui::LayerId::new(order, id);
    if egui::AreaState::load(ctx, id).is_none() {
        let st = egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() };
        ctx.memory_mut(|m| m.areas_mut().set_state(layer, st));
    }
    egui::Area::new(id).order(order).fixed_pos(rect.min).constrain(false).interactable(true).show(ctx, |ui| {
        ui.set_max_size(rect.size());
        add(ui)
    })
}

/// [`Dismissable::check`] for a surface in `ctx`: a press on another floating layer (a select's
/// list or a menu opened from inside it) does not count as outside.
pub(crate) fn dismissed(ctx: &egui::Context, p: &DismissParams) -> Option<Dismiss> {
    let d = ctx.input(|i| Dismissable::check(p, i))?;
    if d.reason == Reason::OutsidePress
        && let Some(at) = d.at
        && ctx.layer_id_at(at).is_some_and(|l| l.order >= Order::Foreground)
    {
        return None;
    }
    Some(d)
}

/// The keyboard grab of an open menu or select list: focus moves to the surface's id while it is
/// open (so the field that had it does not see the keys: egui widgets act only when focused) and
/// goes back when it closes (EV3: a Copy in an Edit menu reaches the field).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Grab {
    /// The focus before the grab.
    pub restore: Option<Id>,
    pub active: bool,
    /// The focus when the last pointer press began (a text field drops focus on a press
    /// elsewhere, before the click that opens the surface).
    pub pressed: Option<Id>,
}

impl Grab {
    /// Notes the focus at a pointer press of this pass (call it every pass, before [`Grab::take`]).
    pub fn note_press(&mut self, ctx: &egui::Context) {
        if !self.active && ctx.input(|i| i.pointer.any_pressed()) {
            self.pressed = crate::focus::pass_start_focus(ctx);
        }
    }

    /// Takes keyboard focus for `id`.
    pub fn take(&mut self, ctx: &egui::Context, id: Id) {
        if !self.active {
            let pointer = ctx.input(|i| i.pointer.any_click() || i.pointer.any_released());
            let now = ctx.memory(|m| m.focused());
            self.restore = now.or(if pointer { self.pressed } else { None }).filter(|f| *f != id);
            self.active = true;
        }
        crate::focus::note_grab(ctx);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// Keeps focus on `id` this pass (called by the open surface: egui drops focus from an id no
    /// widget senses) and keeps egui's Tab, arrow and Escape handling away from it.
    pub fn keep(&self, ui: &Ui, id: Id, rect: Rect) {
        if !self.active {
            return;
        }
        crate::focus::note_grab(ui.ctx());
        ui.interact(rect, id, egui::Sense::focusable_noninteractive());
        ui.memory_mut(|m| {
            if m.focused().is_none() {
                m.request_focus(id);
            }
            m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true });
        });
    }

    /// Gives focus back (to what had it, or to nothing) if the surface still has it.
    pub fn release(&mut self, ctx: &egui::Context, id: Id) {
        if !self.active {
            return;
        }
        self.active = false;
        ctx.memory_mut(|m| {
            if m.focused() == Some(id) || m.focused().is_none() {
                match self.restore {
                    Some(r) => m.request_focus(r),
                    None => m.surrender_focus(id),
                }
            }
        });
    }
}

/// Paints open `popup` overlay `i` anchored to `owner` (its parent's rect): its children laid out
/// by taffy inside the surface box, placed by `placement` with flip and shift, closed by Escape and
/// (with `autohide`) a press outside it and its owner. Returns whether it stays open.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(arena: &mut Arena, i: usize, origin: Vec2, owner: Rect, o: &OverlayW, enabled: bool, ui: &mut Ui, out: &mut Vec<Out>) -> bool {
    let ctx = ui.ctx().clone();
    let Some(node) = arena.slots.id_at(i) else { return false };
    let id = arena.core[i].egui_id.with("own-popup");
    let sheet = css(arena, node);
    let mut state = arena.core[i].state;
    state.set(StateBits::OPEN, true);
    state.set(StateBits::DISABLED, !enabled);
    let s = StyleView::new(sheet.as_deref(), SURFACE_UA, state, dark(&ctx));
    // the content's own padding is taffy's (the `padding` prop); the surface adds its UA padding
    // and border around it
    let ua = StyleView::ua_only(SURFACE_UA, state, s.is_dark());
    let [pl, pr, pt, pb] = ua.sides("padding");
    let [bl, br, bt, bb] = s.sides("border-width");
    let (lead, extra) = (egui::vec2(pl + bl, pt + bt), egui::vec2(pl + pr + bl + br, pt + pb + bt + bb));
    let anchor = if owner.is_positive() { owner } else { Rect::from_min_size(owner.min, Vec2::ZERO) };
    let placement = Placement::parse(o.placement).unwrap_or(Placement::BOTTOM_START);
    // lay the content out once to know its size, then where it goes
    let guess = anchor.left_bottom();
    let size = crate::overlays::lay_out(arena, i, origin, &ctx, guess + lead, [None, None]) + extra;
    let p = AnchorParams { anchor, size, bounds: anchor::bounds(&ctx), placement, gap: gap(&s, placement.side), ..AnchorParams::default() };
    let placed = anchor::place(&p);
    let frame = placed.rect;
    if frame.min != guess {
        crate::overlays::lay_out(arena, i, origin, &ctx, frame.min + lead, [None, None]);
    }
    let dp = DismissParams { inside: vec![frame, anchor], on_outside: if o.autohide { Outside::Close } else { Outside::Ignore }, escape: o.dismissable };
    if dismissed(&ctx, &dp).is_some() {
        arena.core[i].drawn = Rect::NOTHING;
        return false;
    }
    if state != arena.core[i].state {
        crate::style::set_own_state(arena, i, state);
    }
    let sheet = css(arena, node);
    area(&ctx, id, Order::Foreground, frame, |pui| {
        if !enabled {
            pui.disable();
        }
        let s = StyleView::new(sheet.as_deref(), SURFACE_UA, state, dark(pui.ctx()));
        let mut pc = paint::PaintCx::for_node(pui.painter(), node.to_raw());
        pc.tag = "popup";
        paint::paint_box(&mut pc, &s, frame);
        let r = pui.interact(frame, id.with("surface"), egui::Sense::hover());
        pui.ctx().accesskit_node_builder(r.id, |n| n.set_role(egui::accesskit::Role::Dialog));
        crate::overlays::draw_content(arena, i, origin, pui, out, Rect::from_min_size(frame.min + lead, size - extra));
    });
    true
}

/// Where a context menu opened at the pointer goes: a zero-size anchor at `at`.
pub(crate) fn point(at: Pos2) -> Rect {
    Rect::from_min_size(at, Vec2::ZERO)
}
