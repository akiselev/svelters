//! WG2a overlays (plans/milestones/WG2a-menus-popups.md): `popup`, `modal` and `inner-window`, and
//! the out-of-flow dispatch of the walk. Tables are `kinds/popups.rs`.
//!
//! An overlay is a flex box ([`W::is_container`]) that takes no place in its parent's flow
//! (`display: none` there, [`out_of_flow`]). When open, the walk shows it in its own egui area
//! (`egui::Popup`, `egui::Modal`, `egui::Window`) and lays its subtree out there on its own
//! (`layout::layout_overlay`): sized to its content, or to the window for a resizable
//! `inner-window`. Its children's rects stay root-relative, so the walk paints them as usual.
//!
//! The walk calls [`paint_node`] for every out-of-flow node it reaches through a container, and
//! [`paint_attached`] after each leaf for the context menus and popups attached to it. The owner
//! of an attached kind (its anchor) is its parent.
//!
//! Writer lane: WIDGETS.

use crate::kinds::P;
use crate::layout::{self, Metrics};
use crate::menus::{MenuW, announce, as_bool, as_str};
use crate::scene::{Arena, W};
use crate::walk::Out;
use egui::{PopupCloseBehavior, Rect, RectAlign, Sense, Ui, Vec2, vec2};
use svelters_abi::Value;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Popup,
    Modal,
    InnerWindow,
}

/// An overlay node's typed props (the Box props are in `Common`).
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayW {
    pub kind: OverlayKind,
    pub open: bool,
    /// A `popup`/`popdown` call, applied (and announced) at the next pass.
    pub cmd: Option<bool>,
    pub title: String,
    pub position: &'static str,
    pub autohide: bool,
    pub dismissable: bool,
    pub default_w: f64,
    pub default_h: f64,
    /// `default-x`, `default-y` (both needed to place it).
    pub default_x: Option<f64>,
    pub default_y: Option<f64>,
    pub closable: bool,
    pub collapsible: bool,
    pub resizable: bool,
    /// The layout generation of the subtree's last overlay layout (`layout::layout_overlay`).
    pub seen: u64,
}

pub(crate) fn payload(id: &str) -> OverlayW {
    let kind = match id {
        "Modal" => OverlayKind::Modal,
        "InnerWindow" => OverlayKind::InnerWindow,
        _ => OverlayKind::Popup,
    };
    OverlayW {
        kind,
        open: kind != OverlayKind::Popup,
        cmd: None,
        title: String::new(),
        position: "bottom",
        autohide: true,
        dismissable: true,
        default_w: 320.0,
        default_h: 240.0,
        default_x: None,
        default_y: None,
        closable: true,
        collapsible: true,
        resizable: true,
        seen: 0,
    }
}

/// Out of its parent's flow (`display: none` there): the overlays and the menu entries.
pub(crate) fn out_of_flow(w: &W) -> bool {
    match w {
        W::Overlay(_) => true,
        W::Menu(m) => !m.in_flow(),
        _ => false,
    }
}

// ---- props ----

