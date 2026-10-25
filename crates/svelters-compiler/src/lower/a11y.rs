//! W-C9: accessibility checks from the spec's `a11y` facts (presto `compiler-and-js.md` §3.9):
//! what egui's kittest reports at run time ("widget(s) lack accessible name"), at compile time.
//!
//! An element whose kind `requires_name` is named when the element sets one of the kind's
//! `name_props` (statically to a non-empty value, or dynamically, or through a binding), or its
//! `labelled_by` prop. Otherwise it gets `a11y_icon_only_control` when it sets one of the kind's
//! `icon_props` (an icon alone names nothing), else `a11y_missing_name`. An element with a spread
//! is not checked (its props are known at run time only).

use super::ir::*;
use crate::diag::Diagnostics;
use crate::semantics::PlatformSemantics;
use serde_json::Value as Json;

pub fn check(f: &Fragment, sem: &dyn PlatformSemantics, diags: &mut Diagnostics) {
    super::change::each_element(f, &mut |e| element(e, sem, diags));
}

/// The prop an op sets, and whether it surely sets nothing (a static empty or null value).
fn written<'o>(op: &'o Op<'_>) -> Option<(&'o str, bool)> {
    match op {
        Op::Set { prop, value, .. } => Some((prop, matches!(value, Json::Null) || value.as_str() == Some(""))),
        Op::SetDynamic { prop, .. }
        | Op::Bind { prop, .. }
        | Op::BindProp { prop, .. }
        | Op::BindReadonly { prop, .. }
        | Op::BindOnce { prop, .. }
        | Op::BindCommit { prop, .. }
        | Op::BindLink { prop, .. } => Some((prop, false)),
        _ => None,
    }
}

fn element(e: &Element, sem: &dyn PlatformSemantics, diags: &mut Diagnostics) {
    let Ok(el) = sem.resolve_element(&e.kind) else { return };
    let Some(a) = sem.a11y(&el) else { return };
    if !a.requires_name || e.ops.iter().any(|op| matches!(op, Op::Spread { .. })) {
        return;
    }
    let sets = |props: &[String]| e.ops.iter().filter_map(written).any(|(p, empty)| !empty && props.iter().any(|n| n == p));
    let labelled = a.labelled_by.as_ref().is_some_and(|l| sets(std::slice::from_ref(l)));
    if labelled || sets(&a.name_props) {
        return;
    }
    let role = a.role.as_deref().unwrap_or("control");
    let names = a.name_props.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ");
    let span = (e.span.start, e.span.start + e.kind.len() as u32 + 1);
    if sets(&a.icon_props) {
        diags
            .native_warning("a11y_icon_only_control", span, format!("<{}> shows only an icon: assistive technology gets no name for this {role}", e.kind))
            .help(format!("give it a name with one of {names}"));
    } else {
        diags
            .native_warning("a11y_missing_name", span, format!("<{}> has no accessible name: assistive technology announces an unnamed {role}", e.kind))
            .help(format!("set one of {names}"));
    }
}
