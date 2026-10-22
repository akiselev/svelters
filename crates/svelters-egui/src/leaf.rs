//! Built-in leaf painting: one egui widget per leaf node at its rect, with its response turned into
//! events and widget-side prop changes in the outbox.
//!
//! Writer lane: WIDGETS.

use crate::kinds;
use crate::scene::{Arena, Handle, W};
use crate::walk::Out;
use egui::Ui;
use svelters_abi::Value;

pub(crate) fn leaf(arena: &mut Arena, h: Handle, ui: &mut Ui, out: &mut Vec<Out>) {
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
            // 4 points of slack: the value box can come out slightly wider than interact_size.x.
            ui.spacing_mut().slider_width = (size.x - extra - 4.0).max(sp.interact_size.x);
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
        W::Ext(_) => unreachable!("provider kinds are drawn by provider::paint"),
    };
    let n = arena.n_mut(h);
    let mut resp = crate::a11y::decorate(resp, n);
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
