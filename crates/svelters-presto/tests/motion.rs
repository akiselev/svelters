//! Native animations (`OP_ANIMATE`) on the compositor and precompiled sheets (`SVL_STYLE_SHEET_BIN`).

mod common;

use common::Rig;
use presto_base::anim::AnimationEvent;
use presto_base::geom::{Scale, Size};
use presto_base::ids::WindowId;
use presto_base::time::Time;
use svelters_abi::*;

/// A box in a window attached as presto window 1.
fn in_window(r: &Rig) -> svl_handle {
    let w = r.create("window");
    let b = r.create("box");
    r.insert(w, b);
    r.call(w, "present");
    assert_eq!(r.backend.take_presents(), [w]);
    r.backend.attach_presented(w, WindowId(1), Size::new(300.0, 200.0), Scale(1.0)).unwrap();
    b
}

const FADE: &str = r#"{"id":7,"duration":300,"delay":0,"frames":[{"opacity":0},{"opacity":0.5},{"opacity":1}]}"#;

#[test]
fn an_animation_runs_on_the_compositor_and_ends_with_animationend() {
    let r = Rig::new();
    let b = in_window(&r);
    assert_eq!(r.br().style(SVL_STYLE_ANIMATE, b, Some(FADE)).unwrap(), 1, "presto plays it");
    let running = r.backend.running_animations();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].0, 7);
    assert_eq!(running[0].1.len(), 1, "one opacity track");
    r.take();
    r.backend.animation_event(AnimationEvent::Finished { id: running[0].1[0], at: Time(0) });
    r.backend.pump();
    let end = r.signal_id(b, "animationend");
    assert_eq!(r.take().events, [(b, end, vec![Value::I64(7)])]);
    assert!(r.backend.running_animations().is_empty());
}

#[test]
fn a_descriptor_plan_the_compositor_can_play_is_used() {
    let r = Rig::new();
    let b = in_window(&r);
    let op = r#"{"id":3,"duration":400,"delay":0,"frames":[{"opacity":0},{"opacity":1}],
        "native":{"kind":"fade","params":{},"computed":{"opacity":"1"},"t1":0,"t2":1}}"#;
    assert_eq!(r.br().style(SVL_STYLE_ANIMATE, b, Some(op)).unwrap(), 1);
    assert_eq!(r.backend.running_animations()[0].1.len(), 1);
}

#[test]
fn cancel_stops_it_and_a_node_in_no_window_ends_at_once() {
    let r = Rig::new();
    let b = in_window(&r);
    r.br().style(SVL_STYLE_ANIMATE, b, Some(FADE)).unwrap();
    r.br().style(SVL_STYLE_ANIMATE, b, Some(r#"{"id":7,"cancel":true}"#)).unwrap();
    assert!(r.backend.running_animations().is_empty());
    let loose = r.create("box");
    r.take();
    assert_eq!(r.br().style(SVL_STYLE_ANIMATE, loose, Some(FADE)).unwrap(), 1);
    r.backend.pump();
    let end = r.signal_id(loose, "animationend");
    assert_eq!(r.take().events, [(loose, end, vec![Value::I64(7)])]);
}

#[test]
fn precompiled_sheets_load_and_replace_by_id() {
    use svelters_style::binary::{Precompiled, encode, to_base64};
    let r = Rig::new();
    let sheet = svelters_style::Stylesheet::parse(".column { gap: 8px }", &Default::default());
    let b64 = to_base64(&encode(&Precompiled { sheet, ..Precompiled::default() }));
    let text = format!("app.svelte{SVL_STYLE_SEP}{b64}");
    assert_eq!(r.br().style(SVL_STYLE_SHEET_BIN, 0, Some(&text)).unwrap(), 0);
    assert!(r.backend.sheets().contains_key("app.svelte"));
    r.br().style(SVL_STYLE_SHEET_BIN, 0, Some(&text)).unwrap();
    assert_eq!(r.backend.sheets().len(), 1, "the same id replaces");
    assert!(r.br().style(SVL_STYLE_SHEET_BIN, 0, Some(&format!("x{SVL_STYLE_SEP}not-base64!"))).is_err());
}
