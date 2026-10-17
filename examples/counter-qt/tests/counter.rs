//! Demo test: builds the counter tree from a hand-written op sequence (the same ops as
//! fixtures/counter/expected.oplog), shows the window, clicks the button through
//! `QPushButton::click()` (the `click` method), and checks that the `clicked` events reached a
//! stub `svl_host`, whose "JS" handler updates the label. Writes target/p4-counter.png.
//!
//! The ops run inside `run()`'s event loop from the `checkpoint` callback that `wake()` queued, so
//! the test also covers run/wake/quit. Needs a display: `xvfb-run -a cargo test -p counter-qt`.

use std::ffi::c_void;
use std::sync::Mutex;

use svelters_abi::*;

unsafe extern "C" {
    fn svl_qt_grab_png(h: svl_handle, path: *const std::ffi::c_char) -> i32;
}

struct State {
    events: Vec<(svl_handle, svl_id)>,
    checkpoints: u32,
    window: svl_handle,
    button: svl_handle,
    label: svl_handle,
    clicked: svl_id,
    label_prop: svl_id,
    click_method: svl_id,
    failure: Option<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn be() -> &'static svl_backend {
    counter_qt::backend()
}

fn id(what: u32, kind: svl_id, name: &str) -> svl_id {
    unsafe { (be().resolve)(be().self_, what, kind, name.as_ptr().cast(), name.len()) }
}

fn s(v: &str) -> svl_value {
    svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: v.as_ptr().cast(), len: v.len() } })
}

fn i(v: i64) -> svl_value {
    svl_value::new(SVL_I64, svl_value_u { i: v })
}

fn ok(rc: i32, what: &str) {
    if rc != 0 {
        let msg = unsafe { std::ffi::CStr::from_ptr((be().last_error)(be().self_)) };
        panic!("{what} failed: {}", msg.to_string_lossy());
    }
}

fn set(h: svl_handle, kind: svl_id, prop: &str, v: svl_value) {
    ok(unsafe { (be().set_prop)(be().self_, h, id(SVL_PROP, kind, prop), &v) }, prop);
}

fn insert(parent: svl_handle, child: svl_handle) {
    ok(unsafe { (be().insert)(be().self_, parent, child, 0, 0) }, "insert");
}

fn get_str(h: svl_handle, prop: svl_id) -> String {
    let mut out = svl_value::null();
    ok(unsafe { (be().get_prop)(be().self_, h, prop, &mut out) }, "get_prop");
    assert_eq!(out.tag, SVL_STR);
    if out.flags & SVL_F_UTF16 != 0 {
        let u = unsafe { std::slice::from_raw_parts(out.u.s.ptr.cast::<u16>(), out.u.s.len) };
        return String::from_utf16(u).unwrap();
    }
    let s = unsafe { std::slice::from_raw_parts(out.u.s.ptr.cast::<u8>(), out.u.s.len) };
    String::from_utf8(s.to_vec()).unwrap()
}

unsafe extern "C" fn post_event(_: *mut c_void, h: svl_handle, sig: svl_id, _: *const svl_value, _: usize) {
    STATE.lock().unwrap().as_mut().unwrap().events.push((h, sig));
}

unsafe extern "C" fn invoke_callback(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn on_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}

unsafe extern "C" fn frame(_: *mut c_void) {}

unsafe extern "C" fn call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}

unsafe extern "C" fn checkpoint(_: *mut c_void) {
    let result = std::panic::catch_unwind(drive);
    if let Err(e) = result {
        let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
        STATE.lock().unwrap().as_mut().unwrap().failure = Some(msg.unwrap_or_else(|| "panic".into()));
    }
    unsafe { (be().quit)(be().self_) };
}

/// The "JS" side: click three times, handling each delivered event by updating the label.
fn drive() {
    let (button, label, window, clicked, label_prop, click) = {
        let mut g = STATE.lock().unwrap();
        let st = g.as_mut().unwrap();
        st.checkpoints += 1;
        (st.button, st.label, st.window, st.clicked, st.label_prop, st.click_method)
    };
    let mut count = 0;
    for _ in 0..3 {
        ok(unsafe { (be().call)(be().self_, button, click, std::ptr::null(), 0, std::ptr::null_mut()) }, "click");
        let events: Vec<_> = std::mem::take(&mut STATE.lock().unwrap().as_mut().unwrap().events);
        assert_eq!(events, vec![(button, clicked)], "each click is delivered exactly once");
        count += 1;
        let text = format!("Count: {count}");
        let v = s(&text);
        ok(unsafe { (be().set_prop)(be().self_, label, label_prop, &v) }, "set label");
    }
    assert_eq!(get_str(label, label_prop), "Count: 3");

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&dir).unwrap();
    let png = std::ffi::CString::new(dir.join("p4-counter.png").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { svl_qt_grab_png(window, png.as_ptr()) }, 0, "screenshot");
}

fn main() {
    counter_qt::run_as_test("counter_demo", demo);
}

fn demo() {
    let b = be();
    let host = svl_host { self_: std::ptr::null_mut(), post_event, checkpoint, call_sync, on_destroyed, invoke_callback, release_callback, frame };

    let kind = |name: &str| id(SVL_KIND, 0, name);
    let (k_window, k_box, k_label, k_button) = (kind("window"), kind("box"), kind("label"), kind("button"));
    assert!(k_window != 0 && k_box != 0 && k_label != 0 && k_button != 0);

    // create window #1, set title; box #2 vertical, spacing 8; label #3; button #4
    let window = unsafe { (b.create)(b.self_, k_window) };
    set(window, k_window, "title", s("Counter"));
    let bx = unsafe { (b.create)(b.self_, k_box) };
    set(bx, k_box, "orientation", s("vertical"));
    set(bx, k_box, "spacing", i(8));
    let label = unsafe { (b.create)(b.self_, k_label) };
    set(label, k_label, "label", s("Count: 0"));
    insert(bx, label);
    let button = unsafe { (b.create)(b.self_, k_button) };
    let clicked = id(SVL_SIGNAL, k_button, "clicked");
    ok(unsafe { (b.listen)(b.self_, button, clicked, true) }, "listen");
    set(button, k_button, "label", s("Increment"));
    insert(bx, button);
    insert(window, bx);
    let present = id(SVL_METHOD, k_window, "present");
    ok(unsafe { (b.call)(b.self_, window, present, std::ptr::null(), 0, std::ptr::null_mut()) }, "present");

    *STATE.lock().unwrap() = Some(State {
        events: vec![],
        checkpoints: 0,
        window,
        button,
        label,
        clicked,
        label_prop: id(SVL_PROP, k_label, "label"),
        click_method: id(SVL_METHOD, k_button, "click"),
        failure: None,
    });

    unsafe { counter_qt::set_host(&host) };
    unsafe { (b.wake)(b.self_) }; // queued checkpoint, runs inside the loop
    let rc = unsafe { (b.run)(b.self_, &host) };
    assert_eq!(rc, 0, "run returned {rc}");

    let g = STATE.lock().unwrap();
    let st = g.as_ref().unwrap();
    if let Some(f) = &st.failure {
        eprintln!("counter demo failed: {f}");
        std::process::exit(1);
    }
    assert_eq!(st.checkpoints, 1);
    println!("counter demo ok: 3 clicks delivered, label updated, screenshot at target/p4-counter.png");
}
