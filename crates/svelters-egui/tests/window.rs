//! Windows: the sync `close-request` runs before the pass and can keep the window open; an
//! unvetoed close reports `closed` for the window and `parent` for every descendant; `present`
//! picks the root viewport; titles follow the prop. Headless (kittest).

mod common;

use common::Rig;
use svelters_abi::*;

fn request_close(rig: &mut Rig) {
    rig.harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    rig.run();
}

#[test]
fn close_request_is_sync_and_can_veto() {
    let mut rig = Rig::new();
    let (w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", Value::Str("hi".into()))]);
    rig.listen(w, "close-request");
    rig.run();
    assert_eq!(rig.backend.root_window(), Some(w));

    // the handler answers true: the window stays
    rig.log.borrow_mut().sync_answer = Some(Value::Bool(true));
    request_close(&mut rig);
    let log = rig.take();
    assert_eq!(log.syncs, vec![(w, rig.signal_id(w, "close-request"))]);
    assert!(log.gone.is_empty());
    assert!(rig.br().handle_info(w).is_ok());

    // no veto: closed natively, descendants reported `parent` in pre-order
    rig.log.borrow_mut().sync_answer = Some(Value::Bool(false));
    request_close(&mut rig);
    let log = rig.take();
    assert_eq!(log.syncs.len(), 1);
    assert_eq!(
        log.gone,
        vec![
            (w, SVL_GONE_CLOSED),
            (col, SVL_GONE_PARENT),
            (l, SVL_GONE_PARENT)
        ]
    );
    assert!(rig.br().handle_info(w).is_err() && rig.br().handle_info(l).is_err());
    assert_eq!(rig.backend.root_window(), None);
}

#[test]
fn close_without_listener_and_close_method() {
    let mut rig = Rig::new();
    let (w, _col) = rig.window();
    rig.run();
    request_close(&mut rig);
    let log = rig.take();
    assert!(log.syncs.is_empty(), "no listener: no sync call");
    assert_eq!(log.gone.first(), Some(&(w, SVL_GONE_CLOSED)));

    let (w2, _) = rig.window();
    rig.run();
    rig.call(w2, "close");
    rig.run();
    assert_eq!(rig.take().gone.first(), Some(&(w2, SVL_GONE_CLOSED)));
}

#[test]
fn first_presented_window_is_root_and_tree_ops_validate() {
    let mut rig = Rig::new();
    let w1 = rig.create("window");
    let (w2, col) = rig.window();
    rig.run();
    assert_eq!(
        rig.backend.root_window(),
        Some(w2),
        "an unpresented window is not shown"
    );
    rig.call(w1, "present");
    assert_eq!(
        rig.backend.root_window(),
        Some(w2),
        "the first presented stays root"
    );

    let br = rig.br();
    let a = rig.leaf(col, "button", &[("text", Value::Str("a".into()))]);
    let b = rig.leaf(col, "button", &[("text", Value::Str("b".into()))]);
    assert!(
        br.insert(a, b, None, 0).is_err(),
        "a button takes no children"
    );
    assert!(
        br.insert(col, w1, None, 0).is_err(),
        "a window is never a child"
    );
    assert!(br.insert(col, col, None, 0).is_err());
    let inner = rig.create("box");
    rig.insert(col, inner);
    assert!(br.insert(inner, col, None, 0).is_err(), "no cycles");
    // keyed move: b before a
    br.insert(col, b, Some(a), 0).unwrap();
    rig.backend
        .with_scene(|s| assert_eq!(s.n(col).children, vec![b, a, inner]));
    // stale handles fail, destroy is idempotent, handles are not reused
    br.destroy(a);
    br.destroy(a);
    assert!(
        br.set_prop(a, rig.prop_id(b, "text"), &Value::Str("x".into()))
            .is_err()
    );
    let c = rig.create("button");
    assert_ne!(c, a);
    rig.run();
}
