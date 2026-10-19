//! Test harness for the AppKit backend (macOS): the generated verification tests
//! (`examples/counter-mac/tests/generated.rs`) and the hand-written ones drive the backend through
//! its `Backend` implementation with a recording host, on the main thread, without `[NSApp run]`.

use crate::AppKitBackend;
use crate::lookup;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;

/// A sample value in a static row table of the generated tests (`Value` itself owns heap data,
/// so the tables hold this and convert per row).
#[derive(Clone, Copy, Debug)]
pub enum S {
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'static str),
    /// A list of member names (an `NS_OPTIONS` sample).
    Names(&'static [&'static str]),
    /// A well-known record (`SVL_T_RECT`, ...) with its fields in JS order.
    Rec(svl_id, &'static [f64]),
}

impl S {
    pub fn v(&self) -> Value {
        match *self {
            S::Bool(b) => Value::Bool(b),
            S::I64(i) => Value::I64(i),
            S::F64(f) => Value::F64(f),
            S::Str(s) => Value::Str(s.into()),
            S::Names(n) => Value::List(n.iter().map(|x| Value::Str((*x).into())).collect()),
            S::Rec(ty, f) => Value::Record { ty, fields: f.iter().map(|x| Value::F64(*x)).collect() },
        }
    }
}

/// Pointer identity of a native object, for comparisons in tests.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ObjRef(pub usize);

fn oref(o: &AnyObject) -> ObjRef {
    ObjRef(o as *const AnyObject as usize)
}

struct Rec {
    events: RefCell<Vec<(u64, String, Vec<Value>)>>,
    gone: RefCell<Vec<(u64, u32)>>,
    reply: RefCell<Value>,
    frames: RefCell<u32>,
    checkpoints: RefCell<u32>,
    on_checkpoint: RefCell<Option<Box<dyn Fn()>>>,
}

struct TestHost(Rc<Rec>);

fn signal_name(sig: u32) -> String {
    lookup::signal_info(sig & SVL_SIGNAL_ID_MASK).map_or_else(|| format!("#{sig}"), |s| s.name.to_string())
}

impl HostCallbacks for TestHost {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.0.events.borrow_mut().push((h, signal_name(signal), args.to_vec()));
    }
    fn checkpoint(&self) {
        *self.0.checkpoints.borrow_mut() += 1;
        let hook = self.0.on_checkpoint.borrow_mut().take();
        if let Some(f) = hook {
            f();
        }
    }
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        self.0.events.borrow_mut().push((h, signal_name(signal), args.to_vec()));
        Ok(self.0.reply.borrow().clone())
    }
    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.0.gone.borrow_mut().push((h, reason));
    }
    fn frame(&self) {
        *self.0.frames.borrow_mut() += 1;
    }
}

pub struct CaseGuard {
    h: &'static Harness,
    before: Vec<u64>,
}

impl Drop for CaseGuard {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        let new: Vec<u64> = self.h.backend.handles().into_iter().filter(|x| !self.before.contains(x)).collect();
        // Newest first (children are usually created after their parents).
        for x in new.into_iter().rev() {
            self.h.destroy(x);
        }
        self.h.pump(20);
        self.h.take_gone();
    }
}

pub struct Harness {
    pub backend: AppKitBackend,
    rec: Rc<Rec>,
    host: &'static HostVtable,
}

thread_local! {
    static HARNESS: &'static Harness = Harness::make();
}

