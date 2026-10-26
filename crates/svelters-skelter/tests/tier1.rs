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
    // egui's signals are the menu-item's own (A9b: its menu root emits them on it)
    for s in ["activate", "toggled", "hover"] {
        assert!(m.signal_named(s).unwrap().native.is_some(), "{s}");
    }
}

/// The context menu's open level's row rects, in its node's points.
fn menu_rows(r: &Rig, menu: svl_handle) -> Vec<skelter_base::geom::Rect> {
    let n = r.backend.node(menu).unwrap();
    r.backend
        .with_doc(|d| {
            d.with_kind(n, |k| {
                let c = k.as_any().and_then(|a| a.downcast_ref::<skelter_widgets::views::menu::ContextMenu>()).expect("skelter-widgets' context menu");
                c.menus.levels().first().map(|l| l.rows.iter().map(|(_, r)| *r).collect()).unwrap_or_default()
            })
        })
        .unwrap()
}

/// A primary pointer event at `p` in window 1, delivered to the host.
fn pointer_at(r: &Rig, n: &mut u64, phase: skelter_base::event::PointerPhase, p: skelter_base::geom::Point) {
    use skelter_base::event::{InputEnvelope, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
    *n += 1;
    let ev = PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if phase == PointerPhase::Down { 1 } else { 0 },
        pos: p,
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: skelter_base::geom::Vec2::ZERO,
        history: Default::default(),
    };
    let t = skelter_base::time::Time(*n * 100_000_000);
    r.backend.dispatch(&InputEnvelope { seq: skelter_base::ids::InputSeq(*n), window: skelter_base::ids::WindowId(1), os_time: t, recv_time: t, event: InputEvent::Pointer(ev) });
    r.backend.pump();
}

/// svelters-egui's action events (tests/menus.rs, OW3): picking an action fires its own
/// `activate(checked)`, a check action also `toggled(checked)`, and the current row's `hover`.
#[test]
fn action_fires_its_own_activate_toggled_and_hover() {
    use skelter_base::event::PointerPhase;
    use skelter_base::geom::{Point, Scale, Size};
    use skelter_base::ids::WindowId;
    use skelter_base::time::Time;
    let r = Rig::new();
    let root = r.create("box");
    r.backend.attach_presented(root, WindowId(1), Size::new(400.0, 300.0), Scale(1.0)).unwrap();
    let menu = r.create("context-menu");
    r.insert(root, menu);
    let items = r.br().resolve(NameKind::Slot, r.kind(menu), "items");
    let (cut, wrap) = (r.create("action"), r.create("action"));
    r.br().insert(menu, cut, None, items).unwrap();
    r.br().insert(menu, wrap, None, items).unwrap();
    r.set(cut, "label", Value::Str("Cut".into()));
    r.set(wrap, "label", Value::Str("Wrap".into()));
    r.set(wrap, "checkable", Value::Bool(true));
    for (h, s) in [(cut, "activate"), (cut, "hover"), (wrap, "activate"), (wrap, "toggled")] {
        r.listen(h, s);
    }
    let frame = || r.backend.with_doc(|d| d.frame(WindowId(1), Time(0)));
    frame();
    r.call(menu, "popup");
    frame();
    let rows = menu_rows(&r, menu);
    assert_eq!(rows.len(), 2, "open with both rows");
    let origin = r.backend.with_doc(|d| d.layout_rect(r.backend.node(menu).unwrap())).unwrap().min;
    let at = |k: usize| Point::new(origin.x + (rows[k].min.x + rows[k].max.x) / 2.0, origin.y + (rows[k].min.y + rows[k].max.y) / 2.0);
    let mut n = 0;
    for phase in [PointerPhase::Move, PointerPhase::Down, PointerPhase::Up] {
        pointer_at(&r, &mut n, phase, at(0));
    }
    let ev = r.take().events;
    assert!(ev.contains(&(cut, r.signal_id(cut, "hover"), vec![])), "{ev:?}");
    assert!(ev.contains(&(cut, r.signal_id(cut, "activate"), vec![Value::Bool(false)])), "{ev:?}");
    // the check action: toggled, then activate, with the new state
    r.call(menu, "popup");
    frame();
    for phase in [PointerPhase::Move, PointerPhase::Down, PointerPhase::Up] {
        pointer_at(&r, &mut n, phase, at(1));
    }
    let ev: Vec<_> = r.take().events.into_iter().filter(|e| e.0 == wrap).collect();
    assert_eq!(ev, [(wrap, r.signal_id(wrap, "toggled"), vec![Value::Bool(true)]), (wrap, r.signal_id(wrap, "activate"), vec![Value::Bool(true)])]);
}

