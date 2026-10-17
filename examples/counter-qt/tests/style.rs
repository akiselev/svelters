//! X6 styling on Qt at the backend level (ABI v2.3 `style` op). Results are read back natively:
//! the palette after QSS, rendered pixels from `grab()`, the dynamic `class` property and the
//! application stylesheet. Each scenario is its own test process (Qt wants the main thread and
//! one QApplication); its steps run inside the loop one turn apart so deferred work (the
//! coalesced application stylesheet) has happened by the next step. Needs a display (xvfb-run).

use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::sync::Mutex;

use counter_qt::hooks;
use svelters_abi::*;

static FAILURE: Mutex<Option<String>> = Mutex::new(None);
static HANDLES: Mutex<Option<HashMap<&'static str, svl_handle>>> = Mutex::new(None);

fn be() -> &'static svl_backend {
    counter_qt::backend()
}

fn id(what: u32, kind: svl_id, name: &str) -> svl_id {
    unsafe { (be().resolve)(be().self_, what, kind, name.as_ptr().cast(), name.len()) }
}

fn err() -> String {
    unsafe { CStr::from_ptr((be().last_error)(be().self_)) }.to_string_lossy().into_owned()
}

fn create(tag: &str) -> svl_handle {
    let k = id(SVL_KIND, 0, tag);
    assert_ne!(k, 0, "unknown kind {tag}");
    let h = unsafe { (be().create)(be().self_, k) };
    assert_ne!(h, 0, "create {tag}: {}", err());
    h
}

fn insert(parent: svl_handle, child: svl_handle) {
    assert_eq!(unsafe { (be().insert)(be().self_, parent, child, 0, 0) }, 0, "insert: {}", err());
}

fn s(v: &str) -> svl_value {
    svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: v.as_ptr().cast(), len: v.len() } })
}

fn set_str(h: svl_handle, tag: &str, prop: &str, v: &str) {
    let k = id(SVL_KIND, 0, tag);
    let p = id(SVL_PROP, k, prop);
    assert_ne!(p, 0, "unknown prop {prop}");
    let val = s(v);
    assert_eq!(unsafe { (be().set_prop)(be().self_, h, p, &val) }, 0, "{prop}: {}", err());
}

fn set_bool(h: svl_handle, tag: &str, prop: &str, v: bool) {
    let k = id(SVL_KIND, 0, tag);
    let p = id(SVL_PROP, k, prop);
    assert_ne!(p, 0, "unknown prop {prop}");
    let val = svl_value::new(SVL_BOOL, svl_value_u { b: v });
    assert_eq!(unsafe { (be().set_prop)(be().self_, h, p, &val) }, 0, "{prop}: {}", err());
}

fn style(op: u32, h: svl_handle, text: Option<&str>) -> i32 {
    let st = be().style.expect("the Qt backend implements style");
    let v = text.map(s).unwrap_or_else(svl_value::null);
    let rc = unsafe { st(be().self_, op, h, &v) };
    assert!(rc >= 0, "style op {op}: {}", err());
    rc
}

fn sheet(id: &str, qss: &str) {
    style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{qss}")));
}

fn hd(name: &str) -> svl_handle {
    HANDLES.lock().unwrap().as_ref().unwrap()[name]
}

fn present(win: svl_handle) {
    let k = id(SVL_KIND, 0, "window");
    let m = id(SVL_METHOD, k, "present");
    assert_eq!(unsafe { (be().call)(be().self_, win, m, std::ptr::null(), 0, std::ptr::null_mut()) }, 0, "{}", err());
}

unsafe extern "C" fn post_event(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize) {}
unsafe extern "C" fn on_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}
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
            Ok(()) => hooks::run_later(60, Box::new(move || next(steps))),
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                *FAILURE.lock().unwrap() = Some(msg.unwrap_or_else(|| "panic".into()));
                unsafe { (be().quit)(be().self_) };
            }
        }
    }
    hooks::run_later(0, Box::new(move || next(steps)));
    assert_eq!(unsafe { (be().run)(be().self_, host) }, 0);
    if let Some(f) = FAILURE.lock().unwrap().take() {
        eprintln!("FAILED: {f}");
        std::process::exit(1);
    }
}