impl Harness {
    fn make() -> &'static Harness {
        let backend = AppKitBackend::new().expect("AppKit backend (run the tests on the main thread)");
        let rec = Rc::new(Rec {
            events: RefCell::new(Vec::new()),
            gone: RefCell::new(Vec::new()),
            reply: RefCell::new(Value::Null),
            frames: RefCell::new(0),
            checkpoints: RefCell::new(0),
            on_checkpoint: RefCell::new(None),
        });
        let host: &'static HostVtable = Box::leak(Box::new(HostVtable::new(TestHost(rec.clone()))));
        backend.attach_host(host.as_raw());
        Box::leak(Box::new(Harness { backend, rec, host }))
    }

    /// A guard for one test case: if the case panics, dropping the guard destroys every handle it
    /// created, so a failing case cannot fail the leak check of the cases after it. A case that
    /// passes is left alone, so its own leaks still show up in the leak check.
    pub fn case_guard(&'static self) -> CaseGuard {
        CaseGuard { h: self, before: self.backend.handles() }
    }

    /// The process-wide harness (one backend, one recording host).
    pub fn shared() -> &'static Harness {
        HARNESS.with(|h| *h)
    }

    fn b(&self) -> &dyn Backend {
        &self.backend
    }

    fn kind_of(&self, h: u64) -> u32 {
        self.b().handle_info(h).map(|i| i.kind).unwrap_or(0)
    }

    pub fn create(&self, tag: &str) -> u64 {
        let k = self.b().resolve(NameKind::Kind, 0, tag);
        assert!(k != 0, "unknown tag <{tag}>");
        self.b().create(k).unwrap_or_else(|e| panic!("create <{tag}>: {e}"))
    }

    pub fn destroy(&self, h: u64) {
        self.b().destroy(h);
        self.backend.drain();
    }

    pub fn alive(&self, h: u64) -> bool {
        self.b().handle_info(h).is_ok()
    }

    pub fn lifetime(&self, h: u64) -> u32 {
        self.b().handle_info(h).map(|i| i.lifetime).unwrap_or(0)
    }

    fn prop(&self, h: u64, name: &str) -> Result<u32, String> {
        match self.b().resolve(NameKind::Prop, self.kind_of(h), name) {
            0 => Err(format!("unknown prop {name}")),
            id => Ok(id),
        }
    }

    pub fn set(&self, h: u64, name: &str, v: Value) -> Result<(), String> {
        let p = self.prop(h, name)?;
        self.b().set_prop(h, p, &v)
    }

    pub fn get(&self, h: u64, name: &str) -> Result<Value, String> {
        let p = self.prop(h, name)?;
        self.b().get_prop(h, p)
    }

    pub fn insert(&self, parent: u64, child: u64, before: Option<u64>, slot: Option<&str>) -> Result<(), String> {
        let s = match slot {
            Some(n) => match self.b().resolve(NameKind::Slot, self.kind_of(parent), n) {
                0 => return Err(format!("unknown slot {n}")),
                id => id,
            },
            None => 0,
        };
        self.b().insert(parent, child, before, s)
    }

    pub fn remove(&self, parent: u64, child: u64) -> Result<(), String> {
        self.b().remove(parent, child)
    }

    pub fn listen(&self, h: u64, signal: &str, on: bool) -> Result<(), String> {
        match self.b().resolve(NameKind::Signal, self.kind_of(h), signal) {
            0 => Err(format!("unknown signal {signal}")),
            id => self.b().listen(h, id, on),
        }
    }

    pub fn call(&self, h: u64, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match self.b().resolve(NameKind::Method, self.kind_of(h), method) {
            0 => Err(format!("unknown method {method}")),
            id => self.b().call(h, id, &args),
        }
    }

    /// JS names of the readable table props of a tag.
    pub fn readable_props(&self, tag: &str) -> Vec<&'static str> {
        lookup::readable_props(lookup::kind_by_tag(tag))
    }

    fn native(&self, h: u64) -> Retained<AnyObject> {
        self.backend.object(h).unwrap_or_else(|| panic!("handle {h} has no live object"))
    }

    pub fn obj(&self, h: u64) -> ObjRef {
        oref(&self.native(h))
    }

    pub fn is_kind(&self, h: u64, class: &str) -> bool {
        let cls = svelters_apple::objc::bridge::class_named(class).expect("class");
        svelters_apple::objc::bridge::is_kind(&self.native(h), cls)
    }

    /// The objects of an `NSArray` property (`arrangedSubviews`), by identity.
    pub fn native_list(&self, h: u64, key: &str) -> Vec<ObjRef> {
        let list = svelters_apple::objc::bridge::kvc_get(&self.native(h), key).expect("kvc").expect("list");
        let n: usize = unsafe { msg_send![&*list, count] };
        (0..n)
            .map(|i| {
                let o: Retained<AnyObject> = unsafe { msg_send![&*list, objectAtIndex: i] };
                oref(&o)
            })
            .collect()
    }

    pub fn native_get(&self, h: u64, key: &str) -> Option<ObjRef> {
        svelters_apple::objc::bridge::kvc_get(&self.native(h), key).expect("kvc").map(|o| oref(&o))
    }

    pub fn superview(&self, h: u64) -> Option<ObjRef> {
        let s: Option<Retained<AnyObject>> = unsafe { msg_send![&*self.native(h), superview] };
        s.map(|o| oref(&o))
    }

    pub fn is_visible(&self, h: u64) -> bool {
        unsafe { msg_send![&*self.native(h), isVisible] }
    }

    /// Recorded `(handle, signal name, args)` since the last call (posted and sync).
    pub fn take_events(&self) -> Vec<(u64, String, Vec<Value>)> {
        std::mem::take(&mut *self.rec.events.borrow_mut())
    }

    pub fn take_gone(&self) -> Vec<(u64, u32)> {
        std::mem::take(&mut *self.rec.gone.borrow_mut())
    }

    pub fn frames(&self) -> u32 {
        *self.rec.frames.borrow()
    }

    /// Checkpoints the backend asked for so far (wake source, timer).
    pub fn checkpoints(&self) -> u32 {
        *self.rec.checkpoints.borrow()
    }

    /// The backend as the host sees it.
    pub fn backend_trait(&self) -> &dyn Backend {
        &self.backend
    }

    /// Runs `[NSApp run]` through the ABI `run` with the recording host: `schedule_wake(after_ms)`
    /// arms a checkpoint, the checkpoint calls `quit`, and `run` must return.
    pub fn run_until_quit(&self, after_ms: i64) -> Result<(), String> {
        let b = self.backend.clone();
        *self.rec.on_checkpoint.borrow_mut() = Some(Box::new(move || Backend::quit(&b)));
        self.b().schedule_wake(after_ms);
        self.b().run(&self.host.host_ref())
    }

    /// What the recording host answers to `call_sync`.
    pub fn set_sync_reply(&self, v: Value) {
        *self.rec.reply.borrow_mut() = v;
    }

    /// `[control sendAction:control.action to:control.target]`: what a user gesture does.
    pub fn send_action(&self, h: u64) {
        let o = self.native(h);
        unsafe {
            let action: Option<Sel> = msg_send![&*o, action];
            let target: Option<Retained<AnyObject>> = msg_send![&*o, target];
            let _: bool = msg_send![&*o, sendAction: action, to: target.as_deref()];
        }
    }

    pub fn perform_click(&self, h: u64) {
        let _: () = unsafe { msg_send![&*self.native(h), performClick: Option::<&AnyObject>::None] };
    }

    pub fn post_notification(&self, h: u64, name: &str) {
        let o = self.native(h);
        let n = NSString::from_str(name);
        unsafe {
            let center: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
            let _: () = msg_send![&*center, postNotificationName: &*n, object: &*o];
        }
    }

    /// The close button: asks `windowShouldClose:` first.
    pub fn perform_close(&self, h: u64) {
        let _: () = unsafe { msg_send![&*self.native(h), performClose: Option::<&AnyObject>::None] };
    }

    /// `-[NSWindow close]`: closes without asking.
    pub fn close_window(&self, h: u64) {
        let _: () = unsafe { msg_send![&*self.native(h), close] };
    }

    /// Dispatches events and run-loop sources for about `ms` milliseconds, then releases dead natives.
    pub fn pump(&self, ms: u64) {
        pump(ms);
        self.backend.drain();
    }
}

/// Dispatches AppKit events and run-loop sources (our wake source and timers fire in it) for about
/// `ms` milliseconds, like `[NSApp run]` would, without needing `run`.
pub fn pump(ms: u64) {
    unsafe {
        let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
        let until: Retained<AnyObject> = msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: ms as f64 / 1000.0];
        let mode = NSString::from_str("kCFRunLoopDefaultMode");
        loop {
            let ev: Option<Retained<AnyObject>> =
                msg_send![&*app, nextEventMatchingMask: usize::MAX, untilDate: &*until, inMode: &*mode, dequeue: true];
            match ev {
                Some(e) => {
                    let _: () = msg_send![&*app, sendEvent: &*e];
                }
                None => break,
            }
        }
    }
}
