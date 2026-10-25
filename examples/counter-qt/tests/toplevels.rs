//! X7 (G9) on the Qt backend: a TOPLEVEL child inserted under a parent is transient for the
//! parent's window (plans/architecture/backend-abi.md, "Toplevels in the tree"), opens with the
//! non-blocking `open()`, goes with its parent, and reports its own close.
//!
//! Each scenario runs as its own test process (Qt wants the main thread and one QApplication).
//! The steps run inside the Qt loop through `run_later`, one loop turn apart, so deferred
//! deletes really happen between them. Needs a display: `xvfb-run -a cargo nextest run -p counter-qt`.

use std::ffi::{c_void, CStr};
use std::sync::Mutex;

use counter_qt::hooks;
use svelters_abi::*;

static GONE: Mutex<Vec<(svl_handle, u32)>> = Mutex::new(Vec::new());
static FAILURE: Mutex<Option<String>> = Mutex::new(None);

fn be() -> &'static svl_backend {
    counter_qt::backend()
}

fn id(what: u32, kind: svl_id, name: &str) -> svl_id {
    unsafe { (be().resolve)(be().self_, what, kind, name.as_ptr().cast(), name.len()) }
}

fn kind(name: &str) -> svl_id {
    let k = id(SVL_KIND, 0, name);
    assert_ne!(k, 0, "unknown kind {name}");
    k
}

fn err() -> String {
    unsafe { CStr::from_ptr((be().last_error)(be().self_)) }.to_string_lossy().into_owned()
}

fn create(tag: &str) -> svl_handle {
    let h = unsafe { (be().create)(be().self_, kind(tag)) };
    assert_ne!(h, 0, "create {tag}: {}", err());
    h
}

fn insert(parent: svl_handle, child: svl_handle) -> Result<(), String> {
    match unsafe { (be().insert)(be().self_, parent, child, 0, 0) } {
        0 => Ok(()),
        _ => Err(err()),
    }
}

fn remove(parent: svl_handle, child: svl_handle) -> Result<(), String> {
    match unsafe { (be().remove)(be().self_, parent, child) } {
        0 => Ok(()),
        _ => Err(err()),
    }
}

fn destroy(h: svl_handle) {
    unsafe { (be().destroy)(be().self_, h) };
}

fn info(h: svl_handle) -> Result<svl_handle_info, String> {
    let mut out = svl_handle_info::default();
    match unsafe { (be().handle_info)(be().self_, h, &mut out) } {
        0 => Ok(out),
        _ => Err(err()),
    }
}

/// `call` a no-argument object getter by name; `Ok(None)` for a null result.
fn getter(h: svl_handle, name: &str) -> Result<Option<svl_handle>, String> {
    let k = info(h)?.kind;
    let m = id(SVL_METHOD, k, name);
    if m == 0 {
        return Err(format!("unknown method {name}"));
    }
    let mut out = svl_value::null();
    if unsafe { (be().call)(be().self_, h, m, std::ptr::null(), 0, &mut out) } != 0 {
        return Err(err());
    }
    Ok(match out.tag {
        SVL_HANDLE => Some(unsafe { out.u.h }),
        SVL_NULL => None,
        t => panic!("getter {name} returned tag {t}"),
    })
}

fn take_gone() -> Vec<(svl_handle, u32)> {
    std::mem::take(&mut *GONE.lock().unwrap())
}

unsafe extern "C" fn post_event(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize) {}
unsafe extern "C" fn on_destroyed(_: *mut c_void, h: svl_handle, reason: u32) {
    GONE.lock().unwrap().push((h, reason));
}
unsafe extern "C" fn call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn checkpoint(_: *mut c_void) {}
unsafe extern "C" fn invoke_callback(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn frame(_: *mut c_void) {}

type Step = Box<dyn FnOnce()>;

/// Runs `steps` inside the Qt loop, one per loop turn (50 ms apart), then quits. A panic in a
/// step fails the process.
fn run_steps(steps: Vec<Step>) {
    let host = Box::leak(Box::new(svl_host { self_: std::ptr::null_mut(), post_event, checkpoint, call_sync, on_destroyed, invoke_callback, release_callback, frame, prop_changed: None, dispatch_now: None }));
    unsafe { counter_qt::set_host(host) };

    fn next(mut steps: Vec<Step>) {
        if steps.is_empty() {
            unsafe { (be().quit)(be().self_) };
            return;
        }
        let step = steps.remove(0);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(step)) {
            Ok(()) => hooks::run_later(50, Box::new(move || next(steps))),
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                *FAILURE.lock().unwrap() = Some(msg.unwrap_or_else(|| "panic".into()));
                unsafe { (be().quit)(be().self_) };
            }
        }
    }
    hooks::run_later(0, Box::new(move || next(steps)));
    let rc = unsafe { (be().run)(be().self_, host) };
    assert_eq!(rc, 0);
    if let Some(f) = FAILURE.lock().unwrap().take() {
        eprintln!("FAILED: {f}");
        std::process::exit(1);
    }
}

fn call0(h: svl_handle, name: &str) {
    let k = info(h).unwrap().kind;
    let m = id(SVL_METHOD, k, name);
    assert_ne!(m, 0, "unknown method {name}");
    let mut out = svl_value::null();
    assert_eq!(unsafe { (be().call)(be().self_, h, m, std::ptr::null(), 0, &mut out) }, 0, "{name}: {}", err());
}

/// `(modal, transient, visible)` of the toplevel `h` from the native dump.
fn flags(h: svl_handle) -> (bool, bool, bool) {
    let d = hooks::dump(h);
    let field = |name: &str| d.contains(&format!("\"{name}\":true"));
    (field("modal"), field("transient"), field("visible"))
}

