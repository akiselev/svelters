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

use crate::source::SceneSource;
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
        // OW1b: own widgets; their props read by name
        W::Ext(_) => {
            let f = |p: &str| match n.value_of(p) {
                Ok(svelters_abi::Value::F64(v)) => Some(v),
                _ => None,
            };
            match n.tag() {
                "slider" | "drag-value" => f("value").map(num),
                "range-slider" => Some(format!("{}..{}", num(f("low")?), num(f("high")?))),
                _ => None,
            }
        }
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

/// Appends the YAML lines of `n`'s subtree at `depth` (any [`SceneSource`]; the egui scene is one).
pub(crate) fn yaml_in<S: SceneSource>(s: &S, n: S::Node, depth: usize, out: &mut String) {
    if !s.is_visible(n) {
        return;
    }
    let role = s.role(n);
    let name = s.name(n);
    let children = s.children(n);
    if role == "generic" && name.is_empty() {
        for c in children {
            yaml_in(s, c, depth, out);
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
    for st in s.states(n) {
        // pointer and focus states are transient: a snapshot is a regression artefact
        if !matches!(st, "hovered" | "pressed" | "focused" | "focus-within" | "focus-visible") {
            out.push_str(&format!(" [{st}]"));
        }
    }
    let kids = !children.is_empty();
    match s.value(n) {
        Some(v) if !kids => out.push_str(&format!(
            ": {}",
            if v.is_empty() { "\"\"".to_string() } else { v }
        )),
        _ if kids => out.push(':'),
        _ => {}
    }
    out.push('\n');
    for c in children {
        yaml_in(s, c, depth + 1, out);
    }
}
