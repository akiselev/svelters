//! ABI v2.6 `observe` / `prop_changed` on the Qt backend: a property's NOTIFY signal reaches
//! `host.prop_changed(h, prop)` (no value) while observed; calls nest; a coarse NOTIFY
//! (`QAction::changed`) reports every observed property that shares it; a property without NOTIFY is
//! refused with `SVL_OBSERVE_UNSUPPORTED`; destroy and native death drop the observation.
//!
//! Each test runs as its own process (Qt wants one QApplication on the main thread); steps run inside
//! the Qt loop, one loop turn apart. Needs a display: `xvfb-run -a cargo nextest run -p counter-qt`.

use std::ffi::{c_void, CStr};
use std::sync::Mutex;

use counter_qt::hooks;
use svelters_abi::*;

static CHANGED: Mutex<Vec<(svl_handle, svl_id)>> = Mutex::new(Vec::new());
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

fn prop(tag: &str, name: &str) -> svl_id {
    let p = id(SVL_PROP, kind(tag), name);
    assert_ne!(p, 0, "unknown prop {tag}.{name}");
    p
}

fn err() -> String {
    unsafe { CStr::from_ptr((be().last_error)(be().self_)) }.to_string_lossy().into_owned()
}

fn create(tag: &str) -> svl_handle {
    let h = unsafe { (be().create)(be().self_, kind(tag)) };
    assert_ne!(h, 0, "create {tag}: {}", err());
    h
}

fn observe(h: svl_handle, p: svl_id, on: bool) -> Result<(), (i32, String)> {
    let f = be().observe.expect("the Qt backend implements observe");
    match unsafe { f(be().self_, h, p, on) } {
        0 => Ok(()),
        rc => Err((rc, err())),
    }
}

fn set(h: svl_handle, p: svl_id, v: Value) {
    let rc = v.with_raw(|raw| unsafe { (be().set_prop)(be().self_, h, p, raw) });
    assert_eq!(rc, 0, "set_prop: {}", err());
}

