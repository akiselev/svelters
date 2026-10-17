//! Borrowed handles on the Qt backend (plans/milestones/X1-borrowed-gi.md): getters that return
//! objects we did not create mint BORROWED handles with stable identity; their death is reported
//! with the right reason; they never keep an object alive or enter the tree.
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
    let host = Box::leak(Box::new(svl_host { self_: std::ptr::null_mut(), post_event, checkpoint, call_sync, on_destroyed, invoke_callback, release_callback, frame }));
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

/// main-window > box-layout > [line-edit, scroll-area > box-layout > label]
#[derive(Clone, Copy)]
struct Tree {
    win: svl_handle,
    bx: svl_handle,
    entry: svl_handle,
    scroll: svl_handle,
    inner: svl_handle,
    label: svl_handle,
    /// The scroll area's native viewport, borrowed.
    vp: svl_handle,
}

static TREE: Mutex<Option<Tree>> = Mutex::new(None);

fn t() -> Tree {
    TREE.lock().unwrap().expect("tree built by the first step")
}

fn build() {
    let win = create("main-window");
    let bx = create("box-layout");
    let entry = create("line-edit");
    let scroll = create("scroll-area");
    let inner = create("box-layout");
    let label = create("label");
    insert(inner, label).unwrap();
    insert(scroll, inner).unwrap();
    insert(bx, entry).unwrap();
    insert(bx, scroll).unwrap();
    insert(win, bx).unwrap();
    let vp = getter(scroll, "viewport").unwrap().expect("a scroll area has a viewport");
    *TREE.lock().unwrap() = Some(Tree { win, bx, entry, scroll, inner, label, vp });
}

fn identity() {
    let t = t();
    assert!(![t.win, t.bx, t.entry, t.scroll, t.inner, t.label].contains(&t.vp));
    assert_eq!(info(t.vp).unwrap().lifetime, SVL_LIFE_BORROWED);
    assert_eq!(getter(t.scroll, "viewport").unwrap(), Some(t.vp), "same object, same handle");

    // Objects we created keep their own handles, whatever getter reaches them.
    assert_eq!(getter(t.vp, "parentWidget").unwrap(), Some(t.scroll));
    assert_eq!(getter(t.vp, "parent").unwrap(), Some(t.scroll));
    assert_eq!(getter(t.entry, "parentWidget").unwrap(), Some(t.bx));
    assert_eq!(getter(t.entry, "window").unwrap(), Some(t.win));
    assert_eq!(getter(t.label, "window").unwrap(), Some(t.win));
    assert_eq!(info(t.win).unwrap().lifetime, SVL_LIFE_TOPLEVEL);
    assert_eq!(info(t.entry).unwrap().lifetime, SVL_LIFE_OWNED);
    assert_eq!(getter(t.scroll, "widget").unwrap(), Some(t.inner), "the scroll area's widget is our box");
    assert_eq!(getter(t.win, "centralWidget").unwrap(), Some(t.bx));
    // A getter that does not apply says so; null results are null.
    assert!(getter(t.entry, "centralWidget").unwrap_err().contains("does not apply"));
    assert_eq!(getter(t.win, "parentWidget").unwrap(), None);
}

fn guards() {
    let t = t();
    let other = create("box-layout");
    assert!(insert(t.bx, t.vp).unwrap_err().contains("borrowed"));
    assert!(insert(t.vp, other).unwrap_err().contains("borrowed"));
    assert!(remove(t.scroll, t.vp).unwrap_err().contains("borrowed"));
    assert!(take_gone().is_empty());
}

fn destroy_forgets() {
    let t = t();
    destroy(t.vp);
    destroy(t.vp); // idempotent
    assert!(info(t.vp).is_err(), "stale as soon as destroy returns");
    assert!(getter(t.vp, "parentWidget").is_err());
    let again = getter(t.scroll, "viewport").unwrap().unwrap();
    assert!(again > t.vp, "a new handle, never the old number");
    assert_eq!(getter(again, "parentWidget").unwrap(), Some(t.scroll), "the viewport itself was not deleted");
    assert!(take_gone().is_empty(), "the app's own destroy is silent");
}

fn destroy_owner() {
    let t = t();
    destroy(t.scroll);
    assert!(info(t.scroll).is_err());
    // The delete is deferred: nothing is reported inside this step.
    assert!(take_gone().is_empty());
}

