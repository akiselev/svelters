//! `scene.snapshot` (SG7, folded into AU1): a text snapshot of a subtree in Playwright's aria
//! snapshot shape, the default regression artefact (§1.8).
//!
//! ```yaml
//! - button "Increment"
//! - textbox "Name": Ann
//! - slider "Level": 42
//! - checkbox "Level 42, agree: yes" [checked]
//! ```
//!
//! Nodes without a role of their own (`generic`) and no text are flattened: their children take
//! their place. A `label` shows its text as its name. Pointer and focus states are left out.

use crate::role::{role_of, states_of};
use svelters_egui::accessible_name;
use svelters_egui::scene::{NodeRef, W};

/// The value a snapshot shows after the name (`: value`), for value-bearing roles.
pub(crate) fn value_of(n: NodeRef<'_>) -> Option<String> {
    let num = |v: f64| {
        if v.fract() == 0.0 {
            format!("{v:.0}")
        } else {
            format!("{v}")
        }
    };
    match n.builtin() {
        W::TextEdit {
            text,
            password: false,
            ..
        } => Some(text.clone()),
        W::Slider { value, .. } => Some(num(*value)),
        W::DragValue { value, .. } => Some(num(*value)),
        W::RangeSlider { low, high, .. } => Some(format!("{}..{}", num(*low), num(*high))),
        W::ComboBox {
            options, selected, ..
        } => usize::try_from(*selected)
            .ok()
            .and_then(|i| options.get(i))
            .cloned(),
        W::ProgressBar { fraction, .. } => Some(num(*fraction)),
        _ => None,
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Appends the YAML lines of `n`'s subtree at `depth`.
pub(crate) fn yaml(n: NodeRef<'_>, depth: usize, out: &mut String) {
    if !n.is_visible() {
        return;
    }
    let role = role_of(n);
    let name = accessible_name(n);
    if role == "generic" && name.is_empty() {
        for c in n.children() {
            yaml(c, depth, out);
        }
        return;
    }
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str("- ");
    out.push_str(role);
    if !name.is_empty() {
        out.push(' ');
        out.push_str(&quote(&name));
    }
    for s in states_of(n) {
        // pointer and focus states are transient: a snapshot is a regression artefact
        if !matches!(s, "hovered" | "pressed" | "focused" | "focus-within") {
            out.push_str(&format!(" [{s}]"));
        }
    }
    let kids = n.children().count() > 0;
    match value_of(n) {
        Some(v) if !kids => out.push_str(&format!(
            ": {}",
            if v.is_empty() { "\"\"".to_string() } else { v }
        )),
        _ if kids => out.push(':'),
        _ => {}
    }
    out.push('\n');
    for c in n.children() {
        yaml(c, depth + 1, out);
    }
}
