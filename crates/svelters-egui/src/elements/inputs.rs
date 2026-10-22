//! Paint and measure of `hyperlink`, `radio-button`, `switch`, `range-slider` and `drag-value`.

use super::{Painted, changed, finite_or, text_size};
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
        W::RangeSlider { text, show_value, .. } => {
            let mut wd = sp.slider_width;
            if *show_value {
                wd += 2.0 * (sp.item_spacing.x + interact.x);
            }
            if !text.is_empty() {
                wd += sp.item_spacing.x + text_size(m, text).x;
            }
            vec2(wd, interact.y)
        }
        W::DragValue { value, decimals, prefix, suffix, .. } => {
            let shown = match decimals {
                Some(d) => format!("{prefix}{value:.*}{suffix}", *d as usize),
                None => format!("{prefix}{value}{suffix}"),
            };
            let t = text_size(m, &shown);
            vec2((t.x + 2.0 * sp.button_padding.x).max(interact.x), interact.y)
        }
        _ => Vec2::ZERO,
    }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a, OW1b
pub(super) fn paint(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> Painted {
    let size = arena.core[i].rect.size();
    let enabled = arena.common(i).enabled;
    if matches!(arena.w(i), W::RadioButton { .. }) {
        return radio(arena, i, ui, out);
    }
    // the props the widget changed and the `changed` arguments, announced after the borrow
    let mut pending: Option<(Vec<P>, Vec<Value>)> = None;
    // CX3: a drag-value's committed value (`commit`)
    let mut commit: Option<f64> = None;
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
        W::RangeSlider { low, high, min, max, step, text, show_value } => {
            // NaN or an infinity from the host never reaches egui (its clamps panic on a NaN
            // bound): a rail end falls back to 0 or 1 and is kept within ±1e300 (so the rail's
            // length is finite), a handle falls back to its end of the rail. The props keep what
            // the host wrote until the user moves a handle.
            let lo_end = finite_or(*min, 0.0).clamp(-1e300, 1e300);
            let hi_end = finite_or(*max, if lo_end < 1.0 { 1.0 } else { lo_end + 1.0 }).clamp(-1e300, 1e300);
            let (rail_lo, rail_hi) = if lo_end <= hi_end { (lo_end, hi_end) } else { (hi_end, lo_end) };
            let (mut l, mut h) = (finite_or(*low, rail_lo), finite_or(*high, rail_hi));
            let before = (l, h);
            // The rail fills the rect: it takes what the value boxes and caption leave.
            let sp = ui.spacing().clone();
            let mut extra = 0.0;
            if *show_value {
                extra += 2.0 * (sp.item_spacing.x + sp.interact_size.x);
            }
            if !text.is_empty() {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.clone(), font, egui::Color32::WHITE).size().x);
                extra += sp.item_spacing.x + tw;
            }
            // slack for value boxes that come out wider than interact_size.x (as `slider`)
            ui.spacing_mut().slider_width = (size.x - extra - 8.0).max(sp.interact_size.x);
            let step = (step.is_finite() && *step > 0.0).then_some(*step);
            let r = if text.is_empty() && *show_value {
                uncaptioned_range(ui, &mut l, &mut h, rail_lo..=rail_hi, step)
            } else {
                let mut s = crate::numeric::range_slider(egui::RangeSlider::new(&mut l, &mut h, rail_lo..=rail_hi).show_value(*show_value));
                if let Some(step) = step {
                    s = s.step_by(step);
                }
                if !text.is_empty() {
                    s = s.text(text.as_str());
                }
                ui.add(s)
            };
            if (l, h) != before {
                // both are stored: a handle the host left at NaN takes the value it was shown at
                let mut keys = Vec::new();
                if l != *low {
                    *low = l;
                    keys.push(P::Low);
                }
                if h != *high {
                    *high = h;
                    keys.push(P::High);
                }
                pending = Some((keys, vec![Value::F64(l), Value::F64(h)]));
            }
            r
        }
        W::DragValue { value, min, max, speed, decimals, prefix, suffix } => {
            // a NaN bound is no bound; a NaN value shows (and drags from) 0 or the lower bound
            let (lo, hi) = (min.filter(|m| !m.is_nan()), max.filter(|m| !m.is_nan()));
            let mut v = if value.is_nan() { lo.filter(|m| m.is_finite()).unwrap_or(0.0) } else { *value };
            let before = v;
            // IN1: the text shows and reads numbers in the host's locale
            let mut d = crate::numeric::drag_value(egui::DragValue::new(&mut v).speed(finite_or(*speed, 1.0)).min_size(size));
            if lo.is_some() || hi.is_some() {
                d = d.range(lo.unwrap_or(f64::NEG_INFINITY)..=hi.unwrap_or(f64::INFINITY));
            }
            if let Some(n) = decimals {
                d = d.fixed_decimals(*n as usize);
            }
            if !prefix.is_empty() {
                d = d.prefix(prefix.clone());
            }
            if !suffix.is_empty() {
                d = d.suffix(suffix.clone());
            }
            let r = ui.add(d);
            if v != before {
                *value = v;
                pending = Some((vec![P::Value], vec![Value::F64(v)]));
            }
            // CX3 (minimal, until OW4's own widget): a drag commits when it ends; a change made
            // otherwise (arrow keys, a typed value applied on Enter or blur) commits at once
            if r.drag_stopped() || (v != before && !r.dragged()) {
                commit = Some(v);
            }
            r
        }
        _ => unreachable!("not an input kind"),
    };
    if let Some((keys, args)) = pending {
        changed(arena, i, &keys, args, out);
    }
    if let Some(v) = commit {
        let s = crate::kinds::signal_id(arena.core[i].kind as usize, "commit");
        if arena.listens(i, s) {
            let h = arena.slots.id_at(i).expect("live leaf").to_raw();
            out.push(Out::Event(h, s, vec![Value::F64(v)]));
        }
    }
    Painted { resp }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1b
