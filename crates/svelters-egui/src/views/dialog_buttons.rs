//! OW7b: `dialog-buttons` (WIDGET-PARITY §2.21, QDialogButtonBox; FM1 QW11's order): a flex row
//! whose `button` children take their place by `dialog-role` in the platform's order
//! (`Policy::button_order`, or the `order` prop). Only taffy sees the order
//! ([`order_children`], `layout.rs` `children_laid`): paint, hit testing and Tab order stay the
//! tree's. A click reports `clicked(role)` (and `help`); in a `modal` or `inner-window`, an accept
//! or reject button closes it with that result, as `close("accept")` does.

use crate::kinds;
use crate::native::ButtonOrder;
use crate::overlays::OverlayKind;
use crate::scene::{Arena, NodeId as SceneId, W};
use crate::walk::Out;
use egui::{Ui, Vec2};
use svelters_abi::Value;

/// A `dialog-buttons` node's props and its children in layout order.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogButtons {
    pub order: &'static str,
    /// The children in button order (taffy's view), rebuilt before each layout of the row.
    pub(crate) laid: Vec<SceneId>,
}

impl Default for DialogButtons {
    fn default() -> Self {
        DialogButtons { order: "platform", laid: Vec::new() }
    }
}

fn is_button(arena: &Arena, c: usize) -> bool {
    kinds::tag_index("button") == Some(arena.core[c].kind as usize)
}

/// Child `c`'s role: its `dialog-role`, `auto` resolved (`accept` for a `default` button, `reject`
/// for a `cancel` one, else `action`); `None` for a child that is not a button and has no role.
pub(crate) fn role_of(arena: &Arena, c: usize) -> Option<&'static str> {
    let cm = arena.common(c);
    match cm.dialog_role {
        "auto" if !is_button(arena, c) => None,
        "auto" if cm.default_button => Some("accept"),
        "auto" if cm.cancel_button => Some("reject"),
        "auto" => Some("action"),
        r => Some(r),
    }
}

/// The order `name` names; `platform` is the policy's.
pub(crate) fn order_of(name: &str, ctx: &egui::Context) -> Option<ButtonOrder> {
    match name {
        "none" => None,
        "platform" => Some(crate::native::policy_of(ctx).button_order),
        n => ButtonOrder::of(n),
    }
}

/// Rebuilds row `i`'s layout order: help and reset first, then the children without a role,
/// then the platform's roles (stable within a role).
pub(crate) fn order_children(arena: &mut Arena, i: usize, ctx: &egui::Context) {
    let W::DialogButtons(d) = arena.w(i) else { return };
    let order = order_of(d.order, ctx);
    let kids = arena.children[i].clone();
    let laid = match order {
        None => kids,
        Some(o) => {
            let rank = |c: &SceneId| -> usize {
                let Some(ci) = arena.idx(*c) else { return 2 };
                match role_of(arena, ci) {
                    Some("help") => 0,
                    Some("reset") => 1,
                    None => 2,
                    Some(r) => 3 + o.roles().iter().position(|x| *x == r).unwrap_or(o.roles().len()),
                }
            };
            let mut v = kids;
            v.sort_by_key(rank); // stable
            v
        }
    };
    if let W::DialogButtons(d) = arena.w_mut(i) {
        d.laid = laid;
    }
}

/// Paints row `i`'s children, then reports their clicks: `clicked(role)`, `help`, and an accept or
/// reject closing the enclosing `modal` or `inner-window` with that result.
pub(crate) fn paint(arena: &mut Arena, i: usize, r: egui::Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        // hover only, as any box's: it takes no clicks from the buttons over it
        let tr = ui.interact(r, arena.core[i].egui_id.with("tip"), egui::Sense::hover());
        crate::widgets::tooltip::attach(ui, &tr, &tip, arena.own.contains("tooltip"));
    }
    let before = out.len();
    crate::walk::paint_children(arena, i, origin, ui, out);
    let mut roles = Vec::new();
    for k in 0..arena.children[i].len() {
        let Some(c) = arena.idx(arena.children[i][k]) else { continue };
        if !is_button(arena, c) {
            continue;
        }
        let h = arena.slots.id_at(c).map(|n| n.to_raw());
        let sig = kinds::signal_id(arena.core[c].kind as usize, "clicked");
        // its own `clicked` (a handler listens; Enter or Escape through EV3), else the pointer's
        let emitted = out[before..].iter().any(|o| matches!(o, Out::Event(src, s, _) if Some(*src) == h && *s == sig));
        let pointer = ui.ctx().read_response(arena.core[c].egui_id).is_some_and(|r| r.clicked());
        if (emitted || pointer)
            && let Some(r) = role_of(arena, c)
        {
            roles.push(r);
        }
    }
    let node = arena.slots.id_at(i).expect("live node").to_raw();
    let kind = arena.core[i].kind as usize;
    let (s_clicked, s_help) = (kinds::signal_id(kind, "clicked"), kinds::signal_id(kind, "help"));
    for r in roles {
        if arena.listens(i, s_clicked) {
            out.push(Out::Event(node, s_clicked, vec![Value::Str(r.to_string())]));
        }
        if r == "help" && arena.listens(i, s_help) {
            out.push(Out::Event(node, s_help, vec![]));
        }
        if matches!(r, "accept" | "reject") {
            close_dialog(arena, i, r);
            ui.ctx().request_repaint();
        }
    }
}

/// The nearest `modal` or `inner-window` above `i` closes with `result` at the next pass.
fn close_dialog(arena: &mut Arena, i: usize, result: &str) {
    let mut at = arena.core[i].parent;
    while let Some(p) = at.and_then(|p| arena.idx(p)) {
        if let W::Overlay(o) = arena.w_mut(p)
            && matches!(o.kind, OverlayKind::Modal | OverlayKind::InnerWindow)
        {
            o.cmd = Some(false);
            o.result = Some(result.to_string());
            return;
        }
        at = arena.core[p].parent;
    }
}