#[derive(Clone, Copy)]
struct Tree {
    win: svl_handle,
    bx: svl_handle,
    dlg: svl_handle,
}

static TREE: Mutex<Option<Tree>> = Mutex::new(None);

fn t() -> Tree {
    TREE.lock().unwrap().expect("tree built by the first step")
}

fn build() {
    let win = create("main-window");
    let bx = create("box-layout");
    let dlg = create("message-box");
    insert(win, bx).unwrap();
    *TREE.lock().unwrap() = Some(Tree { win, bx, dlg });
}

fn transient_open_remove() {
    let t = t();
    // under a plain widget: the window that contains it
    insert(t.bx, t.dlg).unwrap();
    assert_eq!(getter(t.dlg, "parentWidget").unwrap(), Some(t.win));
    assert!(insert(t.win, t.dlg).unwrap_err().contains("already inserted"));
    assert!(insert(t.win, t.win).is_err());
    call0(t.win, "present");
    call0(t.dlg, "open");
    assert_eq!(flags(t.dlg), (true, true, true), "open() is window-modal and visible, transient for the window");
    remove(t.bx, t.dlg).unwrap();
    assert_eq!(getter(t.dlg, "parentWidget").unwrap(), None);
    assert!(remove(t.bx, t.dlg).is_err(), "not inserted any more");
}

fn modeless_show() {
    let t = t();
    // QMessageBox is modal by itself; a plain QDialog shows modeless under `present`.
    let plain = create("dialog");
    insert(t.win, plain).unwrap();
    call0(plain, "present");
    assert_eq!(flags(plain), (false, true, true), "present is show(): modeless");
}

fn close_parent() {
    let t = t();
    insert(t.win, t.dlg).unwrap();
    call0(t.dlg, "open");
    hooks::close(t.win).unwrap();
    let gone = take_gone();
    assert_eq!(gone[0], (t.win, SVL_GONE_CLOSED), "{gone:?}");
    let mut rest: Vec<(svl_handle, u32)> = gone[1..].to_vec();
    rest.sort_unstable();
    assert_eq!(rest, vec![(t.bx, SVL_GONE_PARENT), (t.dlg, SVL_GONE_PARENT)], "{gone:?}");
}

fn close_dialog() {
    let t = t();
    insert(t.win, t.dlg).unwrap();
    call0(t.win, "present");
    call0(t.dlg, "open");
    hooks::close(t.dlg).unwrap();
    assert_eq!(take_gone(), vec![(t.dlg, SVL_GONE_CLOSED)]);
    assert!(info(t.win).is_ok() && info(t.bx).is_ok(), "the parent lives on");
}

fn attr_str(parent: svl_handle, child: svl_handle, name: &str) -> String {
    let a = id(SVL_ATTR, info(parent).unwrap().kind, name);
    assert_ne!(a, 0, "unknown attribute {name}");
    let mut out = svl_value::null();
    assert_eq!(unsafe { (be().get_child_attr.unwrap())(be().self_, parent, child, a, &mut out) }, 0, "{}", err());
    assert_eq!(out.tag, SVL_STR);
    let units = unsafe { std::slice::from_raw_parts(out.u.s.ptr.cast::<u16>(), out.u.s.len) };
    String::from_utf16(units).unwrap()
}

fn set_attr_str(parent: svl_handle, child: svl_handle, name: &str, v: &str) {
    let a = id(SVL_ATTR, info(parent).unwrap().kind, name);
    let val = svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: v.as_ptr().cast(), len: v.len() } });
    assert_eq!(unsafe { (be().set_child_attr.unwrap())(be().self_, parent, child, a, &val) }, 0, "{}", err());
}

/// X7: toolbars and dock widgets inserted into a window without naming a slot are routed by kind;
/// their area is a child attribute.
fn areas_route_by_kind() {
    let t = t();
    let central = create("label");
    let tb = create("tool-bar");
    let dock = create("dock-widget");
    insert(t.win, central).unwrap();
    insert(t.win, tb).unwrap();
    insert(t.win, dock).unwrap();
    assert_eq!(getter(t.win, "centralWidget").unwrap(), Some(central), "the label is still the central widget");
    assert_eq!(attr_str(t.win, tb, "area"), "top");
    assert_eq!(attr_str(t.win, dock, "area"), "left");
    set_attr_str(t.win, tb, "area", "bottom");
    set_attr_str(t.win, dock, "area", "right");
    assert_eq!(attr_str(t.win, tb, "area"), "bottom");
    assert_eq!(attr_str(t.win, dock, "area"), "right");
    remove(t.win, tb).unwrap();
    remove(t.win, dock).unwrap();
    assert_eq!(getter(tb, "parentWidget").unwrap(), None, "removed means unparented");
}

fn scenario(steps: Vec<fn()>) {
    let mut all: Vec<Step> = vec![Box::new(build)];
    all.extend(steps.into_iter().map(|f| Box::new(f) as Step));
    run_steps(all)
}

fn main() {
    counter_qt::run_as_test("transient_open_remove", || scenario(vec![transient_open_remove]));
    counter_qt::run_as_test("modeless_show", || scenario(vec![modeless_show]));
    counter_qt::run_as_test("close_parent", || scenario(vec![close_parent]));
    counter_qt::run_as_test("areas_route_by_kind", || scenario(vec![areas_route_by_kind]));
    counter_qt::run_as_test("close_dialog", || scenario(vec![close_dialog]));
}
