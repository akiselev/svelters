//! Paint and measure of `hyperlink`, `radio-button` and `switch` (OW1b: `range-slider` and
//! `drag-value` are own widgets, `crate::widgets`).

use super::{Painted, changed, text_size};
use crate::kinds::P;
use crate::layout::Metrics;
use crate::scene::{Arena, W};
use crate::walk::Out;
use egui::{Role, Sense, Ui, Vec2, WidgetInfo, WidgetText, pos2, vec2};
use svelters_abi::Value;

pub(super) fn intrinsic(m: &Metrics, w: &W) -> Vec2 {
    let sp = &m.style.spacing;
    let interact = sp.interact_size;
    match w {
        W::Hyperlink { text, url, .. } => text_size(m, if text.is_empty() { url } else { text }),
        W::RadioButton { text, .. } => {
            let t = text_size(m, text);
            let icon = sp.icon_width + if text.is_empty() { 0.0 } else { sp.icon_spacing };
            vec2(t.x + icon, t.y.max(interact.y))
        }
        W::Switch { text, .. } => {
            let mut wd = 2.0 * interact.y;
            if !text.is_empty() {
                wd += sp.item_spacing.x + text_size(m, text).x;
            }
            vec2(wd, interact.y)
        }
        _ => Vec2::ZERO,
    }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a
pub(super) fn paint(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> Painted {
    let enabled = arena.common(i).enabled;
    if matches!(arena.w(i), W::RadioButton { .. }) {
        return radio(arena, i, ui, out);
    }
    // the props the widget changed and the `changed` arguments, announced after the borrow
    let mut pending: Option<(Vec<P>, Vec<Value>)> = None;
    let resp = match arena.w_mut(i) {
        W::Hyperlink { text, url, new_tab } => {
            let shown = if text.is_empty() { url.clone() } else { text.clone() };
            if url.is_empty() { ui.add(egui::Link::new(shown)) } else { ui.add(egui::Hyperlink::from_label_and_url(shown, url.as_str()).open_in_new_tab(*new_tab)) }
        }
        W::Switch { text, active } => {
            let before = *active;
            let r = switch(ui, active, text, enabled);
            if *active != before {
                pending = Some((vec![P::Active], vec![Value::Bool(*active)]));
            }
            r
        }
        _ => unreachable!("not an input kind"),
    };
    if let Some((keys, args)) = pending {
        changed(arena, i, &keys, args, out);
    }
    Painted { resp }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a
/// `radio-button`: a click checks it, writes its `value` into `group`, and unchecks the sibling
/// radio buttons (setting their `group` too when this one has a value).
fn radio(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> Painted {
    let W::RadioButton { text, checked, value, group } = arena.w(i) else { unreachable!() };
    let (text, checked, value, group) = (text.clone(), *checked, value.clone(), group.clone());
    let resp = ui.add(egui::RadioButton::new(checked, text.as_str()));
    if !resp.clicked() {
        return Painted { resp };
    }
    let new_group = if value.is_empty() { group.clone() } else { value.clone() };
    if !checked || new_group != group {
        if let W::RadioButton { checked: c, group: g, .. } = arena.w_mut(i) {
            *c = true;
            g.clone_from(&new_group);
        }
        let mut keys = vec![];
        if !checked {
            keys.push(P::Checked);
        }
        if new_group != group {
            keys.push(P::Group);
        }
        changed(arena, i, &keys, vec![Value::Bool(true), Value::Str(new_group.clone())], out);
    }
    let Some(parent) = arena.core[i].parent else { return Painted { resp } };
    let pi = arena.ix(parent);
    let mut any = false;
    for k in 0..arena.children[pi].len() {
        let j = arena.ix(arena.children[pi][k]);
        if j == i {
            continue;
        }
        let W::RadioButton { checked: c, value: v, group: g, .. } = arena.w_mut(j) else { continue };
        let mut keys = vec![];
        if !value.is_empty() && *g != new_group {
            g.clone_from(&new_group);
            keys.push(P::Group);
        }
        let now = !v.is_empty() && *g == *v;
        if *c != now {
            *c = now;
            keys.push(P::Checked);
        }
        if !keys.is_empty() {
            let args = vec![Value::Bool(now), Value::Str(g.clone())];
            changed(arena, j, &keys, args, out);
            any = true;
        }
    }
    if any {
        // siblings drawn before this one show the old state until the next frame
        ui.ctx().request_repaint();
    }
    Painted { resp }
}

/// The synthetic `switch`: a rounded track with a knob that slides (egui's toggle-switch demo),
/// then the caption; a click (or Space/Enter when focused) toggles it. AccessKit role `Switch`.
fn switch(ui: &mut Ui, on: &mut bool, text: &str, enabled: bool) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let track = vec2(2.0 * h, h);
    let galley = (!text.is_empty()).then(|| WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body));
    let gap = ui.spacing().item_spacing.x;
    let width = track.x + galley.as_ref().map_or(0.0, |g| gap + g.size().x);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let state = *on;
    resp.widget_info(|| WidgetInfo::selected(Role::Switch, enabled, state, text));
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(resp.id, state);
        let visuals = ui.style().interact_selectable(&resp, state);
        let tr = egui::Rect::from_min_size(pos2(rect.left(), rect.center().y - track.y / 2.0), track).shrink(2.0).expand(visuals.expansion);
        let radius = 0.5 * tr.height();
        ui.painter().rect(tr, radius, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        let cx = egui::lerp((tr.left() + radius)..=(tr.right() - radius), how_on);
        ui.painter().circle(pos2(cx, tr.center().y), 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
        if let Some(g) = galley {
            let color = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
            let pos = pos2(rect.left() + track.x + gap, rect.center().y - g.size().y / 2.0);
            ui.painter().galley(pos, g, color);
        }
    }
    resp
}
