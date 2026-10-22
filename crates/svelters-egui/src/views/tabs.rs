//! `tab-view` and `tab` (WG4a): a bar of tab captions over the selected page.
//!
//! Each child is a page; a `tab` names it with `title` (any other child with its
//! `accessible-name`, else "Tab n"). Only the selected page is laid out (the others get the hidden
//! layout) and drawn. A click on a caption selects it (`changed`, `bind:selected`); a closable tab
//! has a close button and takes a middle click (`close-requested` on the tab: JS removes it).

use super::{hide, measure_child, place_child, shrink};
use crate::kinds;
use crate::layout::{Metrics, Tree};
use crate::scene::{Arena, Dirty, W};
use crate::walk::Out;
use egui::{Rect, Role, Sense, Ui, UiBuilder, Vec2, WidgetInfo, vec2};
use svelters_abi::Value;
use taffy::NodeId;
use taffy::prelude::{AvailableSpace, Size};

/// Height of the tab bar.
pub(crate) fn bar_height(m: &Metrics) -> f32 {
    let sp = &m.style.spacing;
    (m.row_height() + 2.0 * sp.button_padding.y).max(sp.interact_size.y) + 4.0
}

/// The page shown for `selected` among `n` pages.
fn page(selected: i64, n: usize) -> Option<usize> {
    (n > 0).then(|| selected.clamp(0, n as i64 - 1) as usize)
}

/// Natural size: the selected page's, plus the bar.
pub(crate) fn natural(tree: &mut Tree<'_>, kids: &[NodeId], selected: i64, avail: Size<AvailableSpace>) -> Size<f32> {
    let bar = bar_height(tree.m);
    let p = match page(selected, kids.len()) {
        Some(k) => measure_child(tree, kids[k], shrink(avail, 0.0, bar)),
        None => Size::ZERO,
    };
    Size { width: p.width.max(120.0), height: p.height + bar }
}

/// The selected page fills the area under the bar; the others are hidden.
pub(crate) fn place(tree: &mut Tree<'_>, kids: &[NodeId], selected: i64, size: Size<f32>) {
    let bar = bar_height(tree.m);
    let shown = page(selected, kids.len());
    for (k, &c) in kids.iter().enumerate() {
        if Some(k) == shown {
            place_child(tree, c, k as u32, (0.0, bar), Size { width: size.width, height: (size.height - bar).max(0.0) });
        } else {
            hide(tree, c);
        }
    }
}

/// The caption of page `c`.
fn caption(arena: &Arena, c: usize, k: usize) -> (String, bool) {
    match arena.w(c) {
        W::Tab { title, closable } if !title.is_empty() => (title.clone(), *closable),
        W::Tab { closable, .. } => (format!("Tab {}", k + 1), *closable),
        _ if !arena.common(c).accessible_name.is_empty() => (arena.common(c).accessible_name.clone(), false),
        _ => (format!("Tab {}", k + 1), false),
    }
}

/// Paints tab-view `i` at `r`: the bar, then the selected page (if the last layout placed it).
pub(crate) fn paint(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let W::TabView { selected } = *arena.w(i) else { unreachable!() };
    let n = arena.children[i].len();
    let shown = page(selected, n);
    let id = arena.core[i].egui_id;
    let enabled = arena.common(i).enabled;
    // the bar is what the last layout left above the page
    let bar = match shown {
        Some(k) => {
            let pr = arena.core[arena.ix(arena.children[i][k])].rect;
            if pr == Rect::NOTHING { bar_height(&Metrics::new(ui.ctx())) } else { pr.min.y - arena.core[i].rect.min.y }
        }
        None => bar_height(&Metrics::new(ui.ctx())),
    };
    let laid_out = !arena.core[i].dirty.contains(Dirty::LAYOUT);
    let pages: Vec<(String, bool)> = (0..n).map(|k| caption(arena, arena.ix(arena.children[i][k]), k)).collect();
    let bar_rect = Rect::from_min_size(r.min, vec2(r.width(), bar));
    let mut pick = None;
    let mut close = Vec::new();
    let mut b = UiBuilder::new().max_rect(bar_rect).scope_id(id.with("bar")).layout(egui::Layout::left_to_right(egui::Align::Center));
    if !enabled {
        b = b.disabled();
    }
    let mut bui = ui.new_child(b);
    let line_y = bar_rect.bottom() - 0.5;
    egui::ScrollArea::horizontal()
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(&mut bui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (k, (title, closable)) in pages.iter().enumerate() {
                let on = Some(k) == shown;
                let mut resp = ui.add(egui::Button::selectable(on, title.as_str()));
                resp.widget_info(|| WidgetInfo::selected(Role::Tab, enabled, on, title));
                ui.ctx().accesskit_node_builder(resp.id, |b| {
                    b.set_role(Role::Tab);
                    b.set_selected(on);
                });
                if resp.clicked() && !on {
                    pick = Some(k);
                    resp.mark_changed();
                }
                if *closable {
                    if resp.middle_clicked() {
                        close.push(k);
                    }
                    let x = ui.add(egui::Button::new("×").small().frame(false)).on_hover_text("Close");
                    let x = x.accessible_name(format!("Close {title}"));
                    if x.clicked() {
                        close.push(k);
                    }
                }
            }
        });
    ui.painter().hline(bar_rect.x_range(), line_y, ui.visuals().widgets.noninteractive.bg_stroke);
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        ui.interact(bar_rect, id.with("tip"), Sense::hover()).on_hover_text(tip);
    }
    arena.core[i].drawn = r;
    crate::scene::hit::record(arena, i, ui);
    for k in close {
        let c = arena.children[i][k];
        let ci = arena.ix(c);
        let s = kinds::signal_id(arena.core[ci].kind as usize, "close-requested");
        if arena.listens(ci, s) {
            out.push(Out::Event(c.to_raw(), s, vec![]));
        }
    }
    if let Some(k) = pick {
        if let W::TabView { selected } = arena.w_mut(i) {
            *selected = k as i64;
        }
        // another page is laid out next frame
        arena.invalidate_style(node);
        ui.ctx().request_repaint();
        crate::leaf::widget_changed(arena, i, &[kinds::P::SelectedIndex], vec![Value::I64(k as i64)], out);
    }
    // (on the frame of a click, the page laid out before it)
    if laid_out && let Some(k) = shown {
        let c = arena.ix(arena.children[i][k]);
        crate::walk::paint_at(arena, c, origin, ui, out);
    }
}
