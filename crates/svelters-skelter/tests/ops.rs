//! The backend ABI on skelter's tree, through the real vtable: resolve, create, insert (slots, moves,
//! cycles), remove, destroy, set/get with typed conversions, listen flags, observe, call, handle
//! info, describe, style.

mod common;

use common::Rig;
use svelters_abi::*;

#[test]
fn resolve_names_round_trip_to_stable_ids() {
    let r = Rig::new();
    let b = r.create("button");
    let k = r.kind(b);
    assert_ne!(k, 0);
    // an inherited prop has its base kind's id on every kind
    let on_button = r.br().resolve(NameKind::Prop, k, "enabled");
    let l = r.create("label");
    assert_eq!(on_button, r.br().resolve(NameKind::Prop, r.kind(l), "enabled"));
    assert_ne!(r.br().resolve(NameKind::Prop, k, "text"), r.br().resolve(NameKind::Prop, r.kind(l), "text"), "own props differ per kind");
    assert_eq!(r.br().resolve(NameKind::Prop, k, "no-such-prop"), 0);
    assert_eq!(r.br().resolve(NameKind::Kind, 0, "no-such-tag"), 0);
}

#[test]
fn abstract_kinds_are_not_created() {
    let r = Rig::new();
    let widget = svelters_skelter::kinds::kind_wire(&svelters_skelter::surface::tier0::WIDGET);
    assert!(r.backend.kinds().by_wire(widget).is_some());
    assert!(r.br().create(widget).is_err());
}

#[test]
fn set_and_get_convert_by_type() {
    let r = Rig::new();
    let s = r.create("slider");
    r.set(s, "value", Value::I64(20));
    assert_eq!(r.get(s, "value"), Value::F64(20.0));
    let bx = r.create("box");
    r.set(bx, "flex-direction", Value::Str("row".into()));
    assert_eq!(r.get(bx, "flex-direction"), Value::Str("row".into()));
    assert!(r.br().set_prop(bx, r.prop_id(bx, "flex-direction"), &Value::Str("diagonal".into())).is_err());
    // nullable props take null, others do not
    r.set(bx, "width", Value::Null);
    assert_eq!(r.get(bx, "width"), Value::Null);
    assert!(r.br().set_prop(bx, r.prop_id(bx, "gap"), &Value::Null).is_err());
    // a never-written prop reads its default
    assert_eq!(r.get(r.create("window"), "default-width"), Value::F64(480.0));
}

#[test]
fn insert_orders_moves_and_refuses_cycles() {
    let r = Rig::new();
    let (p, a, b, c) = (r.create("box"), r.create("label"), r.create("label"), r.create("box"));
    r.insert(p, a);
    r.insert(p, b);
    r.br().insert(p, c, Some(a), 0).unwrap();
    let kids = |h| {
        let n = r.backend.node(h).unwrap();
        r.backend.with_doc(|d| d.children(n).iter().map(|&k| r.backend.handle(k).unwrap()).collect::<Vec<_>>())
    };
    assert_eq!(kids(p), [c, a, b]);
    // moving within the parent
    r.br().insert(p, b, Some(c), 0).unwrap();
    assert_eq!(kids(p), [b, c, a]);
    // a box inside its own child is a cycle
    assert!(r.br().insert(c, p, None, 0).is_err());
    // a label takes runs, not boxes; a button takes no children at all
    assert!(r.br().insert(r.create("button"), r.create("label"), None, 0).is_err());
    r.br().remove(p, a).unwrap();
    assert_eq!(kids(p), [b, c]);
    assert!(r.br().remove(p, a).is_err(), "not a child any more");
}

#[test]
fn destroy_frees_the_handle_and_orphans_children() {
    let r = Rig::new();
    let (p, a) = (r.create("box"), r.create("label"));
    r.insert(p, a);
    r.br().destroy(p);
    assert!(r.br().handle_info(p).is_err());
    let n = r.backend.node(a).unwrap();
    assert_eq!(r.backend.with_doc(|d| d.parent(n)), None);
    // handles are never reused
    let again = r.create("box");
    assert!(again > a);
}

#[test]
fn handle_info_reports_lifetimes() {
    let r = Rig::new();
    let w = r.create("window");
    assert_eq!(r.br().handle_info(w).unwrap().lifetime, SVL_LIFE_TOPLEVEL);
    assert_eq!(r.br().handle_info(r.create("box")).unwrap().lifetime, SVL_LIFE_OWNED);
    let slot = r.backend.create_borrowed("box").unwrap();
    assert_eq!(r.br().handle_info(slot).unwrap().lifetime, SVL_LIFE_BORROWED);
    r.br().destroy(slot);
    assert!(r.br().handle_info(slot).is_ok(), "JS cannot destroy a slot");
}

#[test]
fn listen_flags_capture_and_passive() {
    let r = Rig::new();
    let b = r.create("button");
    let click = r.signal_id(b, "click");
    r.br().listen_ex(b, click, true, SVL_LISTEN_CAPTURE).unwrap();
    r.br().listen_ex(b, click, true, svelters_skelter::SVL_LISTEN_PASSIVE).unwrap();
    // widget signals do not propagate
    assert!(r.br().listen_ex(b, r.signal_id(b, "clicked"), true, SVL_LISTEN_CAPTURE).is_err());
    assert!(r.br().listen_ex(b, click, true, 1 << 9).is_err());
}

