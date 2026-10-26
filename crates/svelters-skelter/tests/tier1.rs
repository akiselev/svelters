//! A9: skelter-widgets' tier-1 views as JS-facing kinds with svelters-egui's names (`surface::tier1`):
//! each is skelter-widgets' kind (not a placeholder) where it has one, created through the backend,
//! a prop written and read back, a signal delivered.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_skelter::surface::Value as PValue;

#[test]
fn scroll_area_is_skelter_widgets_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("scroll-area").expect("scroll-area");
    assert!(m.hosted, "skelter-widgets' scroll area, not a placeholder");
    assert_eq!(m.surface.extends.map(|b| b.name), Some("Box"), "egui's chain");
    let s = r.create("scroll-area");
    r.insert(s, r.create("label"));
    // props: egui's names onto the core kind's
    r.set(s, "direction", Value::Str("both".into()));
    assert_eq!(r.get(s, "direction"), Value::Str("both".into()));
    r.set(s, "scroll-y", Value::F64(40.0));
    assert_eq!(r.get(s, "scroll-y"), Value::F64(40.0));
    // the signal: egui's `changed(scroll-x, scroll-y)` is the kind's own
    assert!(m.signal_named("changed").unwrap().native.is_some());
    r.listen(s, "changed");
    r.emit(s, "changed", vec![PValue::F64(0.0), PValue::F64(40.0)]);
    r.backend.pump();
    assert_eq!(r.take().events, [(s, r.signal_id(s, "changed"), vec![Value::F64(0.0), Value::F64(40.0)])]);
}

#[test]
fn table_and_columns_are_skelter_widgets_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    assert!(kinds.by_tag("table").unwrap().hosted && kinds.by_tag("table-column").unwrap().hosted);
    let t = r.create("table");
    let c = r.create("table-column");
    let columns = r.br().resolve(NameKind::Slot, r.kind(t), "columns");
    assert_ne!(columns, 0, "egui's `columns` slot");
    r.br().insert(t, c, None, columns).unwrap();
    r.set(c, "title", Value::Str("Name".into()));
    assert_eq!(r.get(c, "title"), Value::Str("Name".into()));
    r.set(t, "striped", Value::Bool(false));
    assert_eq!(r.get(t, "striped"), Value::Bool(false));
    r.set(t, "selection", Value::Str("multi".into()));
    assert_eq!(r.get(t, "selection"), Value::Str("multi".into()));
    // egui lists no params for generated methods: the core method's type the args
    let m = r.br().resolve(NameKind::Method, r.kind(t), "set-ids");
    r.br().call(t, m, &[Value::List(vec![Value::I64(1), Value::I64(2)])]).unwrap();
    // a cell editor snippet (egui's `other`) is kept by the backend
    let ed = r.prop_id(c, "editor");
    r.br().set_prop(c, ed, &Value::Str("snippet".into())).unwrap();
    r.listen(t, "changed");
    r.emit(t, "changed", vec![PValue::List(vec![PValue::I64(2)])]);
    r.backend.pump();
    assert_eq!(r.take().events, [(t, r.signal_id(t, "changed"), vec![Value::List(vec![Value::I64(2)])])]);
}

#[test]
fn tree_view_is_skelter_widgets_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("tree-view").unwrap();
    assert!(m.hosted);
    assert!(matches!(m.prop_named("current").unwrap().target, svelters_skelter::kinds::PropTarget::Native(_)));
    let t = r.create("tree-view");
    r.set(t, "current", Value::I64(7));
    assert_eq!(r.get(t, "current"), Value::I64(7));
    r.set(t, "expand-on", Value::Str("click".into()));
    assert_eq!(r.get(t, "expand-on"), Value::Str("click".into()));
    r.listen(t, "expand");
    r.emit(t, "expand", vec![PValue::I64(3)]);
    r.backend.pump();
    assert_eq!(r.take().events, [(t, r.signal_id(t, "expand"), vec![Value::I64(3)])]);
}