/// A `range-slider` with value boxes and no caption. egui's `RangeSlider` names its value boxes
/// only by its caption (its `Slider` names an uncaptioned box by the slider), so here the rail is
/// drawn without boxes and the two boxes are added around it, each labelled by the rail, which the
/// leaf wrapper names (`accessible-name`, else `tooltip`).
fn uncaptioned_range(ui: &mut Ui, l: &mut f64, h: &mut f64, rail: std::ops::RangeInclusive<f64>, step: Option<f64>) -> egui::Response {
    let (lo, hi) = (*rail.start(), *rail.end());
    // a value per point of rail, as dragging the handles
    let speed = (hi - lo) / ui.spacing().slider_width.max(1.0) as f64;
    let snap = |v: f64| step.map_or(v, |s| (lo + ((v - lo) / s).round() * s).clamp(lo, hi));
    ui.horizontal(|ui| {
        let mut lv = *l;
        let low = ui.add(crate::numeric::drag_value(egui::DragValue::new(&mut lv).range(lo..=*h).speed(speed)));
        let mut s = egui::RangeSlider::new(l, h, rail.clone()).show_value(false);
        if let Some(step) = step {
            s = s.step_by(step);
        }
        let rail_resp = ui.add(s);
        let mut hv = *h;
        let high = ui.add(crate::numeric::drag_value(egui::DragValue::new(&mut hv).range(*l..=hi).speed(speed)));
        if lv != *l && !rail_resp.changed() {
            *l = snap(lv).min(*h);
        }
        if hv != *h && !rail_resp.changed() {
            *h = snap(hv).max(*l);
        }
        let low = low.labelled_by(rail_resp.id);
        let high = high.labelled_by(rail_resp.id);
        // the focused box speaks for the widget (as egui's own)
        let mut resp = rail_resp;
        for b in [low, high] {
            resp = if b.has_focus() || b.gained_focus() || b.lost_focus() { b.union(resp) } else { resp.union(b) };
        }
        resp
    })
    .inner
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