#[test]
fn collapsing_header_has_eguis_surface() {
    let r = Rig::new();
    let kinds = r.backend.kinds().clone();
    let m = kinds.by_tag("collapsing-header").expect("collapsing-header");
    assert!(m.hosted, "skelter-widgets' collapsing header, not a placeholder");
    assert!(m.signal_named("changed").unwrap().native.is_some(), "egui's `changed(open)` is the kind's own");
    let h = r.create("collapsing-header");
    r.insert(h, r.create("label"));
    let end = r.br().resolve(NameKind::Slot, r.kind(h), "end");
    assert_ne!(end, 0, "egui's `end` slot");
    r.br().insert(h, r.create("button"), None, end).unwrap();
    r.set(h, "text", Value::Str("Details".into()));
    assert_eq!(r.get(h, "text"), Value::Str("Details".into()));
    r.set(h, "open", Value::Bool(true));
    assert_eq!(r.get(h, "open"), Value::Bool(true));
}

/// A primary click at (60, `y`) in window 1, delivered to the host.
fn click_at(r: &Rig, seq: &mut u64, y: f32) {
    use skelter_base::event::{InputEnvelope, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
    use skelter_base::geom::{Point, Vec2};
    use skelter_base::ids::{InputSeq, WindowId};
    use skelter_base::time::Time;
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        *seq += 1;
        let p = PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: Some(MouseButton::Primary),
            buttons: if phase == PointerPhase::Down { 1 } else { 0 },
            pos: Point::new(60.0, y),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        };
        let t = Time(*seq * 1_000_000);
        r.backend.dispatch(&InputEnvelope { seq: InputSeq(*seq), window: WindowId(1), os_time: t, recv_time: t, event: InputEvent::Pointer(p) });
    }
    r.backend.pump();
}

/// svelters-egui's `collapsing_header_toggles_open_and_lays_out_its_body` (tests/widgets.rs) on
/// skelter: closed hides its children; a click opens it (announced, `changed`), the children are
/// laid out below the header and indented; a host write opens it without an echo.
#[test]
fn collapsing_header_toggles_open_and_lays_out_its_body() {
    use skelter_base::accesskit::{self, TreeUpdate};
    use skelter_base::geom::{Scale, Size};
    use skelter_base::ids::WindowId;
    use skelter_base::time::Time;
    let r = Rig::new();
    let col = r.create("box");
    r.backend.attach_presented(col, WindowId(1), Size::new(300.0, 200.0), Scale(1.0)).unwrap();
    let h = r.create("collapsing-header");
    r.insert(col, h);
    r.set(h, "text", Value::Str("Placement".into()));
    // (no text system here: the body is a box that has a size of its own)
    let x = r.create("box");
    r.set(x, "padding", Value::F64(10.0));
    r.insert(h, x);
    let after = r.create("label");
    r.insert(col, after);
    r.observe(h, "open", true);
    r.listen(h, "changed");
    let (hn, xn, an) = (r.backend.node(h).unwrap(), r.backend.node(x).unwrap(), r.backend.node(after).unwrap());
    let frame = || r.backend.with_doc(|d| d.frame(WindowId(1), Time(0))).and_then(|c| c.a11y);
    let expanded = |u: &Option<TreeUpdate>| {
        let u = u.as_ref().expect("an accesskit update");
        let n = u.nodes.iter().find(|(id, _)| id.0 == hn.0).map(|(_, n)| n).expect("the header's node");
        assert_eq!(n.role(), accesskit::Role::DisclosureTriangle);
        assert_eq!(n.label(), Some("Placement"));
        n.is_expanded()
    };
    let in_tree = |u: &Option<TreeUpdate>| u.as_ref().is_some_and(|u| u.nodes.iter().any(|(_, n)| n.children().iter().any(|c| c.0 == xn.0)));
    let rect = |n| r.backend.with_doc(|d| d.layout_rect(n)).unwrap();
    let u = frame();
    assert_eq!(expanded(&u), Some(false));
    assert!(!in_tree(&u), "closed: the body is not in the accessibility tree");
    assert_eq!(rect(xn).height(), 0.0, "closed: the body is not laid out");
    let closed_after = rect(an).min.y;
    assert!(closed_after >= 20.0, "the header row: {closed_after}");
    let mut seq = 0;
    click_at(&r, &mut seq, 10.0);
    let l = r.take();
    let open = r.prop_id(h, "open");
    assert_eq!(l.changed, vec![(h, open)]);
    assert_eq!(l.events, [(h, r.signal_id(h, "changed"), vec![Value::Bool(true)])]);
    assert_eq!(r.get(h, "open"), Value::Bool(true));
    let u = frame();
    assert_eq!(expanded(&u), Some(true));
    assert!(in_tree(&u), "open: the body is announced in place");
    let xr = rect(xn);
    assert!(xr.min.y >= 20.0 && xr.height() > 0.0, "the body is below the header: {xr:?}");
    assert!(xr.min.x >= 10.0, "and indented: {xr:?}");
    assert!(rect(an).min.y > closed_after + 10.0, "the next sibling moved down");

    // closing again
    click_at(&r, &mut seq, 10.0);
    assert_eq!(r.get(h, "open"), Value::Bool(false));
    frame();
    assert_eq!(rect(xn).height(), 0.0);
    assert!((rect(an).min.y - closed_after).abs() < 0.5);
    r.take();

    // a host write: shown, never announced
    r.set(h, "open", Value::Bool(true));
    frame();
    assert!(rect(xn).height() > 0.0);
    r.backend.pump();
    assert!(r.take().changed.is_empty());
}
