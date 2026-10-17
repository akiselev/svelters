//! Test support: a GTK thread, a recording stub `svl_host`, and a name-based driver over the C ABI.
//!
//! GTK may only be used from the thread that initialised it, while libtest runs every test on its
//! own thread. `on_gtk` therefore runs closures on one dedicated GTK thread per process (under
//! nextest, which uses a process per test, that is one thread per test).

use crate::generated::tables::SIGNALS;
use crate::GtkBackend;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use svelters_abi::*;

type Job = Box<dyn FnOnce() + Send>;

fn gtk_thread() -> &'static Mutex<Sender<Job>> {
    static TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::Builder::new()
            .name("gtk-main".into())
            .spawn(move || {
                if let Err(e) = gtk::init() {
                    eprintln!("gtk::init failed (run under xvfb-run): {e}");
                    return; // dropping rx makes every on_gtk fail loudly
                }
                for job in rx {
                    job();
                }
            })
            .expect("spawn gtk thread");
        Mutex::new(tx)
    })
}

/// Run `f` on the GTK thread and propagate its panic, if any.
pub fn on_gtk<F: FnOnce() + Send + 'static>(f: F) {
    let (done_tx, done_rx) = channel();
    let job: Job = Box::new(move || {
        let _ = done_tx.send(catch_unwind(AssertUnwindSafe(f)));
    });
    gtk_thread().lock().unwrap().send(job).expect("GTK thread is not running (is DISPLAY set? use xvfb-run)");
    match done_rx.recv().expect("GTK thread died") {
        Ok(()) => {}
        Err(p) => resume_unwind(p),
    }
}

