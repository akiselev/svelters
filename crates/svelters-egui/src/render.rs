//! The walk: paints the visible nodes as egui widgets at the rects taffy computed.
//!
//! Pure apart from the outbox: on a discarded pass it runs again with no input, so nothing here
//! counts or toggles anything except through widget responses (EGUI.md §2.1).

use crate::kinds;
use crate::scene::{Arena, Handle, W};
use egui::{Align, Layout, Ui, UiBuilder, Vec2};
use svelters_abi::Value;

/// What the walk produced for the host, flushed after the pass.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Event(Handle, u32, Vec<Value>),
    Changed(Handle, u32),
}

/// Paints node `h` (and its subtree) with rects offset by `origin` (the window content's screen
/// position).
pub fn paint(arena: &mut Arena, h: Handle, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let n = arena.n(h);
    if !n.c.visible {
        return;
    }
    let r = n.rect.translate(origin);
    if matches!(n.w, W::Box | W::Window { .. }) {
        // Containers draw nothing: cull whole subtrees outside the clip rect.
        if !ui.is_rect_visible(r) {
            return;
        }
        let kids = n.children.clone();
        for c in kids {
            paint(arena, c, origin, ui, out);
        }
        return;
    }
    if !ui.is_rect_visible(r) {
        return;
    }
    let mut b = UiBuilder::new()
        .max_rect(r)
        // Never the widget's own id: a scope sharing it eats the widget's clicks (EGUI.md §1.4).
        .scope_id(n.id.with("scope"))
        .layout(Layout::left_to_right(Align::Center));
    if !n.c.enabled {
        b = b.disabled();
    }
    let mut child = ui.new_child(b);
    leaf(arena, h, &mut child, out);
}

fn leaf(arena: &mut Arena, h: Handle, ui: &mut Ui, out: &mut Vec<Out>) {
    let n = arena.n_mut(h);
    let kind = n.kind;
    let sig = |name: &str| kinds::signal_id(kind, name);
    let size = n.rect.size();
    let mut text_changed = None;
    let before = match &n.w {
        W::Checkbox { checked, .. } => Some(Value::Bool(*checked)),
        W::Slider { value, .. } => Some(Value::F64(*value)),
        _ => None,
    };
    let resp = match &mut n.w {
        W::Label { text, wrap, selectable } => {
            let mode = match *wrap {
                "truncate" => egui::TextWrapMode::Truncate,
                "extend" => egui::TextWrapMode::Extend,
                _ => egui::TextWrapMode::Wrap,
            };
            let mut l = egui::Label::new(text.as_str()).wrap_mode(mode).selectable(*selectable);
            if n.listen.contains(&sig("clicked")) {
                l = l.sense(egui::Sense::click());
            }
            ui.add(l)
        }
        W::Button { text, selected, frame, small } => {
            let mut btn = egui::Button::new(text.as_str()).selected(*selected).frame(*frame).min_size(size);
            if *small {
                btn = btn.small();
            }
            ui.add(btn)
        }
        W::Checkbox { text, checked } => ui.add(egui::Checkbox::new(checked, text.as_str())),
        W::TextEdit { text, hint, multiline, password, char_limit } => {
            let mut te = if *multiline { egui::TextEdit::multiline(text).desired_rows(4) } else { egui::TextEdit::singleline(text) };
            te = te.id(n.id).desired_width(size.x).hint_text(hint.clone()).password(*password);
            if *char_limit > 0 {
                te = te.char_limit(*char_limit as usize);
            }
            let r = ui.add(te);
            if r.changed() {
                text_changed = Some(text.clone());
            }
            r
        }
        W::Slider { value, min, max, step, text, show_value } => {
            // The slider fills its rect: the rail takes what the value box and caption leave.
            let sp = ui.spacing().clone();
            let mut extra = 0.0;
            if *show_value {
                extra += sp.item_spacing.x + sp.interact_size.x;
            }
            if !text.is_empty() {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.clone(), font, egui::Color32::WHITE).size().x);
                extra += sp.item_spacing.x + tw;
            }
            ui.spacing_mut().slider_width = (size.x - extra).max(sp.interact_size.x);
            let mut s = egui::Slider::new(value, *min..=*max).show_value(*show_value);
            if *step > 0.0 {
                s = s.step_by(*step);
            }
            if !text.is_empty() {
                s = s.text(text.as_str());
            }
            ui.add(s)
        }
        W::Box | W::Window { .. } => unreachable!("containers are not leaves"),
    };
    let n = arena.n_mut(h);
    let mut resp = resp;
    if !n.c.accessible_name.is_empty() {
        resp = resp.accessible_name(n.c.accessible_name.clone());
    } else if let W::TextEdit { hint, .. } = &n.w
        && !hint.is_empty()
    {
        resp = resp.accessible_name(hint.clone());
    }
    if !n.c.tooltip.is_empty() {
        resp = resp.on_hover_text(n.c.tooltip.clone());
    }
    if n.want_focus {
        n.want_focus = false;
        resp.request_focus();
    }
    n.drawn = resp.rect;
    let mut fired: Vec<&str> = Vec::new();
    for (yes, name) in [
        (resp.clicked(), "clicked"),
        (resp.double_clicked(), "double-clicked"),
        (resp.secondary_clicked(), "secondary-clicked"),
        (resp.drag_started(), "drag-started"),
        (resp.drag_stopped(), "drag-stopped"),
    ] {
        if yes {
            fired.push(name);
        }
    }
    let focused = resp.has_focus();
    if matches!(n.w, W::TextEdit { .. }) && focused != n.had_focus {
        fired.push(if focused { "gained-focus" } else { "lost-focus" });
    }
    n.had_focus = focused;
    for name in fired {
        let s = sig(name);
        if n.listens(s) {
            out.push(Out::Event(h, s, vec![]));
        }
    }
    // Widget-side value changes: the node already holds the new value (the widget wrote the
    // field); announce it if observed (exact, no echo), then the `changed` event.
    // Compared with the value before the widget ran, so a slider clamping a host-set value into its
    // range is announced too (the binding's `natively_normalizes`).
    let changed = match &n.w {
        W::Checkbox { checked, .. } if Some(Value::Bool(*checked)) != before => Some((kinds::P::Checked, Value::Bool(*checked))),
        W::Slider { value, .. } if Some(Value::F64(*value)) != before => Some((kinds::P::Value, Value::F64(*value))),
        W::TextEdit { .. } => text_changed.map(|t| (kinds::P::Text, Value::Str(t))),
        _ => None,
    };
    if let Some((key, v)) = changed {
        let prop = kinds::prop_id(n.kind, key);
        let s = sig("changed");
        if n.observes(prop) {
            out.push(Out::Changed(h, prop));
        }
        if n.listens(s) {
            out.push(Out::Event(h, s, vec![v]));
        }
    }
}
