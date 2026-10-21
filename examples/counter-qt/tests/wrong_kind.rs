//! FU3 (FU1 open item on Qt): a table member used on a handle whose kind does not have it fails with an
//! error naming the member and the handle's runtime class, as GTK's does since FU1 ("property X does not
//! belong to <GType>"). Before, Qt said only "unknown property id for this kind".
//!
//! Two receivers: an element of another kind (`label`) and a borrowed object of a class with no element kind
//! (the scroll area's viewport, a plain `QWidget`). Each gets a push-button prop through `set_prop` and
//! `get_prop`, a progress-bar method through `call` and a push-button signal through `listen`.
//! One Qt application per process: its own test binary. Needs a display: `tools/dev/headless`.

use std::ffi::CStr;

use svelters_abi::*;

fn be() -> &'static svl_backend {
    counter_qt::backend()
}

fn id(what: u32, kind: svl_id, name: &str) -> svl_id {
    let i = unsafe { (be().resolve)(be().self_, what, kind, name.as_ptr().cast(), name.len()) };
    assert_ne!(i, 0, "unknown {what}/{kind}/{name}");
    i
}

fn err() -> String {
    unsafe { CStr::from_ptr((be().last_error)(be().self_)) }.to_string_lossy().into_owned()
}

fn create(tag: &str) -> svl_handle {
    let h = unsafe { (be().create)(be().self_, id(SVL_KIND, 0, tag)) };
    assert_ne!(h, 0, "create {tag}: {}", err());
    h
}

/// Every wrong-kind path on `h`; each must fail with a message naming the member and `class`.
fn check(h: svl_handle, class: &str) -> Vec<String> {
    let button = id(SVL_KIND, 0, "push-button");
    let bar = id(SVL_KIND, 0, "progress-bar");
    let flat = id(SVL_PROP, button, "flat");
    let reset = id(SVL_METHOD, bar, "reset");
    let clicked = id(SVL_SIGNAL, button, "clicked");
    let mut out = Vec::new();
    let mut expect = |what: &str, rc: i32, member: &str| {
        let e = err();
        let want = format!("{member} does not belong to {class} (kind ");
        if rc == 0 || !e.contains(&want) {
            out.push(format!("{what} on {class}: rc {rc}, error {e:?}, wanted it to contain {want:?}"));
        }
    };
    let v = svl_value::new(SVL_BOOL, svl_value_u { b: true });
    expect("set_prop", unsafe { (be().set_prop)(be().self_, h, flat, &v) }, "set_prop: property push-button:flat");
    let mut o = svl_value::null();
    expect("get_prop", unsafe { (be().get_prop)(be().self_, h, flat, &mut o) }, "get_prop: property push-button:flat");
    let mut o = svl_value::null();
    expect("call", unsafe { (be().call)(be().self_, h, reset, std::ptr::null(), 0, &mut o) }, "call: method progress-bar:reset");
    expect("listen", unsafe { (be().listen)(be().self_, h, clicked, true) }, "listen: signal push-button:clicked");
    out
}

#[test]
fn qt_wrong_kind_errors_name_the_runtime_class() {
    let label = create("label");
    let mut failures = check(label, "QLabel");

    let area = create("scroll-area");
    let k = id(SVL_KIND, 0, "scroll-area");
    let viewport = id(SVL_METHOD, k, "viewport");
    let mut out = svl_value::null();
    assert_eq!(unsafe { (be().call)(be().self_, area, viewport, std::ptr::null(), 0, &mut out) }, 0, "viewport: {}", err());
    assert_eq!(out.tag, SVL_HANDLE, "viewport returns a handle");
    let vp = unsafe { out.u.h };
    failures.extend(check(vp, "QWidget"));

    unsafe { (be().destroy)(be().self_, area) };
    unsafe { (be().destroy)(be().self_, label) };
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