/// Process pending main-context events for about `ms` milliseconds.
pub fn pump(ms: u64) {
    let ctx = glib::MainContext::default();
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        while ctx.pending() {
            ctx.iteration(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum V {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    Handle(u64),
    /// Flags (names) or any list.
    List(Vec<V>),
    /// Record fields by portable name.
    Record(Vec<(String, V)>),
    /// Opaque boxed handle.
    Boxed(u64),
}

#[derive(Default)]
struct Recorder {
    events: RefCell<Vec<(u64, u32, Vec<Value>)>>,
    destroyed: RefCell<Vec<(u64, u32)>>,
    checkpoints: std::cell::Cell<u32>,
}

unsafe extern "C" fn h_post(p: *mut c_void, h: svl_handle, sig: svl_id, args: *const svl_value, n: usize) {
    let r = unsafe { &*(p as *const Recorder) };
    let args = unsafe { Value::vec_from_raw(args, n) };
    r.events.borrow_mut().push((h, sig, args));
}
unsafe extern "C" fn h_checkpoint(p: *mut c_void) {
    let r = unsafe { &*(p as *const Recorder) };
    r.checkpoints.set(r.checkpoints.get() + 1);
}
unsafe extern "C" fn h_call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> std::ffi::c_int {
    -1
}
unsafe extern "C" fn h_destroyed(p: *mut c_void, h: svl_handle, reason: u32) {
    let r = unsafe { &*(p as *const Recorder) };
    r.destroyed.borrow_mut().push((h, reason));
}
unsafe extern "C" fn h_invoke(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> std::ffi::c_int {
    -1
}
unsafe extern "C" fn h_release(_: *mut c_void, _: u64) {}

/// Drives a `GtkBackend` through its C vtable with names instead of IDs, recording host callbacks.
///
/// Values go through the raw ABI exactly as the host would send them. The harness stands in for
/// the host's name resolution only in the `set_str`/`get` conveniences: a string for an enum or
/// flags property is looked up in the type descriptor, and enum results are named again.
pub struct Harness {
    pub backend: GtkBackend,
    rec: Box<Recorder>,
    _host: Box<svl_host>,
    kinds: RefCell<HashMap<u64, u32>>,
}

impl Harness {
    pub fn new() -> Harness {
        let backend = GtkBackend::new().expect("backend");
        let rec = Box::new(Recorder::default());
        let host = Box::new(svl_host {
            self_: &*rec as *const Recorder as *mut c_void,
            post_event: h_post,
            checkpoint: h_checkpoint,
            call_sync: h_call_sync,
            on_destroyed: h_destroyed,
            invoke_callback: h_invoke,
            release_callback: h_release,
        });
        unsafe { backend.attach_host(&*host) };
        Harness { backend, rec, _host: host, kinds: RefCell::new(HashMap::new()) }
    }

    fn vt(&self) -> &svl_backend {
        unsafe { &*self.backend.as_raw() }
    }

    /// The safe view of the vtable (descriptors, typed calls).
    pub fn backend_ref(&self) -> BackendRef<'_> {
        unsafe { BackendRef::from_raw(self.backend.as_raw()) }.expect("abi version")
    }

    fn err(&self) -> String {
        let vt = self.vt();
        unsafe { CStr::from_ptr((vt.last_error)(vt.self_)).to_string_lossy().into_owned() }
    }

    fn check(&self, rc: i32) -> Result<(), String> {
        if rc == 0 { Ok(()) } else { Err(self.err()) }
    }

    fn resolve(&self, what: u32, kind: u32, name: &str) -> u32 {
        let vt = self.vt();
        unsafe { (vt.resolve)(vt.self_, what, kind, name.as_ptr() as *const _, name.len()) }
    }

    pub fn kind_id(&self, name: &str) -> u32 {
        let k = self.resolve(SVL_KIND, 0, name);
        assert_ne!(k, 0, "unknown kind {name}");
        k
    }
    pub fn resolve_prop(&self, kind: &str, name: &str) -> u32 {
        self.resolve(SVL_PROP, self.kind_id(kind), name)
    }
    pub fn resolve_signal(&self, kind: &str, name: &str) -> u32 {
        self.resolve(SVL_SIGNAL, self.kind_id(kind), name)
    }

    pub fn try_create(&self, kind: &str) -> Result<u64, String> {
        let k = self.kind_id(kind);
        let vt = self.vt();
        let h = unsafe { (vt.create)(vt.self_, k) };
        if h == 0 {
            return Err(self.err());
        }
        self.kinds.borrow_mut().insert(h, k);
        Ok(h)
    }
    pub fn create(&self, kind: &str) -> u64 {
        self.try_create(kind).unwrap_or_else(|e| panic!("create {kind}: {e}"))
    }

    fn kind_of(&self, h: u64) -> u32 {
        match self.kinds.borrow().get(&h) {
            Some(k) => *k,
            // Borrowed handles are minted by the backend: ask it.
            None => self.handle_info(h).map(|i| i.kind).unwrap_or(0),
        }
    }

    pub fn insert(&self, parent: u64, child: u64, before: Option<u64>, slot: Option<&str>) -> Result<(), String> {
        let slot = match slot {
            Some(n) => {
                let s = self.resolve(SVL_SLOT, self.kind_of(parent), n);
                if s == 0 {
                    return Err(format!("unknown slot {n}"));
                }
                s
            }
            None => 0,
        };
        let vt = self.vt();
        self.check(unsafe { (vt.insert)(vt.self_, parent, child, before.unwrap_or(0), slot) })
    }
    pub fn remove(&self, parent: u64, child: u64) -> Result<(), String> {
        let vt = self.vt();
        self.check(unsafe { (vt.remove)(vt.self_, parent, child) })
    }
    pub fn destroy(&self, h: u64) {
        let vt = self.vt();
        unsafe { (vt.destroy)(vt.self_, h) };
    }

    /// Sets a property from an owned ABI value.
    pub fn set_value(&self, h: u64, prop: &str, v: &Value) -> Result<(), String> {
        let id = self.resolve(SVL_PROP, self.kind_of(h), prop);
        if id == 0 {
            return Err(format!("unknown property {prop}"));
        }
        let vt = self.vt();
        v.with_raw(|raw| self.check(unsafe { (vt.set_prop)(vt.self_, h, id, raw) }))
    }

    /// The descriptor of a property as the host would see it.
    pub fn prop_desc(&self, h: u64, prop: &str) -> Option<MemberDesc> {
        let k = self.kind_of(h);
        let id = self.resolve(SVL_PROP, k, prop);
        self.backend_ref().describe(NameKind::Prop, k, id)
    }

    pub fn type_desc(&self, ty: u32) -> Option<TypeDesc> {
        self.backend_ref().describe_type(ty)
    }

    /// An enum or flags value by member name(s), as the host would encode it.
    pub fn named(&self, h: u64, prop: &str, names: &[&str]) -> Result<Value, String> {
        let d = self.prop_desc(h, prop).ok_or("no descriptor")?;
        let slot = d.params.first().ok_or("read-only")?;
        let t = self.type_desc(slot.ty).ok_or("no type descriptor")?;
        let mut bits = 0i64;
        for n in names {
            bits |= t.members.iter().find(|m| m.name == *n).ok_or_else(|| format!("no member {n}"))?.value;
        }
        Ok(match slot.tag {
            SVL_ENUM => Value::Enum { ty: slot.ty, value: bits },
            SVL_FLAGS => Value::Flags { ty: slot.ty, mask: bits as u64 },
            t => return Err(format!("not an enum or flags property (tag {t})")),
        })
    }

    pub fn set_str(&self, h: u64, prop: &str, s: &str) -> Result<(), String> {
        if let Some(d) = self.prop_desc(h, prop) {
            if matches!(d.params.first().map(|p| p.tag), Some(SVL_ENUM | SVL_FLAGS)) {
                let names: Vec<&str> = s.split_whitespace().collect();
                let v = self.named(h, prop, &names)?;
                return self.set_value(h, prop, &v);
            }
        }
        self.set_value(h, prop, &Value::Str(s.into()))
    }
    pub fn set_i64(&self, h: u64, prop: &str, i: i64) -> Result<(), String> {
        self.set_value(h, prop, &Value::I64(i))
    }
    pub fn set_bool(&self, h: u64, prop: &str, b: bool) -> Result<(), String> {
        self.set_value(h, prop, &Value::Bool(b))
    }
    pub fn set_handle(&self, h: u64, prop: &str, o: u64) -> Result<(), String> {
        self.set_value(h, prop, &Value::Handle(o))
    }

    /// Reads a property as an owned ABI value.
    pub fn get_value(&self, h: u64, prop: &str) -> Value {
        let id = self.resolve(SVL_PROP, self.kind_of(h), prop);
        assert_ne!(id, 0, "unknown property {prop}");
        let vt = self.vt();
        let mut out = svl_value::null();
        self.check(unsafe { (vt.get_prop)(vt.self_, h, id, &mut out) }).unwrap();
        unsafe { Value::from_raw(&out) }
    }

    /// Reads a property and names enum and flags members and record fields via the descriptors.
    pub fn get(&self, h: u64, prop: &str) -> V {
        self.name_value(&self.get_value(h, prop))
    }

    pub fn name_value(&self, v: &Value) -> V {
        match v {
            Value::Null => V::Null,
            Value::Bool(b) => V::Bool(*b),
            Value::I64(i) => V::I64(*i),
            Value::U64(u) => V::I64(*u as i64),
            Value::F64(f) => V::F64(*f),
            Value::Str(s) => V::Str(s.clone()),
            Value::Handle(h) => V::Handle(*h),
            Value::List(items) => V::List(items.iter().map(|x| self.name_value(x)).collect()),
            Value::Enum { ty, value } => {
                let t = self.type_desc(*ty).expect("enum descriptor");
                V::Str(t.members.iter().find(|m| m.value == *value).map(|m| m.name.clone()).unwrap_or_else(|| value.to_string()))
            }
            Value::Flags { ty, mask } => {
                let t = self.type_desc(*ty).expect("flags descriptor");
                V::List(t.members.iter().filter(|m| m.value != 0 && *mask & m.value as u64 == m.value as u64).map(|m| V::Str(m.name.clone())).collect())
            }
            Value::Record { ty, fields } => {
                let t = self.type_desc(*ty).expect("record descriptor");
                V::Record(t.fields.iter().zip(fields).map(|(f, x)| (f.name.clone().unwrap_or_default(), self.name_value(x))).collect())
            }
            Value::Boxed { handle, .. } => V::Boxed(*handle),
            other => panic!("unexpected value {other:?}"),
        }
    }

    pub fn call(&self, h: u64, method: &str) -> Result<(), String> {
        let id = self.resolve(SVL_METHOD, self.kind_of(h), method);
        if id == 0 {
            return Err(format!("unknown method {method}"));
        }
        let vt = self.vt();
        let mut out = svl_value::null();
        self.check(unsafe { (vt.call)(vt.self_, h, id, std::ptr::null(), 0, &mut out) })
    }
    /// Encodes `v` for a descriptor slot the way the host does: enum and flags members by name.
    fn encode_arg(&self, slot: &SlotDesc, v: &V) -> Result<Value, String> {
        let members = |names: Vec<&str>| -> Result<i64, String> {
            let t = self.type_desc(slot.ty).ok_or("no type descriptor")?;
            let mut bits = 0i64;
            for n in names {
                bits |= t.members.iter().find(|m| m.name == n).ok_or_else(|| format!("'{n}' is not a member of {}", t.name))?.value;
            }
            Ok(bits)
        };
        Ok(match (slot.tag, v) {
            (SVL_ENUM, V::Str(n)) => Value::Enum { ty: slot.ty, value: members(vec![n.as_str()])? },
            (SVL_ENUM, V::I64(i)) => Value::Enum { ty: slot.ty, value: *i },
            (SVL_FLAGS, V::Str(n)) => Value::Flags { ty: slot.ty, mask: members(n.split('|').collect())? as u64 },
            (SVL_FLAGS, V::List(l)) => {
                let names = l.iter().map(|x| if let V::Str(s) = x { Ok(s.as_str()) } else { Err("flag names are strings".to_string()) }).collect::<Result<Vec<_>, _>>()?;
                Value::Flags { ty: slot.ty, mask: members(names)? as u64 }
            }
            (SVL_FLAGS, V::I64(i)) => Value::Flags { ty: slot.ty, mask: *i as u64 },
            (_, V::Null) => Value::Null,
            (_, V::Bool(b)) => Value::Bool(*b),
            (_, V::I64(i)) => Value::I64(*i),
            (_, V::F64(f)) => Value::F64(*f),
            (_, V::Str(s)) => Value::Str(s.clone()),
            (_, V::Handle(h)) => Value::Handle(*h),
            (_, V::Boxed(h)) => Value::Boxed { ty: slot.ty, handle: *h },
            (_, V::List(l)) => Value::List(l.iter().map(|x| self.encode_arg(&SlotDesc::new(SVL_NULL), x)).collect::<Result<_, _>>()?),
            (_, V::Record(_)) => return Err("records are not supported by call_args".into()),
        })
    }

    /// Call a method (generated or typelib) with arguments and return its result. Arguments are
    /// encoded with the method's descriptor, like the host does (enum and flags by name).
    pub fn call_args(&self, h: u64, method: &str, args: &[V]) -> Result<V, String> {
        let k = self.kind_of(h);
        let id = self.resolve(SVL_METHOD, k, method);
        if id == 0 {
            return Err(format!("unknown method {method}"));
        }
        let desc = self.backend_ref().describe(NameKind::Method, k, id);
        let mut vals = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let slot = desc.as_ref().and_then(|d| d.params.get(i).cloned()).unwrap_or_else(|| SlotDesc::new(SVL_NULL));
            vals.push(self.encode_arg(&slot, a)?);
        }
        match self.backend_ref().call(h, id, &vals) {
            Ok(v) => Ok(self.name_value(&v)),
            Err(e) => Err(e.message),
        }
    }
    /// The descriptor of a method as the host would see it.
    pub fn method_desc(&self, h: u64, method: &str) -> Option<MemberDesc> {
        let k = self.kind_of(h);
        let id = self.resolve(SVL_METHOD, k, method);
        self.backend_ref().describe(NameKind::Method, k, id)
    }
    /// The id `resolve(SVL_METHOD)` gives `method` on `h`'s kind (0: unknown).
    pub fn resolve_method(&self, h: u64, method: &str) -> u32 {
        self.resolve(SVL_METHOD, self.kind_of(h), method)
    }
    pub fn listen(&self, h: u64, signal: &str, on: bool) -> Result<(), String> {
        let id = self.resolve(SVL_SIGNAL, self.kind_of(h), signal);
        if id == 0 {
            return Err(format!("unknown signal {signal}"));
        }
        let vt = self.vt();
        self.check(unsafe { (vt.listen)(vt.self_, h, id, on) })
    }

    /// Events posted since the last call: (handle, signal name).
    pub fn take_events(&self) -> Vec<(u64, String)> {
        self.take_events_with_args().into_iter().map(|(h, s, _)| (h, s)).collect()
    }
    pub fn take_events_with_args(&self) -> Vec<(u64, String, Vec<V>)> {
        let evs: Vec<_> = self.rec.events.borrow_mut().drain(..).collect();
        evs.into_iter().map(|(h, s, a)| (h, SIGNALS[s as usize - 1].name.to_string(), a.iter().map(|x| self.name_value(x)).collect())).collect()
    }
    pub fn take_destroyed(&self) -> Vec<u64> {
        self.rec.destroyed.borrow_mut().drain(..).map(|(h, _)| h).collect()
    }
    /// Native deaths reported since the last call, with their `SVL_GONE_*` reason.
    pub fn take_gone(&self) -> Vec<(u64, u32)> {
        self.rec.destroyed.borrow_mut().drain(..).collect()
    }
    pub fn handle_info(&self, h: u64) -> Result<svl_handle_info, String> {
        let vt = self.vt();
        let mut out = svl_handle_info::default();
        self.check(unsafe { (vt.handle_info)(vt.self_, h, &mut out) })?;
        Ok(out)
    }
    pub fn schedule_wake(&self, ms: i64) {
        let vt = self.vt();
        unsafe { (vt.schedule_wake)(vt.self_, ms) };
    }

    pub fn obj(&self, h: u64) -> glib::Object {
        self.backend.object(h).expect("live handle")
    }
    pub fn widget(&self, h: u64) -> gtk::Widget {
        self.obj(h).downcast().expect("widget")
    }
    /// Direct children of a widget in native order.
    pub fn children(&self, h: u64) -> Vec<gtk::Widget> {
        let mut out = Vec::new();
        let mut cur = self.widget(h).first_child();
        while let Some(w) = cur {
            cur = w.next_sibling();
            out.push(w);
        }
        out
    }
}