#[test]
fn combo_box_is_skelter_widgets_select_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("combo-box").unwrap();
    assert!(m.hosted);
    assert_eq!(m.core, Some(skelter_core::KindId::of("select")), "egui's combo-box is skelter-widgets' select");
    assert!(kinds.by_tag("select").is_none(), "not a tag of its own");
    let c = r.create("combo-box");
    r.set(c, "options", Value::List(vec![Value::Str("mm".into()), Value::Str("in".into())]));
    r.set(c, "selected", Value::I64(1));
    assert_eq!(r.get(c, "selected"), Value::I64(1));
    let n = r.backend.node(c).unwrap();
    assert_eq!(r.backend.with_doc(|d| d.get_prop(n, skelter_core::PropId::of("select", "selected", "int"))), Some(skelter_core::Value::I64(1)));
    r.listen(c, "changed");
    r.emit(c, "changed", vec![PValue::I64(0)]);
    r.backend.pump();
    assert_eq!(r.take().events, [(c, r.signal_id(c, "changed"), vec![Value::I64(0)])]);
}

#[test]
fn context_menu_is_skelter_widgets_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("context-menu").unwrap();
    assert!(m.hosted);
    assert_eq!(m.surface.extends.map(|b| b.name), Some("MenuEntry"));
    let c = r.create("context-menu");
    let n = r.backend.node(c).unwrap();
    // MenuEntry's `enabled` is core's common `disabled`
    r.set(c, "enabled", Value::Bool(false));
    assert_eq!(r.get(c, "enabled"), Value::Bool(false));
    r.set(c, "enabled", Value::Bool(true));
    // egui's methods open and close it through the core kind's `open`
    let open = skelter_core::PropId::of("context-menu", "open", "bool");
    r.call(c, "popup");
    assert_eq!(r.backend.with_doc(|d| d.get_prop(n, open)), Some(skelter_core::Value::Bool(true)));
    r.call(c, "popdown");
    assert_eq!(r.backend.with_doc(|d| d.get_prop(n, open)), Some(skelter_core::Value::Bool(false)));
}

#[test]
fn action_is_skelter_widgets_menu_item_with_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("action").unwrap();
    assert!(m.hosted);
    assert_eq!(m.core, Some(skelter_core::KindId::of("menu-item")), "egui's action is skelter-widgets' menu-item");
    let menu = r.create("context-menu");
    let a = r.create("action");
    let items = r.br().resolve(NameKind::Slot, r.kind(menu), "items");
    r.br().insert(menu, a, None, items).unwrap();
    r.set(a, "label", Value::Str("Save".into()));
    r.set(a, "checkable", Value::Bool(true));
    r.set(a, "checked", Value::Bool(true));
    assert_eq!(r.get(a, "label"), Value::Str("Save".into()));
    assert_eq!(r.get(a, "checked"), Value::Bool(true));
    let n = r.backend.node(a).unwrap();
    assert_eq!(r.backend.with_doc(|d| d.get_prop(n, skelter_core::PropId::of("menu-item", "label", "string"))), Some(skelter_core::Value::Str("Save".into())));
    // egui's signals resolve and take listeners; skelter-widgets' menu root emits the picks itself
    // (`activate` on the context-menu), so an action's own `activate` does not fire yet
    r.listen(a, "activate");
    assert!(m.signal_named("activate").unwrap().native.is_none());
}

#[test]
fn collapsing_header_has_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("collapsing-header").expect("collapsing-header");
    // skelter-widgets has no collapsing header yet: a placeholder holds egui's members
    assert!(!m.hosted);
    let h = r.create("collapsing-header");
    r.insert(h, r.create("label"));
    let end = r.br().resolve(NameKind::Slot, r.kind(h), "end");
    assert_ne!(end, 0, "egui's `end` slot");
    r.br().insert(h, r.create("button"), None, end).unwrap();
    r.set(h, "text", Value::Str("Details".into()));
    assert_eq!(r.get(h, "text"), Value::Str("Details".into()));
    r.set(h, "open", Value::Bool(true));
    assert_eq!(r.get(h, "open"), Value::Bool(true));
    r.listen(h, "changed");
    r.emit(h, "changed", vec![PValue::Bool(false)]);
    r.backend.pump();
    assert_eq!(r.take().events, [(h, r.signal_id(h, "changed"), vec![Value::Bool(false)])]);
}
