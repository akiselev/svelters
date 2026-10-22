//! Built-in leaf painting: one egui widget per leaf node at its rect, with its response turned into
//! events and widget-side prop changes in the outbox.
//!
//! Writer lane: WIDGETS.

use crate::kinds;
use crate::scene::{Arena, NodeFlags, W};
use crate::walk::Out;
use egui::Ui;
use svelters_abi::Value;

/// Paints the leaf in slot `i` (the walk's index into the scene's columns).
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) {
    let h = arena.slots.id_at(i).expect("live leaf").to_raw();
    let kind = arena.core[i].kind as usize;
    let sig = |name: &str| kinds::signal_id(kind, name);
    let size = arena.core[i].rect.size();
    let node_id = arena.core[i].egui_id;
    let click_sense = arena.listens(i, sig("clicked"));
    let mut text_changed = None;
    let before = match arena.w(i) {
        W::Checkbox { checked, .. } => Some(Value::Bool(*checked)),
        W::Slider { value, .. } => Some(Value::F64(*value)),
        _ => None,
    };
    let resp = match arena.w_mut(i) {
        W::Label { text, wrap, selectable } => {
            let mode = match *wrap {
                "truncate" => egui::TextWrapMode::Truncate,
                "extend" => egui::TextWrapMode::Extend,
                _ => egui::TextWrapMode::Wrap,
            };
            let mut l = egui::Label::new(text.as_str()).wrap_mode(mode).selectable(*selectable);
            if click_sense {
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
            te = te.id(node_id).desired_width(size.x).hint_text(hint.clone()).password(*password);
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
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    if !arena.common(i).tooltip.is_empty() {
        resp = resp.on_hover_text(arena.common(i).tooltip.clone());
    }
    let core = &mut arena.core[i];
    if core.flags.contains(NodeFlags::WANT_FOCUS) {
        core.flags.remove(NodeFlags::WANT_FOCUS);
        resp.request_focus();
    }
    core.drawn = resp.rect;
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
    let had_focus = arena.core[i].flags.contains(NodeFlags::HAD_FOCUS);
    if matches!(arena.w(i), W::TextEdit { .. }) && focused != had_focus {
        fired.push(if focused { "gained-focus" } else { "lost-focus" });
    }
    arena.core[i].flags.set(NodeFlags::HAD_FOCUS, focused);
    for name in fired {
        let s = sig(name);
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![]));
        }
    }
    // Widget-side value changes: the node already holds the new value (the widget wrote the
    // field); announce it if observed (exact, no echo), then the `changed` event.
    // Compared with the value before the widget ran, so a slider clamping a host-set value into its
    // range is announced too (the binding's `natively_normalizes`).
    let changed = match arena.w(i) {
        W::Checkbox { checked, .. } if Some(Value::Bool(*checked)) != before => Some((kinds::P::Checked, Value::Bool(*checked))),
        W::Slider { value, .. } if Some(Value::F64(*value)) != before => Some((kinds::P::Value, Value::F64(*value))),
        W::TextEdit { .. } => text_changed.map(|t| (kinds::P::Text, Value::Str(t))),
        _ => None,
    };
    if let Some((key, v)) = changed {
        // a user edit is a change of the node (ticks, ARCH-SCENE §6.5)
        let t = arena.stamp();
        arena.core[i].changed = t;
        let prop = kinds::prop_id(kind, key);
        // SG2: journaled as a widget's prop write
        let node = arena.slots.id_at(i).expect("live leaf");
        arena.record(crate::scene::Change::Prop { node, prop: crate::scene::PropId(prop), origin: crate::scene::Origin::Widget }, crate::scene::Origin::Widget);
        let s = sig("changed");
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![v]));
        }
    }
}