fn owner_died() {
    let t = t();
    // The viewport was deleted with its scroll area, so it is reported (as `owner`) although a
    // borrowed handle existed: the handle did not keep the object alive.
    let gone = take_gone();
    assert!(gone.contains(&(t.vp, SVL_GONE_OWNER)), "{gone:?}");
    assert!(info(t.vp).is_err());
    destroy(t.vp); // late release: harmless
}

fn at_most_once() {
    assert!(take_gone().is_empty(), "each handle is reported once");
}

fn close_window() {
    let t = t();
    hooks::close(t.win).unwrap();
    let gone = take_gone();
    assert_eq!(gone[0], (t.win, SVL_GONE_CLOSED), "{gone:?}");
    assert!(gone[1..].iter().all(|(_, r)| *r == SVL_GONE_PARENT), "{gone:?}");
    let mut hs: Vec<svl_handle> = gone.iter().map(|(h, _)| *h).collect();
    hs.sort_unstable();
    let mut want = vec![t.win, t.bx, t.entry, t.scroll, t.inner, t.label, t.vp];
    want.sort_unstable();
    assert_eq!(hs, want, "window first, then everything below it, the borrowed viewport included");
    assert!(info(t.vp).is_err() && info(t.win).is_err());
}

fn str_value(v: &str) -> svl_value {
    svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: v.as_ptr().cast(), len: v.len() } })
}

fn bool_value(v: bool) -> svl_value {
    svl_value::new(SVL_BOOL, svl_value_u { b: v })
}

/// X7 (G12): a QTextEdit's document is a borrowed handle with its own generated kind, so its
/// properties, methods and signals work on it.
fn document_kind() {
    let edit = create("text-edit");
    let doc = getter(edit, "document").unwrap().expect("a text edit has a document");
    assert_eq!(info(doc).unwrap().lifetime, SVL_LIFE_BORROWED);
    assert_eq!(info(doc).unwrap().kind, kind("text-document"), "borrowed objects get the kind of their class");
    assert_eq!(getter(edit, "document").unwrap(), Some(doc), "same object, same handle");

    let k = kind("text-document");
    let a = str_value("one\ntwo");
    let mut out = svl_value::null();
    assert_eq!(unsafe { (be().call)(be().self_, doc, id(SVL_METHOD, k, "setPlainText"), &a, 1, &mut out) }, 0, "{}", err());
    assert_eq!(unsafe { (be().get_prop)(be().self_, doc, id(SVL_PROP, k, "block-count"), &mut out) }, 0, "{}", err());
    assert_eq!((out.tag, unsafe { out.u.i }), (SVL_I64, 2));
    // the editor shows what the document holds
    let mut text = svl_value::null();
    let ek = kind("text-edit");
    assert_eq!(unsafe { (be().get_prop)(be().self_, edit, id(SVL_PROP, ek, "plain-text"), &mut text) }, 0);
    // Qt returns UTF-16 (SVL_CAP_UTF16_OUT); len counts code units
    let units = unsafe { std::slice::from_raw_parts(text.u.s.ptr.cast::<u16>(), text.u.s.len) };
    assert_eq!(String::from_utf16(units).unwrap(), "one\ntwo");
    let v = bool_value(false);
    assert_eq!(unsafe { (be().set_prop)(be().self_, doc, id(SVL_PROP, k, "undo-redo-enabled"), &v) }, 0, "{}", err());
    destroy(doc);
    destroy(edit);
}

fn scenario(steps: Vec<fn()>) {
    let mut all: Vec<Step> = vec![Box::new(build)];
    all.extend(steps.into_iter().map(|f| Box::new(f) as Step));
    run_steps(all)
}

fn main() {
    counter_qt::run_as_test("identity", || scenario(vec![identity]));
    counter_qt::run_as_test("guards", || scenario(vec![guards]));
    counter_qt::run_as_test("destroy_forgets", || scenario(vec![destroy_forgets]));
    counter_qt::run_as_test("owner_death", || scenario(vec![destroy_owner, owner_died, at_most_once]));
    counter_qt::run_as_test("document_kind", || scenario(vec![document_kind]));
    counter_qt::run_as_test("window_close_cascade", || scenario(vec![close_window, at_most_once]));
}
