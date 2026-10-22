//! NA1 capture on Qt (WG3): every pin of `conformance/pins/` on real `QLineEdit`s and a
//! `QPushButton` in a `QMainWindow`, created through the Qt backend's C ABI, with input injected
//! through XTEST on a private Xvfb (the X server path into Qt's xcb plugin, not `sendEvent`). The
//! selection anchor and keyboard focus come from the `svl_qt_na_state` hook (QLineEdit has no
//! selection-start property); text and caret are cross-checked against the backend's `text` and
//! `cursor-position`, and `text`, `cursor-position`, `selectedText` and `focus` are observed (ABI
//! v2.6) so the capture records whether the backend announced each change the user path made.
//!
//! Writes `target/na/linux/qt-<version>.json` and the per-pin traces; compares with
//! `conformance/golden/linux/qt-<version>.json` when it exists; `NA_UPDATE=1` rewrites it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_void;
use std::sync::Mutex;

use counter_qt::hooks;
use svelters_abi::*;
use svelters_conformance::golden::{Capture, finish};
use svelters_conformance::pins::{BUTTON, BUTTON_LABEL, Chord, FIELDS, Keys, Pin, load_pins};
use svelters_conformance::trace::{Checkpoint, State, Trace};
use svelters_conformance::xtest::XInput;
use svelters_conformance::{
    Driver, observe_selections, prepare_selections, rerun_on_private_display, run_pin,
};

unsafe extern "C" {
    fn svl_qt_meta_prop_id(native: *const std::ffi::c_char) -> svl_id;
}

static CHANGED: Mutex<Vec<(svl_handle, svl_id)>> = Mutex::new(Vec::new());

unsafe extern "C" fn post_event(
    _: *mut c_void,
    _: svl_handle,
    _: svl_id,
    _: *const svl_value,
    _: usize,
) {
}
unsafe extern "C" fn on_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}
unsafe extern "C" fn call_sync(
    _: *mut c_void,
    _: svl_handle,
    _: svl_id,
    _: *const svl_value,
    _: usize,
    _: *mut svl_value,
) -> i32 {
    -1
}
unsafe extern "C" fn checkpoint(_: *mut c_void) {}
unsafe extern "C" fn invoke_callback(
    _: *mut c_void,
    _: u64,
    _: *const svl_value,
    _: usize,
    _: *mut svl_value,
) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn frame(_: *mut c_void) {}
unsafe extern "C" fn prop_changed(_: *mut c_void, h: svl_handle, p: svl_id) {
    CHANGED.lock().unwrap().push((h, p));
}

fn br() -> BackendRef<'static> {
    unsafe { BackendRef::from_raw(counter_qt::backend()) }.expect("abi version")
}

fn p(ms: u64) {
    hooks::pump(ms as i32)
}

fn kind(tag: &str) -> svl_id {
    let k = br().resolve(NameKind::Kind, 0, tag);
    assert_ne!(k, 0, "unknown kind {tag}");
    k
}

fn prop(tag: &str, name: &str) -> svl_id {
    let id = br().resolve(NameKind::Prop, kind(tag), name);
    assert_ne!(id, 0, "unknown prop {tag}.{name}");
    id
}

fn meta(native: &str) -> svl_id {
    let c = std::ffi::CString::new(native).unwrap();
    unsafe { svl_qt_meta_prop_id(c.as_ptr()) }
}

fn set(h: svl_handle, tag: &str, name: &str, v: Value) -> Result<(), String> {
    br().set_prop(h, prop(tag, name), &v)
        .map_err(|e| format!("set {tag}.{name}: {e:?}"))
}

/// The hook's view of one widget: (state, UTF-16 caret).
fn na_state(h: svl_handle) -> Result<(State, Option<i64>, serde_json::Value), String> {
    let v: serde_json::Value =
        serde_json::from_str(&hooks::na_state(h)).map_err(|e| format!("na_state #{h}: {e}"))?;
    let focus = v["focus"].as_bool().unwrap_or(false);
    let s = match v["text"].as_str() {
        Some(t) => State {
            text: Some(t.to_string()),
            sel: Some([
                v["anchor"].as_u64().unwrap() as usize,
                v["caret"].as_u64().unwrap() as usize,
            ]),
            focus,
        },
        None => State {
            text: None,
            sel: None,
            focus,
        },
    };
    Ok((s, v["u16caret"].as_i64(), v))
}