pub(crate) fn set_prop(o: &mut OverlayW, key: P, v: &Value) -> Result<(), String> {
    let f = |v: &Value| match v {
        Value::F64(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        other => Err(format!("expected a number, got {other:?}")),
    };
    match key {
        P::Open => o.open = as_bool(v)?,
        P::Title => o.title = as_str(v)?,
        P::Position if o.kind == OverlayKind::Popup => {
            let s = as_str(v)?;
            o.position = crate::kinds::POSITION
                .iter()
                .copied()
                .find(|x| *x == s)
                .ok_or_else(|| {
                    format!("`{s}` is not one of {}", crate::kinds::POSITION.join(", "))
                })?;
        }
        P::Autohide => o.autohide = as_bool(v)?,
        P::Dismissable => o.dismissable = as_bool(v)?,
        P::DefaultWidth => o.default_w = f(v)?,
        P::DefaultHeight => o.default_h = f(v)?,
        P::DefaultX => {
            o.default_x = if matches!(v, Value::Null) {
                None
            } else {
                Some(f(v)?)
            }
        }
        P::DefaultY => {
            o.default_y = if matches!(v, Value::Null) {
                None
            } else {
                Some(f(v)?)
            }
        }
        P::Closable => o.closable = as_bool(v)?,
        P::Collapsible => o.collapsible = as_bool(v)?,
        P::Resizable => o.resizable = as_bool(v)?,
        key => return Err(format!("{key:?} is not a prop of {:?}", o.kind)),
    }
    Ok(())
}

pub(crate) fn get_prop(o: &OverlayW, key: P) -> Result<Value, String> {
    Ok(match key {
        P::Open => Value::Bool(o.open),
        P::Title => Value::Str(o.title.clone()),
        P::Position => Value::Str(o.position.to_string()),
        P::Autohide => Value::Bool(o.autohide),
        P::Dismissable => Value::Bool(o.dismissable),
        P::DefaultWidth => Value::F64(o.default_w),
        P::DefaultHeight => Value::F64(o.default_h),
        P::DefaultX => o.default_x.map_or(Value::Null, Value::F64),
        P::DefaultY => o.default_y.map_or(Value::Null, Value::F64),
        P::Closable => Value::Bool(o.closable),
        P::Collapsible => Value::Bool(o.collapsible),
        P::Resizable => Value::Bool(o.resizable),
        key => return Err(format!("{key:?} is not a prop of {:?}", o.kind)),
    })
}

// ---- the walk's dispatch ----

/// Where the owner of node `i` (its parent) was drawn: its widget's rect when it drew one this
/// pass, else its layout rect at `origin`.
fn owner_rect(arena: &Arena, i: usize, origin: Vec2) -> Rect {
    let Some(p) = arena.core[i].parent.and_then(|p| arena.idx(p)) else {
        return Rect::NOTHING;
    };
    let d = arena.core[p].drawn;
    if d != Rect::NOTHING && d.is_positive() {
        d
    } else {
        arena.core[p].rect.translate(origin)
    }
}

/// Paints out-of-flow node `i` (the walk reached it as a child): an overlay, or a context menu
/// anchored to its parent. Menu entries outside a menu draw nothing.
pub(crate) fn paint_node(
    arena: &mut Arena,
    i: usize,
    origin: Vec2,
    ui: &mut Ui,
    out: &mut Vec<Out>,
) {
    let owner = owner_rect(arena, i, origin);
    match arena.w(i) {
        W::Overlay(_) => overlay(arena, i, origin, owner, ui, out),
        W::Menu(MenuW::Context { .. }) => crate::menus::context(arena, i, owner, ui, out),
        _ => {}
    }
}

/// Paints the context menus and popups attached to leaf `i` (just drawn).
pub(crate) fn paint_attached(
    arena: &mut Arena,
    i: usize,
    origin: Vec2,
    ui: &mut Ui,
    out: &mut Vec<Out>,
) {
    for k in 0..arena.children[i].len() {
        let c = arena.children[i][k].index() as usize;
        let attached = matches!(arena.w(c), W::Menu(MenuW::Context { .. }))
            || matches!(arena.w(c), W::Overlay(o) if o.kind == OverlayKind::Popup);
        if attached && arena.common(c).visible {
            paint_node(arena, c, origin, ui, out);
        }
    }
}

/// Lays out overlay `i`'s subtree at `ui`'s cursor (`size`: the definite axes) and paints its
/// children; allocates its size in `ui`.
fn content(
    arena: &mut Arena,
    i: usize,
    origin: Vec2,
    ui: &mut Ui,
    out: &mut Vec<Out>,
    size: [Option<f32>; 2],
) {
    let m = Metrics::new(ui.ctx());
    let at = ui.available_rect_before_wrap().min;
    let Some(node) = arena.slots.id_at(i) else {
        return;
    };
    let seen = match arena.w(i) {
        W::Overlay(o) => o.seen,
        _ => 0,
    };
    let (sz, generation) = layout::layout_overlay(arena, &m, node, size, at - origin, seen);
    if let W::Overlay(o) = arena.w_mut(i) {
        o.seen = generation;
    }
    let r = Rect::from_min_size(at, sz);
    arena.core[i].drawn = r;
    ui.allocate_rect(r, Sense::hover());
    crate::walk::paint_children(arena, i, origin, ui, out);
}

/// Paints overlay `i` (open or not) anchored to `owner`, and announces a user (or `popup`/`popdown`)
/// open or close.
fn overlay(
    arena: &mut Arena,
    i: usize,
    origin: Vec2,
    owner: Rect,
    ui: &mut Ui,
    out: &mut Vec<Out>,
) {
    let W::Overlay(o) = arena.w(i) else { return };
    let o = (**o).clone();
    let was = o.open;
    let mut open = o.cmd.unwrap_or(was);
    if let W::Overlay(w) = arena.w_mut(i) {
        w.cmd = None;
        w.open = open;
    }
    let id = arena.core[i].egui_id;
    let enabled = arena.common(i).enabled && ui.is_enabled();
    let ctx = ui.ctx().clone();
    arena.core[i].drawn = Rect::NOTHING;
    if open {
        match o.kind {
            OverlayKind::Popup => {
                let align = match o.position {
                    "top" => RectAlign::TOP_START,
                    "left" => RectAlign::LEFT_START,
                    "right" => RectAlign::RIGHT_START,
                    _ => RectAlign::BOTTOM_START,
                };
                let anchor = if owner.is_positive() {
                    owner
                } else {
                    Rect::from_min_size(owner.min, Vec2::ZERO)
                };
                let mut still = true;
                egui::Popup::new(id.with("popup"), ctx.clone(), anchor, ui.layer_id())
                    .open_bool(&mut still)
                    .close_behavior(if o.autohide {
                        PopupCloseBehavior::CloseOnClickOutside
                    } else {
                        PopupCloseBehavior::IgnoreClicks
                    })
                    .align(align)
                    .gap(4.0)
                    .show(|pui| {
                        if !enabled {
                            pui.disable();
                        }
                        content(arena, i, origin, pui, out, [None, None]);
                    });
                open = still;
            }
            OverlayKind::Modal => {
                let r = egui::Modal::new(id.with("modal")).show(&ctx, |mui| {
                    if !enabled {
                        mui.disable();
                    }
                    if !o.title.is_empty() {
                        mui.heading(o.title.as_str());
                    }
                    content(arena, i, origin, mui, out, [None, None]);
                });
                if o.dismissable && r.should_close() {
                    open = false;
                }
            }
            OverlayKind::InnerWindow => {
                let mut still = true;
                let mut w = egui::Window::new(o.title.as_str())
                    .id(id.with("window"))
                    .default_size(vec2(o.default_w as f32, o.default_h as f32))
                    .collapsible(o.collapsible)
                    .resizable(o.resizable);
                if let (Some(x), Some(y)) = (o.default_x, o.default_y) {
                    w = w.default_pos(egui::pos2(x as f32, y as f32));
                }
                if o.closable {
                    w = w.open(&mut still);
                }
                w.show(&ctx, |wui| {
                    if !enabled {
                        wui.disable();
                    }
                    let size = if o.resizable {
                        let a = wui.available_size();
                        [Some(a.x), Some(a.y)]
                    } else {
                        [None, None]
                    };
                    content(arena, i, origin, wui, out, size);
                });
                open = still;
            }
        }
    }
    if open != was {
        if let W::Overlay(w) = arena.w_mut(i) {
            w.open = open;
        }
        let mut events = vec![("changed", vec![Value::Bool(open)])];
        if !open {
            events.push(("closed", vec![]));
        }
        announce(arena, i, Some(P::Open), &events, out);
    }
}