/// window > box > [label, button, checkbox]; shown. Starts from an empty stylesheet.
fn build() {
    style(SVL_STYLE_SHEET, 0, None);
    let win = create("window");
    let bx = create("box");
    let label = create("label");
    let button = create("button");
    let check = create("checkbox");
    set_str(label, "label", "label", "some text");
    set_str(button, "button", "label", "go");
    set_str(check, "checkbox", "label", "check");
    insert(bx, label);
    insert(bx, button);
    insert(bx, check);
    insert(win, bx);
    *HANDLES.lock().unwrap() = Some(HashMap::from([("win", win), ("bx", bx), ("label", label), ("button", button), ("check", check)]));
    present(win);
}

fn scenario(steps: Vec<fn()>) {
    let mut all: Vec<Step> = vec![Box::new(build)];
    all.extend(steps.into_iter().map(|f| Box::new(f) as Step));
    run_steps(all)
}

fn hex(c: u32) -> String {
    format!("{c:06x}")
}

fn pixel_hex(h: svl_handle, x: i32, y: i32) -> String {
    hex(hooks::pixel(h, x, y).expect("pixel"))
}

// ---- scenarios ----

fn target() {
    assert_eq!(style(SVL_STYLE_TARGET, 0, None), SVL_STYLE_TARGET_QSS);
}

fn class_step_1() {
    sheet("App", ".QLabel.warn.svelte-x { background-color: #ff0000; }\n.QLabel.ok.svelte-x { background-color: #00ff00; }\n");
    style(SVL_STYLE_CLASSES, hd("label"), Some("warn svelte-x"));
}

fn class_step_2() {
    let l = hd("label");
    assert_eq!(hooks::native_prop(l, "class", false), "QVariant(QString, \"QLabel warn svelte-x\")");
    assert_eq!(pixel_hex(l, 1, 1), "ff0000");
    style(SVL_STYLE_CLASSES, l, Some("ok svelte-x"));
    // the other widgets never had the class
    assert_ne!(pixel_hex(hd("button"), 1, 1), "ff0000");
}

fn class_step_3() {
    let l = hd("label");
    assert_eq!(pixel_hex(l, 1, 1), "00ff00", "toggling the class restyled the widget");
    style(SVL_STYLE_CLASSES, l, None);
}

fn class_step_4() {
    let l = hd("label");
    assert_ne!(pixel_hex(l, 1, 1), "00ff00");
    assert_eq!(hooks::native_prop(l, "class", false), "QVariant(QString, \"QLabel\")", "the Qt class name stays so .QLabel selectors keep matching");
}

fn color_step_1() {
    sheet("App", ".QLabel.c { color: #123456; }\n");
    style(SVL_STYLE_CLASSES, hd("label"), Some("c"));
}

fn color_step_2() {
    assert_eq!(hex(hooks::palette_color(hd("label"), false).unwrap()), "123456", "QSS color reaches the palette");
}

fn id_step_1() {
    sheet("App", "#lbl { background-color: #0000ff; }\n");
    style(SVL_STYLE_NAME, hd("label"), Some("lbl"));
}

fn id_step_2() {
    assert_eq!(pixel_hex(hd("label"), 1, 1), "0000ff");
}

fn state_step_1() {
    sheet("App", ".QCheckBox.s { background-color: #111111; }\n.QCheckBox.s:checked { background-color: #222222; }\n.QCheckBox.s:disabled { background-color: #333333; }\n");
    style(SVL_STYLE_CLASSES, hd("check"), Some("s"));
}

fn state_step_2() {
    assert_eq!(pixel_hex(hd("check"), 1, 1), "111111");
    set_bool(hd("check"), "checkbox", "checked", true);
}

fn state_step_3() {
    assert_eq!(pixel_hex(hd("check"), 1, 1), "222222", ":checked applies without a manual re-polish");
    set_bool(hd("check"), "checkbox", "checked", false);
    set_bool(hd("check"), "checkbox", "enabled", false);
}

fn state_step_4() {
    assert_eq!(pixel_hex(hd("check"), 1, 1), "333333", ":disabled applies");
}

fn inline_step_1() {
    style(SVL_STYLE_INLINE, hd("label"), Some("background-color: #00ff00;"));
    style(SVL_STYLE_INLINE, hd("button"), Some("background-color: #0000ff;\nborder: none;"));
}

fn inline_step_2() {
    assert_eq!(pixel_hex(hd("label"), 1, 1), "00ff00");
    assert_eq!(pixel_hex(hd("button"), 3, 3), "0000ff");
    style(SVL_STYLE_INLINE, hd("label"), None);
}

