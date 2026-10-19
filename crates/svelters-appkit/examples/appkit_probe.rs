//! The AppKit runtime prober (AI1 slice 4), driven by `tools/metadata/appkit/probe.py`.
//!
//! macOS only (prints a note and exits elsewhere). Runs on the main thread through the same
//! backend harness the generated tests use (`svelters_appkit::testing::Harness`), so what it
//! observes is what a generated test of the same fact will see:
//!
//!   appkit_probe --plan plan.json [--class NSButton] [--after <prop>] [--prop <prop>] [--skip-class]
//!
//! The plan (written by probe.py) lists, per class, the subject element its facts are tested on,
//! the props to probe with their candidate samples, the signals to listen to while setting, and
//! the notification names whose AppKit constants to look up. For each prop the prober creates a
//! fresh subject, reads the default, observes the KVC key with KVO, sets the first candidate that
//! differs from the default through the backend (KVC `setValue:forKey:`), reads it back and
//! records: the outcome (`exact`, `normalized`, `ignored`, `error`, `unreadable`), whether KVO
//! fired, and which listened signals the programmatic set emitted (an echo).
//!
//! Output: one JSON object per line on stdout, written as soon as it is known, so that when a
//! native crash kills the process the driver keeps everything before it and knows which prop was
//! in flight (it reruns the class with `--after <that prop>`).

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_foundation::NSString;
    use serde_json::{Value as J, json};
    use std::cell::Cell;
    use std::ffi::{CString, c_char, c_void};
    use std::io::Write;
    use svelters_abi::{SVL_T_INSETS, SVL_T_POINT, SVL_T_RECT, SVL_T_SIZE, Value};
    use svelters_apple::objc::bridge;
    use svelters_appkit::testing::Harness;

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the observer does not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersProbeObserver"]
        #[ivars = Cell<u32>]
        struct Observer;

        impl Observer {
            #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
            fn observe(&self, _path: Option<&NSString>, _obj: Option<&AnyObject>, _change: Option<&AnyObject>, _ctx: *mut c_void) {
                self.ivars().set(self.ivars().get() + 1);
            }
        }
    );

    impl Observer {
        fn new(mtm: MainThreadMarker) -> Retained<Observer> {
            let this = Self::alloc(mtm).set_ivars(Cell::new(0));
            unsafe { msg_send![super(this), init] }
        }
    }

    fn emit(line: J) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    /// A plan sample `{"t": "bool"|"i64"|"f64"|"str"|"names"|"rect"|"size"|"point"|"insets", "v": ...}`.
    fn sample(j: &J) -> Option<Value> {
        let v = &j["v"];
        Some(match j["t"].as_str()? {
            "bool" => Value::Bool(v.as_bool()?),
            "i64" => Value::I64(v.as_i64()?),
            "f64" => Value::F64(v.as_f64()?),
            "str" => Value::Str(v.as_str()?.to_string()),
            "names" => Value::List(v.as_array()?.iter().map(|x| Some(Value::Str(x.as_str()?.to_string()))).collect::<Option<_>>()?),
            t @ ("rect" | "size" | "point" | "insets") => Value::Record {
                ty: match t {
                    "rect" => SVL_T_RECT,
                    "size" => SVL_T_SIZE,
                    "point" => SVL_T_POINT,
                    _ => SVL_T_INSETS,
                },
                fields: v.as_array()?.iter().map(|x| x.as_f64().map(Value::F64)).collect::<Option<_>>()?,
            },
            _ => return None,
        })
    }

    /// A read value as JSON: scalars as themselves, records as `{"record": [fields]}`, anything
    /// else (handles, lists of objects) by kind only, so the output stays deterministic.
    fn out(v: &Value) -> J {
        match v {
            Value::Null => J::Null,
            Value::Bool(b) => json!(b),
            Value::I64(i) => json!(i),
            Value::U64(u) => json!(u),
            Value::F64(f) => json!(f),
            Value::Str(s) => json!(s),
            Value::Record { fields, .. } => json!({ "record": fields.iter().map(out).collect::<Vec<_>>() }),
            Value::List(items) if items.iter().all(|x| matches!(x, Value::Str(_) | Value::I64(_) | Value::F64(_) | Value::Bool(_))) => {
                J::Array(items.iter().map(out).collect())
            }
            Value::Handle(_) => json!({ "kind": "object" }),
            Value::List(_) => json!({ "kind": "list" }),
            _ => json!({ "kind": "other" }),
        }
    }

    fn equal(a: &Value, b: &Value) -> bool {
        a == b
    }

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    /// The value of an `NSNotificationName` constant exported by the loaded frameworks, if any.
    fn notification_constant(name: &str) -> Option<String> {
        let sym = CString::new(name).ok()?;
        // RTLD_DEFAULT on Darwin is ((void *)-2).
        let p = unsafe { dlsym(-2isize as *mut c_void, sym.as_ptr()) } as *const *const AnyObject;
        if p.is_null() {
            return None;
        }
        let obj = unsafe { *p };
        if obj.is_null() {
            return None;
        }
        let s: &AnyObject = unsafe { &*obj };
        let utf8: *const c_char = unsafe { msg_send![s, UTF8String] };
        if utf8.is_null() {
            return None;
        }
        Some(unsafe { std::ffi::CStr::from_ptr(utf8) }.to_string_lossy().to_string())
    }

    struct Args {
        plan: String,
        class: Option<String>,
        after: Option<String>,
        prop: Option<String>,
        skip_class: bool,
    }

    fn args() -> Args {
        let mut a = Args { plan: String::new(), class: None, after: None, prop: None, skip_class: false };
        let mut it = std::env::args().skip(1);
        while let Some(x) = it.next() {
            match x.as_str() {
                "--plan" => a.plan = it.next().unwrap_or_default(),
                "--class" => a.class = it.next(),
                "--after" => a.after = it.next(),
                "--prop" => a.prop = it.next(),
                "--skip-class" => a.skip_class = true,
                other => {
                    eprintln!("appkit_probe: unknown argument {other}");
                    std::process::exit(2);
                }
            }
        }
        if a.plan.is_empty() {
            eprintln!("usage: appkit_probe --plan plan.json [--class C] [--after prop] [--prop prop]");
            std::process::exit(2);
        }
        a
    }

    pub fn main() {
        let a = args();
        let plan: J = serde_json::from_str(&std::fs::read_to_string(&a.plan).expect("read plan")).expect("plan is JSON");
        let mtm = MainThreadMarker::new().expect("main thread");
        let h = Harness::shared();
        for c in plan["classes"].as_array().expect("classes") {
            let cls = c["class"].as_str().unwrap_or_default();
            if a.class.as_deref().is_some_and(|x| x != cls) {
                continue;
            }
            probe_class(h, mtm, c, &a);
        }
        emit(json!({ "done": true }));
    }

    fn probe_class(h: &'static Harness, mtm: MainThreadMarker, c: &J, a: &Args) {
        let cls = c["class"].as_str().unwrap_or_default();
        let subject = c["subject"].as_str().unwrap_or_default();
        let listen: Vec<&str> = c["listen"].as_array().map(|x| x.iter().filter_map(|s| s.as_str()).collect()).unwrap_or_default();
        if !a.skip_class && a.after.is_none() && a.prop.is_none() {
            // The class itself: instantiation, its signals delivering, its notification constants.
            let o = h.create(subject);
            let native = h.backend.object(o).map(|x| bridge::class_name_of(&x)).unwrap_or_default();
            let mut signals = serde_json::Map::new();
            for s in c["deliver"].as_array().into_iter().flatten() {
                let name = s["signal"].as_str().unwrap_or_default();
                h.take_events();
                let r = h.listen(o, name, true);
                if let Err(e) = r {
                    signals.insert(name.into(), json!({ "listen": e }));
                    continue;
                }
                match s["how"].as_str() {
                    Some("send_action") => h.send_action(o),
                    Some("post") => h.post_notification(o, s["notification"].as_str().unwrap_or_default()),
                    _ => {}
                }
                let ev = h.take_events();
                let got: Vec<&(u64, String, Vec<Value>)> = ev.iter().filter(|e| e.0 == o && e.1 == name).collect();
                signals.insert(
                    name.into(),
                    json!({ "delivered": !got.is_empty(), "args": got.first().map(|e| e.2.iter().map(out).collect::<Vec<_>>()) }),
                );
                let _ = h.listen(o, name, false);
            }
            let mut notes = serde_json::Map::new();
            for n in c["notifications"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                notes.insert(n.into(), json!({ "symbol": notification_constant(n) }));
            }
            emit(json!({ "class": cls, "subject": subject, "created": true, "native_class": native,
                         "signals": signals, "notifications": notes }));
            h.destroy(o);
            h.pump(5);
        }
        let mut started = a.after.is_none();
        for p in c["props"].as_array().into_iter().flatten() {
            let name = p["name"].as_str().unwrap_or_default();
            if !started {
                started = a.after.as_deref() == Some(name);
                continue;
            }
            if a.prop.as_deref().is_some_and(|x| x != name) {
                continue;
            }
            emit(json!({ "class": cls, "prop": name, "start": true }));
            let rec = probe_prop(h, mtm, subject, p, &listen);
            emit(json!({ "class": cls, "prop": name, "result": rec }));
        }
    }

    /// Probes one prop: the default from a fresh subject, then each candidate that differs from it on its
    /// own fresh subject until one round-trips exactly (`attempts` counts them). Without an exact one, the
    /// first attempt is reported.
    fn probe_prop(h: &'static Harness, mtm: MainThreadMarker, subject: &str, p: &J, listen: &[&str]) -> J {
        let name = p["name"].as_str().unwrap_or_default();
        let o = h.create(subject);
        let default = h.get(o, name);
        h.destroy(o);
        h.pump(2);
        let mut rec = serde_json::Map::new();
        rec.insert("default".into(), match &default {
            Ok(v) => out(v),
            Err(e) => json!({ "error": e }),
        });
        let cands: Vec<(Value, Value, &J)> = p["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                let v = sample(s)?;
                let e = s.get("expect").and_then(sample).unwrap_or_else(|| v.clone());
                Some((v, e, s))
            })
            .collect();
        let mut tries: Vec<&(Value, Value, &J)> = cands.iter().filter(|(_, e, _)| default.as_ref().map_or(true, |d| !equal(d, e))).collect();
        if tries.is_empty() {
            match cands.last() {
                Some(c) => tries.push(c),
                None => {
                    rec.insert("outcome".into(), json!("no_sample"));
                    return J::Object(rec);
                }
            }
        }
        let mut first: Option<serde_json::Map<String, J>> = None;
        for (n, (v, expect, raw)) in tries.iter().enumerate() {
            let mut a = attempt(h, mtm, subject, p, listen, &default, v, expect);
            a.insert("sample".into(), (*raw).clone());
            a.insert("attempts".into(), json!(n + 1));
            if a["outcome"] == "exact" {
                rec.extend(a);
                return J::Object(rec);
            }
            first.get_or_insert(a);
        }
        let mut a = first.unwrap();
        a.insert("attempts".into(), json!(tries.len()));
        rec.extend(a);
        J::Object(rec)
    }

    #[allow(clippy::too_many_arguments)]
    fn attempt(
        h: &'static Harness,
        mtm: MainThreadMarker,
        subject: &str,
        p: &J,
        listen: &[&str],
        default: &Result<Value, String>,
        v: &Value,
        expect: &Value,
    ) -> serde_json::Map<String, J> {
        let name = p["name"].as_str().unwrap_or_default();
        let key = p["key"].as_str().unwrap_or_default();
        let mut rec = serde_json::Map::new();
        let o = h.create(subject);
        let obj = h.backend.object(o).expect("live subject");
        for s in listen {
            let _ = h.listen(o, s, true);
        }
        h.take_events();
        let observer = Observer::new(mtm);
        let nskey = NSString::from_str(key);
        let observing = !key.is_empty()
            && bridge::catch("addObserver", || unsafe {
                let _: () = msg_send![&*obj, addObserver: &*observer, forKeyPath: &*nskey, options: 1usize, context: std::ptr::null_mut::<c_void>()];
            })
            .is_ok();
        let set = h.set(o, name, v.clone());
        let read = h.get(o, name);
        let kvo = observer.ivars().get();
        if observing {
            let _ = bridge::catch("removeObserver", || unsafe {
                let _: () = msg_send![&*obj, removeObserver: &*observer, forKeyPath: &*nskey];
            });
        }
        let events: Vec<String> =
            h.take_events().into_iter().filter(|e| e.0 == o).map(|e| e.1).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for s in listen {
            let _ = h.listen(o, s, false);
        }
        let outcome = match (&set, &read) {
            (Err(e), _) => {
                rec.insert("error".into(), json!(e));
                "error"
            }
            (Ok(()), Err(e)) => {
                rec.insert("error".into(), json!(e));
                "unreadable"
            }
            (Ok(()), Ok(r)) if equal(r, expect) => "exact",
            (Ok(()), Ok(r)) if default.as_ref().is_ok_and(|d| equal(d, r)) => "ignored",
            _ => "normalized",
        };
        if let Ok(r) = &read {
            rec.insert("read".into(), out(r));
        }
        rec.insert("outcome".into(), json!(outcome));
        rec.insert("kvo".into(), json!(observing && kvo > 0));
        rec.insert("events".into(), json!(events));
        drop(obj);
        h.destroy(o);
        h.pump(2);
        rec
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    mac::main();
    #[cfg(not(target_os = "macos"))]
    eprintln!("appkit_probe: macOS only (run tools/metadata/appkit/probe.py on a Mac)");
}