#[test]
fn present_and_close_windows() {
    let r = Rig::new();
    let w = r.create("window");
    r.call(w, "present");
    r.call(w, "present");
    assert_eq!(r.backend.windows(), [w]);
    r.call(w, "close");
    assert_eq!(r.backend.take_closes(), [w]);
    r.backend.window_closed(w);
    assert!(r.backend.windows().is_empty());
    assert_eq!(r.take().gone, [(w, SVL_GONE_CLOSED)]);
    // focus needs a window; scrolling into view is not on skelter yet
    let t = r.create("text-edit");
    let m = r.br().resolve(NameKind::Method, r.kind(t), "request-focus");
    assert!(r.br().call(t, m, &[]).unwrap_err().message.contains("not in a window"));
    let m = r.br().resolve(NameKind::Method, r.kind(t), "scroll-into-view");
    assert!(r.br().call(t, m, &[]).unwrap_err().message.contains("not implemented"));
}

#[test]
fn describe_gives_the_type_tag() {
    let r = Rig::new();
    let s = r.create("slider");
    let d = r.br().describe(NameKind::Prop, r.kind(s), r.prop_id(s, "value")).unwrap();
    assert_eq!(d.params[0].tag, SVL_F64);
    let bx = r.create("box");
    let w = r.br().describe(NameKind::Prop, r.kind(bx), r.prop_id(bx, "width")).unwrap();
    assert!(w.params[0].has(SVL_D_NULLABLE));
}

#[test]
fn style_ops_reach_the_tree() {
    let r = Rig::new();
    let b = r.create("box");
    assert_eq!(r.br().style(SVL_STYLE_TARGET, 0, None).unwrap(), svelters_skelter::style::SVL_STYLE_TARGET_ENGINE);
    r.br().style(SVL_STYLE_CLASSES, b, Some("column svelte-1 column")).unwrap();
    r.br().style(SVL_STYLE_NAME, b, Some("main")).unwrap();
    r.br().style(SVL_STYLE_OPACITY, b, Some("0.5")).unwrap();
    let n = r.backend.node(b).unwrap();
    let (style, vis) = (r.backend.style_input(n).unwrap(), r.backend.visual(n));
    assert_eq!(style.classes.iter().map(|c| &**c).collect::<Vec<_>>(), ["column", "svelte-1"]);
    assert_eq!(style.id.as_deref(), Some("main"));
    assert_eq!(vis.opacity, Some(0.5));
    let sheet = format!("app.svelte{SVL_STYLE_SEP}.column {{ gap: 8px }}");
    r.br().style(SVL_STYLE_SHEET, 0, Some(&sheet)).unwrap();
    assert_eq!(r.backend.sheets().get("app.svelte").cloned(), Some(".column { gap: 8px }".into()));
    r.br().style(SVL_STYLE_SHEET, 0, None).unwrap();
    assert!(r.backend.sheets().is_empty());
    // skelter plays animations itself (tests/motion.rs); a bad payload is an error
    assert!(r.br().style(SVL_STYLE_ANIMATE, b, Some("x")).is_err());
}

#[test]
fn timers_frames_and_wakes_go_through_the_driver_seam() {
    let r = Rig::new();
    r.br().schedule_wake(10);
    assert!(r.backend.deadline().is_some());
    r.br().schedule_wake(-1);
    assert!(r.backend.deadline().is_none());
    r.br().request_frame();
    r.backend.frame();
    r.backend.frame();
    assert_eq!(r.take().frames, 1, "one frame per request");
    r.br().wake();
    r.backend.poll_wake();
    assert_eq!(r.take().checkpoints, 1);
}

/// A27: the host's provider states resolve `:state(ns.name)` in the app's sheets.
#[test]
fn a_state_rule_in_an_app_sheet_matches_the_hosts_bit() {
    use skelter_base::geom::{Scale, Size};
    use skelter_base::ids::WindowId;
    let r = Rig::new();
    let pending = svelters_style::StateMask(1 << 40);
    r.backend.set_state_names(Box::new([("cad.pending", pending)]));
    let sheet = format!("app.svelte{SVL_STYLE_SEP}box:state(cad.pending) {{ width: 33px }}");
    r.br().style(SVL_STYLE_SHEET, 0, Some(&sheet)).unwrap();
    let root = r.create("box");
    let b = r.create("box");
    r.insert(root, b);
    r.backend.attach_presented(root, WindowId(1), Size::new(200.0, 100.0), Scale(1.0)).unwrap();
    let n = r.backend.node(b).unwrap();
    let doc = r.backend.document();
    doc.borrow_mut().set_state(n, pending, true);
    doc.borrow_mut().frame(WindowId(1), skelter_base::time::Time(0));
    assert_eq!(doc.borrow().world_rect(n).unwrap().width(), 33.0);
}