fn inline_step_3() {
    assert_ne!(pixel_hex(hd("label"), 1, 1), "00ff00");
    assert_eq!(pixel_hex(hd("button"), 3, 3), "0000ff", "removing one widget's style leaves the other");
}

fn inline_beats_sheet_1() {
    sheet("App", ".QLabel.c { background-color: #111111; }\n");
    style(SVL_STYLE_CLASSES, hd("label"), Some("c"));
    style(SVL_STYLE_INLINE, hd("label"), Some("background-color: #abcdef;"));
}

fn inline_beats_sheet_2() {
    assert_eq!(pixel_hex(hd("label"), 1, 1), "abcdef");
}

fn opacity_step() {
    style(SVL_STYLE_OPACITY, hd("label"), Some("0.25"));
    assert!((hooks::opacity(hd("label")) - 0.25).abs() < 1e-9);
    style(SVL_STYLE_OPACITY, hd("label"), None);
    assert_eq!(hooks::opacity(hd("label")), 1.0);
}

fn sheets_step_1() {
    sheet("A", ".QLabel.one { background-color: #010101; }\n");
    sheet("B", ".QLabel.two { background-color: #020202; }\n");
    style(SVL_STYLE_CLASSES, hd("label"), Some("one"));
}

fn sheets_step_2() {
    let ss = hooks::app_stylesheet();
    assert!(ss.find(".one").unwrap() < ss.find(".two").unwrap(), "registration order is kept: {ss}");
    assert_eq!(pixel_hex(hd("label"), 1, 1), "010101");
    // a token change re-sends the rendered text: replace A, remove B
    sheet("A", ".QLabel.one { background-color: #0a0a0a; }\n");
    style(SVL_STYLE_SHEET, 0, Some("B"));
}

fn sheets_step_3() {
    assert_eq!(pixel_hex(hd("label"), 1, 1), "0a0a0a");
    assert!(!hooks::app_stylesheet().contains(".two"));
    style(SVL_STYLE_SHEET, 0, None);
}

fn sheets_step_4() {
    assert_eq!(hooks::app_stylesheet(), "");
    assert_ne!(pixel_hex(hd("label"), 1, 1), "0a0a0a");
}

fn scheme_step_1() {
    let pal = ["window=#242424", "windowText=#ffffff", "base=#303030", "highlight=#3584e4"].join(&SVL_STYLE_SEP.to_string());
    style(SVL_STYLE_SCHEME, 0, Some(&format!("dark{SVL_STYLE_SEP}{pal}")));
}

fn scheme_step_2() {
    assert_eq!(hex(hooks::palette_color(hd("win"), true).unwrap()), "242424");
    assert_eq!(hex(hooks::palette_color(hd("label"), false).unwrap()), "ffffff");
    style(SVL_STYLE_SCHEME, 0, Some(&format!("light{SVL_STYLE_SEP}window=#f6f5f4{SVL_STYLE_SEP}windowText=#2e3436")));
}

fn scheme_step_3() {
    assert_eq!(hex(hooks::palette_color(hd("win"), true).unwrap()), "f6f5f4");
}

fn main() {
    counter_qt::run_as_test("target_is_qss", || scenario(vec![target]));
    counter_qt::run_as_test("class_toggling_restyles_the_widget", || scenario(vec![class_step_1, class_step_2, class_step_3, class_step_4]));
    counter_qt::run_as_test("color_reaches_the_palette", || scenario(vec![color_step_1, color_step_2]));
    counter_qt::run_as_test("id_selects_through_the_object_name", || scenario(vec![id_step_1, id_step_2]));
    counter_qt::run_as_test("checked_and_disabled_states", || scenario(vec![state_step_1, state_step_2, state_step_3, state_step_4]));
    counter_qt::run_as_test("inline_styles_are_per_widget", || scenario(vec![inline_step_1, inline_step_2, inline_step_3]));
    counter_qt::run_as_test("inline_beats_the_application_sheet", || scenario(vec![inline_beats_sheet_1, inline_beats_sheet_2]));
    counter_qt::run_as_test("opacity_uses_a_graphics_effect", || scenario(vec![opacity_step]));
    counter_qt::run_as_test("sheets_concatenate_replace_and_clear", || scenario(vec![sheets_step_1, sheets_step_2, sheets_step_3, sheets_step_4]));
    counter_qt::run_as_test("scheme_sets_the_palette", || scenario(vec![scheme_step_1, scheme_step_2, scheme_step_3]));
}
