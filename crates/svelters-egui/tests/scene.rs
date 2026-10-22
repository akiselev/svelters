//! The scene's public API from Rust (ARCH-SCENE §6, SG1), next to a runtime driving the same
//! scene through host ops (the recording host of `common`, headless kittest).

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_egui::scene::{DumpOpts, NodeId, Owner, ProviderId, SceneError, keys};

fn id(h: svl_handle) -> NodeId {
    NodeId::from_raw(h).unwrap()
}

/// R18: a Rust write of an observed (bound) prop is announced once with `prop_changed`, at the
/// next logic, and never fires the kind's events; an unobserved one is not announced.
#[test]
fn rust_write_of_a_bound_prop_announces_and_fires_no_event() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let cb = rig.leaf(col, "checkbox", &[("text", Value::Str("Snap".into()))]);
    let slider = rig.leaf(col, "slider", &[("accessible-name", Value::Str("Gain".into()))]);
    rig.observe(cb, "checked", true);
    rig.listen(cb, "changed");
    rig.listen(slider, "changed");
    rig.run();
    let checkpoints = rig.log.borrow().checkpoints;
    rig.log.borrow_mut().changed.clear();

    let app = ProviderId::named("cad.test");
    rig.backend
        .scene()
        .write(app, |s| {
            s.node(id(cb)).unwrap().set(&keys::checkbox::CHECKED, true).unwrap();
            s.node(id(slider)).unwrap().set(&keys::slider::VALUE, 42.0).unwrap();
            // the same value again: nothing to announce
            s.node(id(cb)).unwrap().set(&keys::checkbox::CHECKED, true).unwrap();
        })
        .unwrap();
    rig.run();
    assert_eq!(rig.get(cb, "checked"), Value::Bool(true));
    assert_eq!(rig.get(slider, "value"), Value::F64(42.0));
    let l = rig.log.borrow();
    let pid = rig.prop_id(cb, "checked");
    assert_eq!(l.changed, vec![(cb, pid)], "announced once, the unobserved slider not at all");
    assert!(l.events.is_empty(), "no `changed` event for a Rust write: {:?}", l.events);
    assert!(l.checkpoints > checkpoints, "the announcement runs a checkpoint");
}

/// Owners (S3): Rust builds and edits its own subtrees, mounts them under a runtime's node only
/// through a Rust-owned parent, and cannot change a runtime's children.
#[test]
fn rust_edits_its_own_structure_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let js_label = rig.leaf(col, "label", &[("text", Value::Str("js".into()))]);
    let h = rig.backend.scene();
    let app = ProviderId::APP;
    let (panel, row) = h
        .write(app, |s| {
            let panel = s.create_tag("box").unwrap();
            let row = s.create_tag("label").unwrap();
            s.insert(panel, row, None).unwrap();
            assert!(matches!(s.insert(id(col), panel, None), Err(SceneError::NotOwner { owner: Owner::Js, .. })), "no Rust edits of a runtime's children");
            assert!(matches!(s.remove(id(js_label)), Err(SceneError::NotOwner { .. })));
            assert!(matches!(s.destroy(id(js_label)), Err(SceneError::NotOwner { .. })));
            (panel, row)
        })
        .unwrap();
    // a runtime may mount Rust's panel (a host op), then Rust keeps editing inside it
    rig.br().insert(col, panel.to_raw(), None, 0).unwrap();
    h.write(app, |s| {
        let extra = s.create_tag("button").unwrap();
        s.node(extra).unwrap().set(&keys::button::TEXT, "Rust".to_string()).unwrap();
        s.insert(panel, extra, Some(row)).unwrap();
    })
    .unwrap();
    rig.run();
    h.read(|s| {
        s.validate().unwrap();
        let p = s.get(panel).unwrap();
        assert_eq!(p.owner(), Owner::Rust(app));
        assert_eq!(p.parent().map(|n| n.id()), Some(id(col)));
        assert_eq!(p.children().map(|c| c.tag()).collect::<Vec<_>>(), ["button", "label"]);
        assert_eq!(p.root(), s.get(id(col)).unwrap().root());
        assert!(p.children().next().unwrap().drawn().is_some(), "drawn in the window");
        let dump = s.dump(id(col), DumpOpts::default());
        assert!(dump.contains("<button>") && dump.contains("[rust:app]") && dump.contains("\"Rust\""), "{dump}");
    });
    // mounted under a runtime's node, it is the runtime's child: Rust cannot take it out
    h.write(app, |s| assert!(matches!(s.destroy(panel), Err(SceneError::NotOwner { owner: Owner::Js, .. })))).unwrap();
    // once the runtime removed it, Rust destroys its subtree; the runtime's nodes stay
    rig.br().remove(col, panel.to_raw()).unwrap();
    h.write(app, |s| s.destroy(panel).unwrap()).unwrap();
    rig.run();
    h.read(|s| {
        assert!(s.get(panel).is_none() && s.get(row).is_none());
        assert_eq!(s.get(id(col)).unwrap().children().count(), 1);
        s.validate().unwrap();
    });
}

/// `defer` (callable anywhere on the UI thread) runs at the start of the next frame's logic, so
/// the runtime sees the write in that frame's checkpoint.
#[test]
fn deferred_writes_run_at_the_next_logic() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", Value::Str("before".into()))]);
    rig.run();
    let h = rig.backend.scene();
    h.defer(ProviderId::APP, move |s| s.node(id(l)).unwrap().set(&keys::label::TEXT, "after".to_string()).unwrap());
    assert_eq!(h.deferred_len(), 1, "R16: pending work");
    assert_eq!(rig.get(l, "text"), Value::Str("before".into()));
    rig.run();
    assert_eq!(h.deferred_len(), 0);
    assert_eq!(rig.get(l, "text"), Value::Str("after".into()));
}

/// A label change inside a fixed-size row re-lays out that row only, and a resize re-lays out the
/// whole root (counters through `SceneRef::stats`).
#[test]
fn a_label_change_relays_out_its_boundary_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let mut labels = Vec::new();
    for r in 0..20 {
        let row = rig.create("box");
        for (k, v) in [("flex-direction", Value::Str("row".into())), ("width", Value::F64(400.0)), ("height", Value::F64(18.0)), ("flex-shrink", Value::F64(0.0))] {
            rig.set(row, k, v);
        }
        rig.insert(col, row);
        for k in 0..5 {
            labels.push(rig.leaf(row, "label", &[("text", Value::Str(format!("{r}.{k}")))]));
        }
    }
    rig.run();
    let h = rig.backend.scene();
    let before = h.read(|s| s.stats());
    rig.set(labels[37], "text", Value::Str("a longer label than before".into()));
    rig.run();
    let after = h.read(|s| s.stats());
    assert_eq!(after.layouts, before.layouts, "no whole-root layout");
    assert_eq!(after.boundaries_laid_out - before.boundaries_laid_out, 1);
    assert_eq!(after.leaves_measured - before.leaves_measured, 1);
    h.read(|s| {
        s.validate().unwrap();
        let n = s.get(id(labels[37])).unwrap();
        assert!(n.parent().unwrap().is_layout_boundary());
        assert!(n.rect().width() > 100.0, "laid out at its new width: {:?}", n.rect());
        assert_eq!(n.get(&keys::label::TEXT).unwrap(), "a longer label than before");
    });
    // bounds (a synchronous layout) agrees with the frame's
    let b = rig.br().bounds(labels[37]).unwrap().unwrap();
    let r = h.read(|s| s.get(id(labels[37])).unwrap().rect());
    assert_eq!([b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32], [r.min.x, r.min.y, r.width(), r.height()]);
}