#[derive(Default)]
struct Evidence {
    last: HashMap<(u64, &'static str), String>,
    counts: BTreeMap<String, (u32, u32)>,
    unsupported: BTreeMap<String, String>,
}

struct Qt {
    trace: Trace,
    win: u64,
    fields: Vec<(String, u64)>,
    button: u64,
    x: Option<XInput>,
    ev: Evidence,
    /// (handle, name, prop id) of every observation.
    observed: Vec<(u64, &'static str, svl_id)>,
    /// The pin's selection pseudo-anchors.
    selections: Vec<String>,
}

impl Qt {
    fn handle(&self, anchor: &str) -> Result<u64, String> {
        if anchor == BUTTON {
            return Ok(self.button);
        }
        self.fields
            .iter()
            .find(|(a, _)| a == anchor)
            .map(|(_, h)| *h)
            .ok_or_else(|| format!("no anchor {anchor}"))
    }

    fn observe_on(&mut self, h: u64, name: &'static str, id: svl_id) {
        match br().observe(h, id, true) {
            Ok(()) => self.observed.push((h, name, id)),
            Err(e) => {
                self.ev
                    .unsupported
                    .insert(name.to_string(), format!("{e:?}"));
            }
        }
    }

    fn point(&self, target: &str, at: Option<usize>) -> Result<(f64, f64), String> {
        hooks::na_point(self.handle(target)?, at).ok_or_else(|| format!("no point for {target}"))
    }
}

impl Driver for Qt {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let title = format!("svelters-na {}", pin.id);
        let b = br();
        let t = &mut self.trace;
        let win = b
            .create(kind("main-window"))
            .map_err(|e| format!("{e:?}"))?;
        t.create("main-window", win);
        set(
            win,
            "main-window",
            "window-title",
            Value::Str(title.clone()),
        )?;
        t.set_str(win, "window-title", &title);
        set(win, "main-window", "minimum-width", Value::I64(640))?;
        t.set_raw(win, "minimum-width", "640");
        let bx = b.create(kind("box-layout")).map_err(|e| format!("{e:?}"))?;
        t.create("box-layout", bx);
        set(
            bx,
            "box-layout",
            "direction",
            Value::Str("top-to-bottom".into()),
        )?;
        t.set_str(bx, "direction", "top-to-bottom");
        self.fields.clear();
        for a in FIELDS {
            let e = b.create(kind("line-edit")).map_err(|e| format!("{e:?}"))?;
            t.create("line-edit", e);
            let f = pin.setup.field(a);
            set(e, "line-edit", "text", Value::Str(f.text.clone()))?;
            t.set_str(e, "text", &f.text);
            b.insert(bx, e, None, 0).map_err(|e| format!("{e:?}"))?;
            t.insert(bx, e);
            self.fields.push((a.to_string(), e));
        }
        let btn = b
            .create(kind("push-button"))
            .map_err(|e| format!("{e:?}"))?;
        t.create("push-button", btn);
        set(btn, "push-button", "text", Value::Str(BUTTON_LABEL.into()))?;
        t.set_str(btn, "text", BUTTON_LABEL);
        b.insert(bx, btn, None, 0).map_err(|e| format!("{e:?}"))?;
        t.insert(bx, btn);
        b.insert(win, bx, None, 0).map_err(|e| format!("{e:?}"))?;
        t.insert(win, bx);
        b.call(
            win,
            b.resolve(NameKind::Method, kind("main-window"), "present"),
            &[],
        )
        .map_err(|e| format!("present: {e:?}"))?;
        t.call(win, "present", "[]");
        self.win = win;
        self.button = btn;
        p(200);
        let mut x = XInput::attach(&title, &mut p)?;
        prepare_selections(&mut x, pin, &mut p)?;
        self.x = Some(x);
        self.selections = pin.observe.clone();
        if let Some(f) = &pin.setup.focus {
            hooks::na_setup(self.handle(f)?, true, None);
        }
        for (a, e) in self.fields.clone() {
            hooks::na_setup(e, false, Some(pin.setup.field(&a).sel()));
        }
        p(100);
        self.observed.clear();
        for (_, e) in self.fields.clone() {
            self.observe_on(e, "text", prop("line-edit", "text"));
            self.observe_on(e, "cursor-position", prop("line-edit", "cursor-position"));
            self.observe_on(e, "selectedText", meta("selectedText"));
            self.observe_on(e, "focus", meta("focus"));
        }
        self.observe_on(btn, "focus", meta("focus"));
        self.ev.last.clear();
        CHANGED.lock().unwrap().clear();
        Ok(())
    }

    fn key(&mut self, chord: &Chord) -> Result<(), String> {
        self.x.as_ref().unwrap().key(chord, &mut p)
    }