fn take_changed() -> Vec<(svl_handle, svl_id)> {
    std::mem::take(&mut *CHANGED.lock().unwrap())
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
unsafe extern "C" fn prop_changed(_: *mut c_void, h: svl_handle, p: svl_id) {
    CHANGED.lock().unwrap().push((h, p));
}

type Step = Box<dyn FnOnce()>;

/// Runs `steps` inside the Qt loop, one per loop turn (50 ms apart), then quits.
fn run_steps(steps: Vec<Step>) {
    let host = Box::leak(Box::new(svl_host {
        self_: std::ptr::null_mut(),
        post_event,
        checkpoint,
        call_sync,
        on_destroyed,
        invoke_callback,
        release_callback,
        frame,
        prop_changed: Some(prop_changed),
    }));
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

#[test]
fn qt_a_notify_property_is_reported_while_observed_and_calls_nest() {
    run_steps(vec![Box::new(|| {
        let s = create("slider");
        let value = prop("slider", "value");
        set(s, prop("slider", "maximum"), Value::I64(100));
        set(s, value, Value::I64(1));
        assert!(take_changed().is_empty(), "not observed yet");

        observe(s, value, true).unwrap();
        set(s, value, Value::I64(5));
        assert_eq!(take_changed(), vec![(s, value)], "valueChanged is valueChanged's NOTIFY");
        // Clamped by the maximum: Qt emits for the value it really took.
        set(s, value, Value::I64(500));
        assert_eq!(take_changed(), vec![(s, value)]);

        observe(s, value, true).unwrap();
        observe(s, value, false).unwrap();
        set(s, value, Value::I64(6));
        assert_eq!(take_changed(), vec![(s, value)], "one off of two keeps the connection");
        observe(s, value, false).unwrap();
        set(s, value, Value::I64(7));
        assert!(take_changed().is_empty(), "the last off disconnects");
        observe(s, value, false).unwrap();

        // Typing into a line edit: QLineEdit::text's NOTIFY (textChanged) per keystroke.
        let e = create("line-edit");
        let text = prop("line-edit", "text");
        observe(e, text, true).unwrap();
        hooks::type_text(e, "ab").unwrap();
        assert_eq!(take_changed(), vec![(e, text), (e, text)]);
    })]);
}

#[test]
fn qt_a_coarse_notify_reports_each_observed_property() {
    run_steps(vec![Box::new(|| {
        let a = create("action");
        let (text, tip) = (prop("action", "text"), prop("action", "tool-tip"));
        observe(a, text, true).unwrap();
        observe(a, tip, true).unwrap();
        set(a, text, Value::Str("Open".into()));
        let mut got = take_changed();
        got.sort_unstable();
        let mut want = vec![(a, text), (a, tip)];
        want.sort_unstable();
        assert_eq!(got, want, "QAction::changed() is the NOTIFY of both");
    })]);
}

fn construct(type_name: &str) -> (svl_handle, svl_id) {
    let k = id(SVL_KIND, 0, type_name);
    assert_ne!(k, 0, "unknown type {type_name}");
    let mut out = svl_value::null();
    let f = be().construct.expect("the Qt backend implements construct");
    let rc = unsafe { f(be().self_, k, std::ptr::null(), std::ptr::null(), 0, &mut out) };
    assert_eq!(rc, 0, "construct {type_name}: {}", err());
    assert_eq!(out.tag, SVL_HANDLE);
    (unsafe { out.u.h }, k)
}

// Milestone R3 acceptance (REACTIVITY.md §10): a QBindable-only property (QTimer::interval has no NOTIFY) and an
// event-announced one (QWidget::enabled -> QEvent::EnabledChange) are observed; off removes the notifier and the
// event filter.
#[test]
fn qt_bindable_and_event_properties_are_observed() {
    run_steps(vec![Box::new(|| {
        let (t, k) = construct("QTimer");
        let interval = id(SVL_PROP, k, "interval");
        assert_ne!(interval, 0);
        observe(t, interval, true).unwrap();
        set(t, interval, Value::I64(250));
        assert_eq!(take_changed(), vec![(t, interval)], "the QPropertyNotifier on QTimer::interval");
        set(t, interval, Value::I64(250));
        assert!(take_changed().is_empty(), "an identical set does not notify a bindable");
        observe(t, interval, false).unwrap();
        set(t, interval, Value::I64(300));
        assert!(take_changed().is_empty(), "off destroys the notifier");

        let s = create("slider");
        let enabled = prop("slider", "enabled");
        observe(s, enabled, true).unwrap();
        set(s, enabled, Value::Bool(false));
        assert_eq!(take_changed(), vec![(s, enabled)], "QEvent::EnabledChange through the event filter");
        observe(s, enabled, false).unwrap();
        set(s, enabled, Value::Bool(true));
        assert!(take_changed().is_empty(), "the filter went with the last event-based observation");
        unsafe { (be().destroy)(be().self_, s) };
    })]);
}

#[test]
fn qt_a_property_without_notify_is_refused_and_teardown_drops_observations() {
    let slider = std::rc::Rc::new(std::cell::Cell::new(0));
    let s2 = slider.clone();
    run_steps(vec![
        Box::new(|| {
            let l = create("label");
            let (rc, msg) = observe(l, prop("label", "text"), true).unwrap_err();
            assert_eq!(rc, SVL_OBSERVE_UNSUPPORTED, "{msg}");
            assert!(msg.contains("no NOTIFY"), "{msg}");
            // A dead or unknown handle is an error; off on it is not.
            assert_eq!(observe(999_999, prop("label", "text"), true).unwrap_err().0, -1);
            observe(999_999, prop("label", "text"), false).unwrap();
        }),
        Box::new(move || {
            let s = create("slider");
            let value = prop("slider", "value");
            observe(s, value, true).unwrap();
            observe(s, value, true).unwrap();
            unsafe { (be().destroy)(be().self_, s) };
            s2.set(s);
        }),
        Box::new(move || {
            // The slider was deleted from the loop; its relays went with the handle.
            let s = slider.get();
            observe(s, prop("slider", "value"), false).unwrap();
            assert!(take_changed().is_empty());
            // Native death: a closed window takes its observed child's relay with it.
            let win = create("main-window");
            let e = create("line-edit");
            assert_eq!(unsafe { (be().insert)(be().self_, win, e, 0, 0) }, 0, "{}", err());
            observe(e, prop("line-edit", "text"), true).unwrap();
            hooks::close(win).unwrap();
        }),
        Box::new(|| {
            assert!(take_changed().is_empty());
        }),
    ]);
}
