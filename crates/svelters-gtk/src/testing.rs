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
}

/// What the harness host answers to `call_sync`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SyncReply {
    /// Busy: the call is recorded as an ordinary event (what a real host does with a deferred one).
    #[default]
    Busy,
    /// Failed (`-1`): the backend uses the default.
    Failed,
    /// Ok with this handler result.
    Value(V),
}

#[derive(Default)]
struct Recorder {
    reply: RefCell<SyncReply>,
    /// `call_sync` calls that were answered with a value or failure, with their arguments.
    sync_calls: RefCell<Vec<(u64, u32, Vec<V>)>>,
    events: RefCell<Vec<(u64, u32, Vec<V>)>>,
    destroyed: RefCell<Vec<(u64, u32)>>,
    checkpoints: std::cell::Cell<u32>,
    frames: std::cell::Cell<u32>,
}

unsafe fn to_v(v: &svl_value) -> V {
    unsafe {
        match v.tag {
            SVL_BOOL => V::Bool(v.u.b),
            SVL_I64 => V::I64(v.u.i),
            SVL_F64 => V::F64(v.u.f),
            SVL_STR => V::Str(crate::convert::as_str(v).unwrap_or("").to_string()),
            SVL_HANDLE => V::Handle(v.u.h),
            _ => V::Null,
        }
    }
}

unsafe extern "C" fn h_post(p: *mut c_void, h: svl_handle, sig: svl_id, args: *const svl_value, n: usize) {
    let r = unsafe { &*(p as *const Recorder) };
    let args = (0..n).map(|i| unsafe { to_v(&*args.add(i)) }).collect();
    r.events.borrow_mut().push((h, sig, args));
}
unsafe extern "C" fn h_checkpoint(p: *mut c_void) {
    let r = unsafe { &*(p as *const Recorder) };
    r.checkpoints.set(r.checkpoints.get() + 1);
}
unsafe extern "C" fn h_call_sync(p: *mut c_void, h: svl_handle, sig: svl_id, args: *const svl_value, n: usize, out: *mut svl_value) -> std::ffi::c_int {
    let r = unsafe { &*(p as *const Recorder) };
    let args: Vec<V> = (0..n).map(|i| unsafe { to_v(&*args.add(i)) }).collect();
    let reply = r.reply.borrow().clone();
    match reply {
        SyncReply::Busy => {
            r.events.borrow_mut().push((h, sig, args));
            SVL_SYNC_BUSY
        }
        SyncReply::Failed => {
            r.sync_calls.borrow_mut().push((h, sig, args));
            SVL_SYNC_FAILED
        }
        SyncReply::Value(v) => {
            r.sync_calls.borrow_mut().push((h, sig, args));
            unsafe {
                *out = match v {
                    V::Bool(b) => svl_value { tag: SVL_BOOL, u: svl_value_u { b } },
                    V::I64(i) => svl_value { tag: SVL_I64, u: svl_value_u { i } },
                    _ => svl_value { tag: SVL_NULL, u: svl_value_u { i: 0 } },
                };
            }
            SVL_SYNC_OK
        }
    }
}
unsafe extern "C" fn h_frame(p: *mut c_void) {
    let r = unsafe { &*(p as *const Recorder) };
    r.frames.set(r.frames.get() + 1);
}
unsafe extern "C" fn h_destroyed(p: *mut c_void, h: svl_handle, reason: u32) {
    let r = unsafe { &*(p as *const Recorder) };
    r.destroyed.borrow_mut().push((h, reason));
}

