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
