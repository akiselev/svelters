//! Test harness for the AppKit backend (macOS): the generated verification tests
//! (`examples/counter-mac/tests/generated.rs`) and the hand-written ones drive the backend through
//! its `Backend` implementation with a recording host, on the main thread, without `[NSApp run]`.

use crate::AppKitBackend;

/// Helpers of the non-GUI type tests (N4): `examples/counter-mac/tests/types.rs` (generated) and `types_core.rs`.
#[path = "testing_types.rs"]
pub mod types;
use crate::lookup;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
pub use svelters_abi::ObserveError;

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
    /// ABI v2.6 `prop_changed` calls, in order.
    changed: RefCell<Vec<(u64, u32)>>,
    /// `invoke_callback` calls (completions of async type members, N4), in order.
    completions: RefCell<Vec<(u64, Vec<Value>)>>,
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
    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.0.changed.borrow_mut().push((h, prop));
    }
    fn invoke_callback(&self, callback: u64, args: &[Value]) -> StrResult<Value> {
        self.0.completions.borrow_mut().push((callback, args.to_vec()));
        Ok(Value::Null)
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
    /// `(tag, prop, value)` set right after every create of the tag (the overrides' `test_setup`).
    setup: RefCell<&'static [(&'static str, &'static str, S)]>,
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
            changed: RefCell::new(Vec::new()),
            completions: RefCell::new(Vec::new()),
        });
        let host: &'static HostVtable = Box::leak(Box::new(HostVtable::new(TestHost(rec.clone()))));
        backend.attach_host(host.as_raw());
        Box::leak(Box::new(Harness { backend, rec, host, setup: RefCell::new(&[]) }))
    }

    /// Props to set right after every create of a tag (generated from the overrides' `test_setup`: a
    /// pop-up-button gets items, so an index can be selected).
    pub fn set_setup(&self, setup: &'static [(&'static str, &'static str, S)]) {
        *self.setup.borrow_mut() = setup;
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

    /// Completions the backend delivered (`invoke_callback`), drained.
    pub fn take_completions(&self) -> Vec<(u64, Vec<Value>)> {
        std::mem::take(&mut *self.rec.completions.borrow_mut())
    }

    fn b(&self) -> &dyn Backend {
        &self.backend
    }

    pub fn kind_of(&self, h: u64) -> u32 {
        self.b().handle_info(h).map(|i| i.kind).unwrap_or(0)
    }

    pub fn create(&self, tag: &str) -> u64 {
        let k = self.b().resolve(NameKind::Kind, 0, tag);
        assert!(k != 0, "unknown tag <{tag}>");
        let h = self.b().create(k).unwrap_or_else(|e| panic!("create <{tag}>: {e}"));
        let setup = *self.setup.borrow();
        for (t, prop, v) in setup.iter().filter(|s| s.0 == tag) {
            self.set(h, prop, v.v()).unwrap_or_else(|e| panic!("test setup <{t}> {prop}: {e}"));
        }
        h
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

    /// Whether the native object answers `selector` (members newer than the running macOS do not).
    pub fn responds(&self, h: u64, selector: &str) -> bool {
        svelters_apple::objc::bridge::responds(&self.native(h), svelters_apple::objc::bridge::sel(selector))
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
        if !svelters_apple::objc::bridge::responds(&o, objc2::sel!(sendAction:to:)) {
            // Not a control (a menu item): the application sends the action.
            return self.app_send_action(h);
        }
        unsafe {
            let action: Option<Sel> = msg_send![&*o, action];
            let target: Option<Retained<AnyObject>> = msg_send![&*o, target];
            let _: bool = msg_send![&*o, sendAction: action, to: target.as_deref()];
        }
    }

    /// `[NSApp sendAction:item.action to:item.target from:item]`: what picking a menu item does (NSMenuItem
    /// has no -sendAction:to:).
    pub fn app_send_action(&self, h: u64) {
        let o = self.native(h);
        unsafe {
            let action: Option<Sel> = msg_send![&*o, action];
            let target: Option<Retained<AnyObject>> = msg_send![&*o, target];
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let _: bool = msg_send![&*app, sendAction: action, to: target.as_deref(), from: &*o];
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

    // ---- observe (ABI v2.6, milestone R4) ----

    /// The prop id `name` resolves to on `h`.
    pub fn prop_id(&self, h: u64, name: &str) -> u32 {
        self.prop(h, name).unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn observe(&self, h: u64, name: &str, on: bool) -> Result<(), ObserveError> {
        let p = self.prop(h, name).map_err(ObserveError::Failed)?;
        self.b().observe(h, p, on)
    }

    /// A write that is not a host op (no echo marker): what native code changing the prop does.
    pub fn set_natively(&self, h: u64, name: &str, v: Value) -> Result<(), String> {
        let p = self.prop(h, name)?;
        self.backend.set_prop_natively(h, p, &v)
    }

    /// `prop_changed` calls for (h, prop) since the last call (all recorded calls are taken).
    pub fn take_changed(&self, h: u64, prop: u32) -> usize {
        let all = std::mem::take(&mut *self.rec.changed.borrow_mut());
        all.into_iter().filter(|c| *c == (h, prop)).count()
    }

    /// The native object of a live handle, retained (tests that outlive the handle).
    pub fn retain(&self, h: u64) -> Retained<AnyObject> {
        self.native(h)
    }

    /// `-[obj observationInfo]`: NULL when no KVO observer is registered on the object.
    pub fn observation_info(obj: &AnyObject) -> usize {
        let p: *mut std::ffi::c_void = unsafe { msg_send![obj, observationInfo] };
        p as usize
    }

    /// Puts `child` in a new window as its content view and shows the window (key if the app can be active).
    pub fn mount(&self, child: u64) -> u64 {
        let w = self.create("window");
        self.insert(w, child, None, Some("content-view")).unwrap_or_else(|e| panic!("mount: {e}"));
        self.call(w, "make-key-and-order-front", vec![]).unwrap_or_else(|e| panic!("make-key-and-order-front: {e}"));
        self.pump(30);
        w
    }

    /// Makes `h` the first responder of its window (a text field hands over to its field editor).
    pub fn focus(&self, h: u64) -> bool {
        let o = self.native(h);
        unsafe {
            let w: Option<Retained<AnyObject>> = msg_send![&*o, window];
            let w = w.expect("the element is in a window");
            msg_send![&*w, makeFirstResponder: &*o]
        }
    }

    /// Key-down events through `-[NSWindow sendEvent:]` to the first responder of `h`'s window: what a user
    /// typing does (the field editor inserts the text and posts NSControlTextDidChangeNotification).
    pub fn type_keys(&self, h: u64, text: &str, key_code: u16) {
        let o = self.native(h);
        unsafe {
            let w: Option<Retained<AnyObject>> = msg_send![&*o, window];
            let w = w.expect("the element is in a window");
            let num: isize = msg_send![&*w, windowNumber];
            for ch in text.chars() {
                let s = NSString::from_str(&ch.to_string());
                for ty in [NS_KEY_DOWN, NS_KEY_UP] {
                    let ev: Option<Retained<AnyObject>> = msg_send![
                        class!(NSEvent),
                        keyEventWithType: ty,
                        location: NsPoint { x: 0.0, y: 0.0 },
                        modifierFlags: 0usize,
                        timestamp: 0.0f64,
                        windowNumber: num,
                        context: Option::<&AnyObject>::None,
                        characters: &*s,
                        charactersIgnoringModifiers: &*s,
                        isARepeat: false,
                        keyCode: key_code
                    ];
                    let ev = ev.expect("NSEvent keyEventWithType");
                    let _: () = msg_send![&*w, sendEvent: &*ev];
                }
            }
        }
    }

    /// `-[NSWindow setFrame:display:]` with a frame that differs from the current one in size and origin.
    pub fn move_window(&self, w: u64, n: usize) {
        let o = self.native(w);
        let d = 10.0 * (n % 5) as f64;
        let r = NsRect { x: 100.0 + d, y: 100.0 + d, w: 400.0 + d, h: 300.0 + d };
        let _: () = unsafe { msg_send![&*o, setFrame: r, display: true] };
    }

    /// Dispatches events and run-loop sources for about `ms` milliseconds, then releases dead natives.
    pub fn pump(&self, ms: u64) {
        pump(ms);
        self.backend.drain();
    }
}

const NS_KEY_DOWN: usize = 10;
const NS_KEY_UP: usize = 11;
/// macOS virtual key code of the right arrow (kVK_RightArrow).
pub const KEY_RIGHT: u16 = 124;

#[repr(C)]
#[derive(Clone, Copy)]
struct NsPoint {
    x: f64,
    y: f64,
}

unsafe impl objc2::encode::Encode for NsPoint {
    const ENCODING: objc2::encode::Encoding =
        objc2::encode::Encoding::Struct("CGPoint", &[objc2::encode::Encoding::Double, objc2::encode::Encoding::Double]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

unsafe impl objc2::encode::Encode for NsRect {
    const ENCODING: objc2::encode::Encoding = objc2::encode::Encoding::Struct(
        "CGRect",
        &[
            objc2::encode::Encoding::Struct("CGPoint", &[objc2::encode::Encoding::Double, objc2::encode::Encoding::Double]),
            objc2::encode::Encoding::Struct("CGSize", &[objc2::encode::Encoding::Double, objc2::encode::Encoding::Double]),
        ],
    );
}

// ---- observe rows (milestone R4; plans/research/REACTIVITY.md 4.5) ----

/// How an observe row checks its fact (`tools/metadata/appkit/gen_backend.py` picks it from the fact).
#[derive(Clone, Copy, Debug)]
pub enum ObsCheck {
    /// Set the sample natively (KVC, not the host op): announced; then identical set, echo, coalescing, teardown.
    Change,
    /// A native change that is not a write of the prop (`verify.trigger`).
    Cross(Trig),
    /// The mechanisms install and uninstall; nothing triggers them here.
    Connect,
    /// `observe` answers SVL_OBSERVE_UNSUPPORTED (class none or set_only).
    Unsupported,
    /// `observe` answers -1 (constant).
    Refused,
}

#[derive(Clone, Copy, Debug)]
pub enum Trig {
    /// `-[NSWindow setFrame:display:]` (posts DidResize / DidMove).
    SetFrame,
}

/// The user path of a fallback mechanism, where the runner can synthesize it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum User {
    None,
    /// Typing into the focused field through `-[NSWindow sendEvent:]` (NSControlTextDidChangeNotification).
    Type,
    /// `-[NSButton performClick:]` (the action).
    Click,
    /// The right-arrow key through `-[NSWindow sendEvent:]` to the focused control (the action).
    Key,
}

pub struct ObserveRow {
    pub tag: &'static str,
    pub prop: &'static str,
    /// The native KVC key; with a KVO mechanism the row checks the registration is gone before the object is.
    pub kvo: &'static str,
    pub check: ObsCheck,
    pub sample: Option<S>,
    pub user: User,
    /// The prop is new: a class without this selector (an older macOS) skips the row.
    pub requires: &'static str,
}

/// One observe fact through the real ABI v2.6 path (REACTIVITY.md 4.5, backend level):
/// 1. native change (KVC set, or the trigger): announced (`prop_changed(h, prop)`) synchronously on the main thread;
/// 2. identical set: logged (KVO re-announces it; the host's compare drops it);
/// 3. echo: a set through `set_prop` (the host op) is announced unless proportional (backend suppression);
/// 4. coalescing input: 10 native changes are announced at least once (more is logged; the host coalesces);
/// 5. user path of the fallback (typing, click, arrow key): announced;
/// 6. teardown: nested on/off, nothing after `observe(off)`, and destroying the handle while observed removes the KVO
///    registration before our reference is released (`observationInfo` is NULL on the object we keep).
pub fn observe_row(r: &ObserveRow) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let o = h.create(r.tag);
    if !r.requires.is_empty() && !h.responds(o, r.requires) {
        eprintln!("skipped: {} has no {} on this macOS", r.tag, r.requires);
        h.destroy(o);
        return;
    }
    let pid = h.prop_id(o, r.prop);
    match r.check {
        ObsCheck::Unsupported | ObsCheck::Refused => {
            let got = h.observe(o, r.prop, true);
            h.destroy(o);
            match (r.check, got) {
                (ObsCheck::Unsupported, Err(ObserveError::Unsupported(_))) | (ObsCheck::Refused, Err(ObserveError::Failed(_))) => return,
                (c, g) => panic!("<{}> {}: observe answered {g:?}, want {c:?}", r.tag, r.prop),
            }
        }
        _ => {}
    }
    let window = match (r.check, r.user) {
        (ObsCheck::Cross(Trig::SetFrame), _) => None,
        (_, User::None) => None,
        _ => Some(h.mount(o)),
    };
    let fresh = Harness::observation_info(&h.retain(o)) == 0;
    h.observe(o, r.prop, true).unwrap_or_else(|e| panic!("<{}> {}: observe: {e:?}", r.tag, r.prop));
    assert!(h.backend.is_observed(o, pid));
    h.take_changed(o, pid);
    let sample = r.sample.map(|s| s.v());
    let first = h.get(o, r.prop).unwrap_or(Value::Null);
    let change = |n: usize| match r.check {
        ObsCheck::Change => {
            let v = if n % 2 == 0 { sample.clone().unwrap() } else { first.clone() };
            h.set_natively(o, r.prop, v).unwrap_or_else(|e| panic!("<{}> {} native set: {e}", r.tag, r.prop));
        }
        ObsCheck::Cross(Trig::SetFrame) => h.move_window(o, n + 1),
        _ => {}
    };
    let moves = !matches!(r.check, ObsCheck::Connect);
    if moves {
        // 1. one native change
        change(0);
        let n1 = h.take_changed(o, pid);
        assert!(n1 >= 1, "<{}> {}: a native change was not announced", r.tag, r.prop);
        // 2. identical set (KVO re-announces: decided by the host compare)
        if let ObsCheck::Change = r.check {
            let cur = h.get(o, r.prop).unwrap();
            h.set_natively(o, r.prop, cur).unwrap();
            println!("observe <{}> {}: identical set announced {} time(s)", r.tag, r.prop, h.take_changed(o, pid));
            // 3. echo: our own write
            h.set(o, r.prop, first.clone()).unwrap();
            let n3 = h.take_changed(o, pid);
            let proportional = svelters_appkit_proportional(o, r.prop);
            if proportional {
                assert_eq!(n3, 0, "<{}> {}: our own write of a proportional prop was announced", r.tag, r.prop);
            } else {
                assert!(n3 >= 1, "<{}> {}: our own write was not announced (the host compare drops it, not the backend)", r.tag, r.prop);
            }
        }
        // 4. coalescing input
        for n in 0..10 {
            change(n + 1);
        }
        h.pump(10);
        let n4 = h.take_changed(o, pid);
        // More than one per change is AppKit changing the prop itself meanwhile (NSWindow `views-need-display`
        // around the display cycle): delivered, then coalesced by the host.
        assert!(n4 >= 1, "<{}> {}: 10 native changes were not announced", r.tag, r.prop);
        if n4 > 10 {
            println!("observe <{}> {}: 10 native changes announced {n4} times", r.tag, r.prop);
        }
    }
    // 5. the user path of the fallback
    match r.user {
        User::None => {}
        User::Type => {
            assert!(h.focus(o), "<{}>: the field did not become first responder", r.tag);
            h.take_changed(o, pid);
            h.type_keys(o, "abc", 0);
            h.pump(10);
            let n5 = h.take_changed(o, pid);
            assert!(n5 >= 1, "<{}> {}: typing was not announced", r.tag, r.prop);
        }
        User::Click => {
            h.take_changed(o, pid);
            h.perform_click(o);
            let n5 = h.take_changed(o, pid);
            assert!(n5 >= 1, "<{}> {}: a click was not announced", r.tag, r.prop);
        }
        User::Key => {
            assert!(h.focus(o), "<{}>: the control did not become first responder", r.tag);
            h.take_changed(o, pid);
            h.type_keys(o, "\u{F703}", KEY_RIGHT);
            h.pump(10);
            let n5 = h.take_changed(o, pid);
            assert!(n5 >= 1, "<{}> {}: the arrow key was not announced", r.tag, r.prop);
        }
    }
    // 6. teardown
    h.observe(o, r.prop, true).unwrap();
    h.observe(o, r.prop, false).unwrap();
    assert!(h.backend.is_observed(o, pid), "<{}> {}: a nested off ended the observation", r.tag, r.prop);
    h.observe(o, r.prop, false).unwrap();
    assert!(!h.backend.is_observed(o, pid));
    let kept = h.retain(o);
    if fresh && !r.kvo.is_empty() {
        assert_eq!(Harness::observation_info(&kept), 0, "<{}> {}: the KVO registration survived observe(off)", r.tag, r.prop);
    }
    h.take_changed(o, pid);
    if moves {
        change(1);
        change(2);
    }
    assert_eq!(h.take_changed(o, pid), 0, "<{}> {}: announced after observe(off)", r.tag, r.prop);
    h.observe(o, r.prop, false).unwrap();
    h.observe(o, r.prop, true).unwrap();
    if let Some(w) = window {
        h.destroy(w);
    }
    h.destroy(o);
    // We still hold the object: the backend must have removed its KVO registration before releasing its reference.
    if fresh && !r.kvo.is_empty() {
        assert_eq!(Harness::observation_info(&kept), 0, "<{}> {}: the KVO observer was not removed before the handle released the object", r.tag, r.prop);
    }
    h.pump(5);
    assert_eq!(h.take_changed(o, pid), 0, "<{}> {}: announced after the handle was destroyed", r.tag, r.prop);
    drop(kept);
}

fn svelters_appkit_proportional(h: u64, prop: &str) -> bool {
    let hs = Harness::shared();
    let kind = hs.kind_of(h);
    lookup::observe_info(kind, prop).is_some_and(|o| o.proportional)
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
