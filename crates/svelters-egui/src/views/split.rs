//! `split-view` (WG4a): two panes and a divider.
//!
//! The first two children are the panes (more are hidden). `position` is the first pane's size
//! along the split (null: half), clamped at layout so each pane keeps `min-size`. Dragging the
//! divider (or the arrow keys while it has focus) writes `position` (`changed`, `bind:position`);
//! the panes follow at the next layout, which the drag requests at once.

use super::{hide, measure_child, place_child};
use crate::kinds;
use crate::layout::Tree;
use crate::scene::{Arena, W};
use crate::walk::Out;
use egui::{CursorIcon, Rect, Role, Sense, Ui, Vec2, WidgetInfo, pos2, vec2};
use svelters_abi::Value;
use taffy::NodeId;
use taffy::prelude::{AvailableSpace, Size};

/// Thickness of the divider, in points.
pub(crate) const DIVIDER: f32 = 6.0;
/// Arrow-key step, in points.
const STEP: f32 = 10.0;

/// The first pane's size for a split of `main` points.
pub(crate) fn first(position: Option<f64>, min_size: f64, main: f32) -> f32 {
    let room = (main - DIVIDER).max(0.0);
    let p = position.map_or(room / 2.0, |p| p as f32);
    let lo = (min_size as f32).min(room / 2.0);
    p.clamp(lo, (room - lo).max(lo))
}

/// Natural size: the panes side by side (or stacked) with the divider between.
pub(crate) fn natural(
    tree: &mut Tree<'_>,
    kids: &[NodeId],
    vertical: bool,
    avail: Size<AvailableSpace>,
) -> Size<f32> {
    let mut main = 0.0;
    let mut cross: f32 = 0.0;
    for &c in kids.iter().take(2) {
        let s = measure_child(tree, c, avail);
        let (m, x) = if vertical {
            (s.height, s.width)
        } else {
            (s.width, s.height)
        };
        main += m;
        cross = cross.max(x);
    }
    main += DIVIDER;
    if vertical {
        Size {
            width: cross,
            height: main,
        }
    } else {
        Size {
            width: main,
            height: cross,
        }
    }
}

/// The panes at their sizes; extra children hidden.
pub(crate) fn place(
    tree: &mut Tree<'_>,
    kids: &[NodeId],
    vertical: bool,
    position: Option<f64>,
    min_size: f64,
    size: Size<f32>,
) {
    let main = if vertical { size.height } else { size.width };
    let p = first(position, min_size, main);
    let rest = (main - p - DIVIDER).max(0.0);
    for (k, &c) in kids.iter().enumerate() {
        let (at, m) = match k {
            0 => (0.0, p),
            1 => (p + DIVIDER, rest),
            _ => {
                hide(tree, c);
                continue;
            }
        };
        let (xy, sz) = if vertical {
            (
                (0.0, at),
                Size {
                    width: size.width,
                    height: m,
                },
            )
        } else {
            (
                (at, 0.0),
                Size {
                    width: m,
                    height: size.height,
                },
            )
        };
        place_child(tree, c, k as u32, xy, sz);
    }
}

/// Paints split-view `i` at `r`: the panes, then the divider (drag, keys, cursor).
pub(crate) fn paint(
    arena: &mut Arena,
    i: usize,
    r: Rect,
    origin: Vec2,
    ui: &mut Ui,
    out: &mut Vec<Out>,
) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let W::SplitView {
        vertical,
        position,
        min_size,
        resizable,
    } = *arena.w(i)
    else {
        unreachable!()
    };
    for k in 0..arena.children[i].len().min(2) {
        let c = arena.ix(arena.children[i][k]);
        crate::walk::paint_at(arena, c, origin, ui, out);
    }
    let main = if vertical { r.height() } else { r.width() };
    let p = first(position, min_size, main);
    let div = if vertical {
        Rect::from_min_size(pos2(r.min.x, r.min.y + p), vec2(r.width(), DIVIDER))
    } else {
        Rect::from_min_size(pos2(r.min.x + p, r.min.y), vec2(DIVIDER, r.height()))
    };
    let enabled = arena.common(i).enabled && resizable;
    let id = arena.core[i].egui_id.with("divider");
    let resp = ui.interact(
        div,
        id,
        if enabled {
            Sense::drag()
        } else {
            Sense::hover()
        },
    );
    if enabled && (resp.hovered() || resp.dragged()) {
        ui.ctx().set_cursor_icon(if vertical {
            CursorIcon::ResizeVertical
        } else {
            CursorIcon::ResizeHorizontal
        });
    }
    let name = arena.common(i).accessible_name.clone();
    resp.widget_info(|| WidgetInfo::labeled(Role::Splitter, enabled, &name));
    ui.ctx().accesskit_node_builder(resp.id, |b| {
        b.set_role(Role::Splitter);
        b.set_numeric_value(p as f64);
        b.set_orientation(if vertical {
            egui::accesskit::Orientation::Horizontal
        } else {
            egui::accesskit::Orientation::Vertical
        });
    });
    let v = ui.style().interact(&resp);
    let stroke = if resp.hovered() || resp.dragged() || resp.has_focus() {
        v.fg_stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    let c = div.center();
    if vertical {
        ui.painter().hline(div.x_range(), c.y, stroke);
    } else {
        ui.painter().vline(c.x, div.y_range(), stroke);
    }
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        resp = resp.on_hover_text(tip);
    }
    arena.core[i].drawn = r;
    crate::scene::hit::record(arena, i, ui);
    let mut want = None;
    if enabled
        && resp.dragged()
        && let Some(ptr) = resp.interact_pointer_pos()
    {
        let at = if vertical {
            ptr.y - r.min.y
        } else {
            ptr.x - r.min.x
        } - DIVIDER / 2.0;
        want = Some(at);
    }
    if enabled && resp.has_focus() {
        let (less, more) = if vertical {
            (egui::Key::ArrowUp, egui::Key::ArrowDown)
        } else {
            (egui::Key::ArrowLeft, egui::Key::ArrowRight)
        };
        let d =
            ui.input(|inp| (inp.key_pressed(more) as i32 - inp.key_pressed(less) as i32) as f32);
        if d != 0.0 {
            want = Some(p + d * STEP);
        }
    }
    let sig = |name: &str| kinds::signal_id(arena.core[i].kind as usize, name);
    for (yes, name) in [
        (resp.drag_started(), "drag-started"),
        (resp.drag_stopped(), "drag-stopped"),
    ] {
        let s = sig(name);
        if yes && arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![]));
        }
    }
    if let Some(at) = want {
        let np = first(Some(at as f64), min_size, main).round();
        if (np - p).abs() >= 0.5 {
            if let W::SplitView { position, .. } = arena.w_mut(i) {
                *position = Some(np as f64);
            }
            arena.invalidate_content(node);
            ui.ctx().request_repaint();
            crate::leaf::widget_changed(
                arena,
                i,
                &[kinds::P::Position],
                vec![Value::F64(np as f64)],
                out,
            );
        }
    }
}