    fn click(
        &mut self,
        target: &str,
        at: Option<usize>,
        count: u32,
        button: u32,
    ) -> Result<(), String> {
        let pt = self.point(target, at)?;
        self.x
            .as_ref()
            .unwrap()
            .click_button(pt, count, button as u8, &mut p)
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.x.as_ref().unwrap().type_text(text, &mut p)
    }

    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
        let (a, b) = (
            self.point(target, Some(from))?,
            self.point(target, Some(to))?,
        );
        self.x.as_ref().unwrap().drag(a, b, count, &mut p)
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        let announced: HashSet<(u64, svl_id)> = std::mem::take(&mut *CHANGED.lock().unwrap())
            .into_iter()
            .collect();
        let mut cp = Vec::new();
        let mut now: HashMap<(u64, &'static str), String> = HashMap::new();
        for (a, e) in self.fields.clone() {
            let (s, u16caret, raw) = na_state(e)?;
            // the backend's own channel agrees with the hook
            let caret = br()
                .get_prop(e, prop("line-edit", "cursor-position"))
                .map_err(|e| format!("{e:?}"))?;
            if caret != Value::I64(u16caret.unwrap_or(-1)) {
                return Err(format!(
                    "{a}: backend cursor-position {caret:?}, widget {u16caret:?}"
                ));
            }
            let text = s.text.clone().unwrap();
            let [x, y] = s.sel.unwrap();
            let selected: String = text
                .chars()
                .skip(x.min(y))
                .take(x.max(y) - x.min(y))
                .collect();
            now.insert((e, "text"), text);
            now.insert((e, "cursor-position"), raw["u16caret"].to_string());
            now.insert((e, "selectedText"), selected);
            now.insert((e, "focus"), s.focus.to_string());
            self.trace.state(e, &s);
            cp.push((a, s));
        }
        let (s, _, _) = na_state(self.button)?;
        now.insert((self.button, "focus"), s.focus.to_string());
        self.trace.state(self.button, &s);
        cp.push((BUTTON.to_string(), s));
        observe_selections(
            self.x.as_ref().unwrap(),
            &self.selections,
            &mut p,
            &mut self.trace,
            &mut cp,
        )?;
        for (h, name, id) in self.observed.clone() {
            let Some(v) = now.get(&(h, name)) else {
                continue;
            };
            if let Some(old) = self.ev.last.get(&(h, name)) {
                if old != v {
                    let c = self.ev.counts.entry(name.to_string()).or_default();
                    c.0 += 1;
                    if announced.contains(&(h, id)) {
                        c.1 += 1;
                    }
                }
            }
            self.ev.last.insert((h, name), v.clone());
        }
        Ok(cp)
    }

    fn teardown(&mut self) -> Result<String, String> {
        for (h, _, id) in std::mem::take(&mut self.observed) {
            let _ = br().observe(h, id, false);
        }
        br().destroy(self.win);
        self.trace.destroy(self.win);
        self.x = None;
        p(100);
        CHANGED.lock().unwrap().clear();
        Ok(std::mem::take(&mut self.trace).text())
    }
}

fn capture() {
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
    p(0); // creates the QApplication
    let keys = Keys::load("linux").unwrap();
    let pins = load_pins().unwrap();
    let mut d = Qt {
        trace: Trace::default(),
        win: 0,
        fields: vec![],
        button: 0,
        x: None,
        ev: Evidence::default(),
        observed: vec![],
        selections: vec![],
    };
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| panic!("{e}"));
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let settings: serde_json::Value = serde_json::from_str(&hooks::na_settings()).unwrap();
    let version = settings["qtVersion"]
        .as_str()
        .expect("qtVersion")
        .to_string();
    let mut settings: BTreeMap<String, String> = settings
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| *k != "qtVersion")
        .map(|(k, v)| (k.clone(), v.to_string()))
        .collect();
    settings.insert(
        "QT_IM_MODULE".into(),
        serde_json::Value::String(std::env::var("QT_IM_MODULE").unwrap_or_default()).to_string(),
    );
    let settings = settings.into_iter().collect();
    let mut observe: Vec<(String, String)> =
        d.ev.counts
            .iter()
            .map(|(k, (c, a))| (k.clone(), format!("{{\"changes\":{c},\"announced\":{a}}}")))
            .collect();
    observe.extend(d.ev.unsupported.iter().map(|(k, m)| {
        (
            k.clone(),
            format!(
                "{{\"unsupported\":{}}}",
                serde_json::Value::String(m.clone())
            ),
        )
    }));
    let cap = Capture {
        platform: "linux".into(),
        toolkit: "qt".into(),
        version,
        injector: "xtest".into(),
        settings,
        observe,
        not_applicable: vec![],
        pins: results,
    };
    finish(&cap, &traces).unwrap_or_else(|e| panic!("{e}"));
}

fn main() {
    counter_qt::run_as_test("na_capture", || {
        if let Some(ok) = rerun_on_private_display(&["na_capture"]) {
            if !ok {
                eprintln!("the capture on the private display failed (output above)");
                std::process::exit(1);
            }
            return;
        }
        capture();
    });
}