/// Drives a `GtkBackend` through its C vtable with names instead of IDs, recording host callbacks.
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
            frame: h_frame,
        });
        unsafe { backend.attach_host(&*host) };
        Harness { backend, rec, _host: host, kinds: RefCell::new(HashMap::new()) }
    }

    fn vt(&self) -> &svl_backend {
        unsafe { &*self.backend.as_raw() }
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
        self.kinds.borrow().get(&h).copied().unwrap_or(0)
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

    fn set(&self, h: u64, prop: &str, v: svl_value) -> Result<(), String> {
        let id = self.resolve(SVL_PROP, self.kind_of(h), prop);
        if id == 0 {
            return Err(format!("unknown property {prop}"));
        }
        let vt = self.vt();
        self.check(unsafe { (vt.set_prop)(vt.self_, h, id, &v) })
    }
    pub fn set_str(&self, h: u64, prop: &str, s: &str) -> Result<(), String> {
        self.set(h, prop, svl_value { tag: SVL_STR, u: svl_value_u { s: svl_str { ptr: s.as_ptr() as *const _, len: s.len() } } })
    }
    pub fn set_i64(&self, h: u64, prop: &str, i: i64) -> Result<(), String> {
        self.set(h, prop, svl_value { tag: SVL_I64, u: svl_value_u { i } })
    }
    pub fn set_bool(&self, h: u64, prop: &str, b: bool) -> Result<(), String> {
        self.set(h, prop, svl_value { tag: SVL_BOOL, u: svl_value_u { b } })
    }
    pub fn set_handle(&self, h: u64, prop: &str, o: u64) -> Result<(), String> {
        self.set(h, prop, svl_value { tag: SVL_HANDLE, u: svl_value_u { h: o } })
    }
    pub fn get(&self, h: u64, prop: &str) -> V {
        let id = self.resolve(SVL_PROP, self.kind_of(h), prop);
        assert_ne!(id, 0, "unknown property {prop}");
        let vt = self.vt();
        let mut out = svl_value { tag: SVL_NULL, u: svl_value_u { i: 0 } };
        self.check(unsafe { (vt.get_prop)(vt.self_, h, id, &mut out) }).unwrap();
        unsafe { to_v(&out) }
    }
    pub fn call(&self, h: u64, method: &str) -> Result<(), String> {
        let id = self.resolve(SVL_METHOD, self.kind_of(h), method);
        if id == 0 {
            return Err(format!("unknown method {method}"));
        }
        let vt = self.vt();
        let mut out = svl_value { tag: SVL_NULL, u: svl_value_u { i: 0 } };
        self.check(unsafe { (vt.call)(vt.self_, h, id, std::ptr::null(), 0, &mut out) })
    }
    pub fn listen(&self, h: u64, signal: &str, on: bool) -> Result<(), String> {
        let id = self.resolve(SVL_SIGNAL, self.kind_of(h), signal);
        if id == 0 {
            return Err(format!("unknown signal {signal}"));
        }
        let vt = self.vt();
        self.check(unsafe { (vt.listen)(vt.self_, h, id, on) })
    }

    /// Chooses what the harness host answers to `call_sync` (default: busy).
    pub fn set_sync_reply(&self, r: SyncReply) {
        *self.rec.reply.borrow_mut() = r;
    }

    /// `call_sync` calls answered with a value or a failure since the last take.
    pub fn take_sync_calls(&self) -> Vec<(u64, String, Vec<V>)> {
        let calls: Vec<_> = self.rec.sync_calls.borrow_mut().drain(..).collect();
        calls.into_iter().map(|(h, sig, a)| (h, crate::generated::tables::SIGNALS[sig as usize - 1].name.to_string(), a)).collect()
    }

    /// `host.frame` calls received since the last take.
    pub fn take_frames(&self) -> u32 {
        self.rec.frames.replace(0)
    }

    /// Asks the backend for a frame (`request_frame`).
    pub fn request_frame(&self) {
        let vt = self.vt();
        unsafe { (vt.request_frame)(vt.self_) }
    }

    /// Events posted since the last call: (handle, signal name).
    pub fn take_events(&self) -> Vec<(u64, String)> {
        self.take_events_with_args().into_iter().map(|(h, s, _)| (h, s)).collect()
    }
    pub fn take_events_with_args(&self) -> Vec<(u64, String, Vec<V>)> {
        self.rec.events.borrow_mut().drain(..).map(|(h, s, a)| (h, SIGNALS[s as usize - 1].name.to_string(), a)).collect()
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
