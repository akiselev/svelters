//! The Apple object core for non-GUI types (plans/research/NON-GUI-TYPES.md §2.2 "Apple", milestone N4;
//! design notes in plans/milestones/N4-appkit-types.md). Hand-written, small, and the only native code of
//! the type layer: the generated part is data (`generated/types.rs`) and JS (`packages/types/appkit`).
//!
//! - **Kinds** are ObjC classes, named by their class name (`resolve(SVL_KIND, "NSURL")`), with ids from
//!   `TYPE_KIND_BASE`. An object's kind is the nearest *bound* class along its class chain, so a class
//!   cluster's private subclass (`__NSCFString`, `NSComponentColor`) reports the public class.
//!   `describe(SVL_KIND, 0, k)` names the class and its superclass's kind (the host's `typeChain`).
//! - **Lifetime** (decision Q1): every non-widget object crosses as a `SHARED` handle that holds one
//!   strong reference (retain/release); one handle per object (identity through `by_ptr`, safe because the
//!   reference keeps the pointer from being reused). Value classes (immutable) use the same handle: sharing
//!   an immutable object is indistinguishable from copying it. Widget objects keep X1's BORROWED handles.
//! - **Members** are reached reflectively by selector: properties by KVC (`get_prop`/`set_prop` resolve the
//!   KVC key in the runtime), methods and class methods by `NSInvocation` (`bridge::invoke_ex`), so the JS
//!   module passes the selector as the method name. `init…` through `call_static` is alloc + init; results of
//!   the new/copy/init families are +1.
//! - **Errors**: a trailing `NSError **` (a selector ending in `error:`) is hidden from JS; a failure rejects
//!   or throws with a `NativeError` (domain, code, localizedDescription).
//! - **Async**: a method listed in `generated::types::ASYNC` gets a completion block of its shape
//!   (`svelters_apple::blocks`); the block may run on any thread, so it only queues its retained arguments
//!   and wakes the run loop; the main thread converts them and completes the host's one-shot callback
//!   (the NT1 model). An `NSProgress` result is kept so `cancel` can cancel the operation; the owner's
//!   destruction drops its pending operations (the host rejects them with `disposed`).

use super::*;
use crate::types_table::{self as tt, TYPE_KIND_BASE};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use svelters_apple::blocks;
use svelters_apple::objc::bridge::{Failed, Invoke};

/// The completion callback type of async members (`SVL_M_ASYNC`): one result value or an `ERROR`.
pub(super) const T_ASYNC_DONE: u32 = SVL_T_FIRST_BACKEND + 0x00FF_0000;

/// A registered type kind.
pub(super) struct TypeKind {
    pub name: String,
    pub class: &'static AnyClass,
    /// The superclass's kind (0 at the root).
    pub parent: u32,
}

#[derive(Default)]
pub(super) struct TypeKinds {
    list: Vec<TypeKind>,
    by_name: HashMap<String, u32>,
}

/// An async operation in flight, by its completion callback id.
pub(super) struct Pending {
    owner: u64,
    settled: Arc<AtomicBool>,
    shape: &'static str,
    progress: Option<Retained<AnyObject>>,
}

/// Completions queued by blocks (any thread), delivered on the main thread by [`Inner::type_hops`].
static COMPLETIONS: Mutex<Vec<(u64, blocks::Args)>> = Mutex::new(Vec::new());

fn widget_class(cls: &AnyClass) -> bool {
    bridge::class_chain(cls).iter().any(|c| {
        let n = c.name().to_string_lossy();
        n == "NSResponder" || n == "NSCell" || lookup::kind_by_class(&n) != 0
    })
}

fn class_object(cls: &AnyClass) -> &AnyObject {
    // SAFETY: a class is an object (the receiver of class methods).
    unsafe { &*(cls as *const AnyClass as *const AnyObject) }
}

fn fail(e: Failed) -> Result<Value, String> {
    match e {
        Failed::Msg(m) => Err(m),
        Failed::Native(n) => Ok(Value::Error(Box::new(n))),
    }
}

impl Inner {
    // ---- kinds ----

    fn register_class(&self, cls: &'static AnyClass) -> u32 {
        let name = cls.name().to_string_lossy().into_owned();
        if let Some(k) = self.type_kinds.borrow().by_name.get(&name) {
            return *k;
        }
        let parent = cls.superclass().map_or(0, |s| self.register_class(s));
        let mut tk = self.type_kinds.borrow_mut();
        tk.list.push(TypeKind { name: name.clone(), class: cls, parent });
        let id = TYPE_KIND_BASE + tk.list.len() as u32 - 1;
        tk.by_name.insert(name, id);
        id
    }

    /// `resolve(SVL_KIND, name)` for an ObjC class name (0 when no such class is loaded).
    pub(super) fn type_kind_by_name(&self, name: &str) -> u32 {
        if let Some(k) = self.type_kinds.borrow().by_name.get(name) {
            return *k;
        }
        match bridge::class_named(name) {
            Some(c) => self.register_class(c),
            None => 0,
        }
    }

    pub(super) fn type_kind(&self, kind: u32) -> Option<(String, &'static AnyClass, u32)> {
        let tk = self.type_kinds.borrow();
        tk.list.get(kind.checked_sub(TYPE_KIND_BASE)? as usize).map(|t| (t.name.clone(), t.class, t.parent))
    }

    /// `kind` and its superclasses' kinds, most derived first.
    pub(super) fn type_chain(&self, kind: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut k = kind;
        while let Some((_, _, p)) = self.type_kind(k) {
            out.push(k);
            k = p;
        }
        out
    }

    /// The kind of a native object: its nearest bound class (the facts' `TYPE_CLASSES`), else its own class.
    fn kind_of_object(&self, obj: &AnyObject) -> u32 {
        let chain = bridge::class_chain(obj.class());
        let bound = chain.iter().find(|c| tt::type_class(&c.name().to_string_lossy()).is_some());
        self.register_class(bound.copied().unwrap_or(chain[0]))
    }

    /// The handle of a non-widget object read from native code (`None` for widgets: they stay BORROWED).
    pub(super) fn shared_handle(&self, obj: &AnyObject) -> Option<u64> {
        if widget_class(obj.class()) {
            return None;
        }
        let kind = self.kind_of_object(obj);
        // SAFETY: `obj` is a live object handed to us by native code; retain it (SHARED holds a strong ref).
        let strong = unsafe { Retained::retain(obj as *const AnyObject as *mut AnyObject) }?;
        Some(self.mint_shared(strong, kind))
    }

    fn mint_shared(&self, obj: Retained<AnyObject>, kind: u32) -> u64 {
        let ptr = Retained::as_ptr(&obj) as usize;
        if let Some(h) = self.by_ptr.borrow().get(&ptr) {
            return *h;
        }
        let h = self.table.borrow_mut().mint(Native::strong(obj), kind, SVL_LIFE_SHARED);
        self.by_ptr.borrow_mut().insert(ptr, h);
        h
    }

    /// `resolve(SVL_METHOD, kind, name)` on a type kind: `name` is a selector the class or its instances
    /// respond to (the generated modules pass selectors), or a JS-style name found by `find_selector`.
    pub(super) fn type_method(&self, kind: u32, name: &str) -> u32 {
        if let Some(i) = self.dyn_methods.borrow().iter().position(|(k, n, _)| *k == kind && n == name) {
            return DYN_BASE + i as u32;
        }
        let Some((_, cls, _)) = self.type_kind(kind) else { return 0 };
        let s = bridge::sel(name);
        let found = if cls.instance_method(s).is_some() || bridge::responds(class_object(cls), s) {
            Some(s)
        } else {
            bridge::find_selector(cls, name)
        };
        let Some(s) = found else { return 0 };
        let mut d = self.dyn_methods.borrow_mut();
        d.push((kind, name.to_string(), s));
        DYN_BASE + d.len() as u32 - 1
    }

    fn type_selector(&self, kind: u32, method: u32) -> Result<Sel, String> {
        let d = self.dyn_methods.borrow();
        let (k, _, s) = d.get(method.checked_sub(DYN_BASE).ok_or("unknown method id")? as usize).ok_or("unknown method id")?;
        if *k != kind && !self.type_chain(kind).contains(k) {
            return Err("method does not apply to this kind".into());
        }
        Ok(*s)
    }

    /// The async member behind selector `s` on `cls` or a superclass.
    fn async_of(cls: &AnyClass, s: Sel, is_static: bool) -> Option<&'static tt::AsyncMember> {
        let sel = s.name().to_string_lossy();
        bridge::class_chain(cls).iter().find_map(|c| tt::async_member(&c.name().to_string_lossy(), &sel, is_static))
    }

    /// `describe(SVL_METHOD, kind, m)` on a type kind: async members only (everything else converts
    /// dynamically against the runtime method signature).
    pub(super) fn type_method_desc(&self, kind: u32, method: u32) -> Option<MemberDesc> {
        let (_, cls, _) = self.type_kind(kind)?;
        let s = self.type_selector(kind, method).ok()?;
        let (a, is_static) = match Self::async_of(cls, s, false) {
            Some(a) => (a, false),
            None => (Self::async_of(cls, s, true)?, true),
        };
        let mut params: Vec<SlotDesc> = (0..a.args).map(|i| SlotDesc::new(SVL_NULL).named(&format!("arg{i}"))).collect();
        params.push(SlotDesc::new(SVL_CALLBACK).typed(T_ASYNC_DONE).named("done"));
        Some(MemberDesc {
            name: a.sel.to_string(),
            params,
            ret: SlotDesc::new(SVL_NULL),
            flags: SVL_M_ASYNC | if is_static { SVL_M_STATIC } else { 0 },
        })
    }

    // ---- ops ----

    /// ABI v2.5 `construct`: `[[cls alloc] init]`, then the construct-time properties by KVC.
    pub(super) fn type_construct(&self, kind: u32, props: &[u32], values: &[Value]) -> Result<Value, String> {
        let (name, cls, _) = self.type_kind(kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        if widget_class(cls) {
            return Err(format!("{name} is a widget: create it as an element"));
        }
        let obj: Option<Retained<AnyObject>> = bridge::catch(&name, || unsafe { msg_send![cls, new] })?;
        let obj = obj.ok_or_else(|| format!("[{name} new] returned nil"))?;
        let h = self.mint_shared(obj, kind);
        for (p, v) in props.iter().zip(values) {
            if let Err(e) = self.set_prop_raw(h, *p, v) {
                self.destroy(h);
                return Err(e);
            }
        }
        Ok(Value::Handle(h))
    }

    /// ABI v2.5 `call_static`: a class method by selector, or `init…` (alloc + init).
    pub(super) fn type_call_static(&self, kind: u32, method: u32, args: &[Value]) -> Result<Value, String> {
        let (name, cls, _) = self.type_kind(kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let s = self.type_selector(kind, method)?;
        let sname = s.name().to_string_lossy().into_owned();
        let error_out = sname.ends_with("error:");
        let prev = self.cur_owner.replace(0);
        let r = if sname.starts_with("init") {
            if !cls.instance_method(s).is_some() {
                Err(format!("{name} has no initializer {sname}"))
            } else {
                // SAFETY: `+alloc` returns a +1 object, which the initializer consumes (invoke_ex `init`).
                let a: *mut AnyObject = unsafe { MessageReceiver::send_message(cls, sel!(alloc), ()) };
                match unsafe { a.as_ref() } {
                    None => Err(format!("[{name} alloc] returned nil")),
                    Some(a) => match bridge::invoke_ex(a, s, args, self, &Invoke { init: true, error_out, ..Default::default() }) {
                        Ok(o) if o.obj.is_none() => Err(format!("[{name} {sname}] returned nil")),
                        Ok(o) => Ok(o.value),
                        Err(e) => fail(e),
                    },
                }
            }
        } else if let Some(a) = Self::async_of(cls, s, true) {
            self.start_async(0, class_object(cls), s, a, args)
        } else {
            match bridge::invoke_ex(class_object(cls), s, args, self, &Invoke { error_out, ..Default::default() }) {
                Ok(o) => Ok(o.value),
                Err(e) => fail(e),
            }
        };
        self.cur_owner.set(prev);
        r
    }

    /// `call` on a handle of a type kind.
    pub(super) fn type_call(&self, h: u64, kind: u32, method: u32, args: &[Value]) -> Result<Value, String> {
        let s = self.type_selector(kind, method)?;
        let obj = self.live(h)?;
        let sname = s.name().to_string_lossy().into_owned();
        if sname.starts_with("init") || names_owning_alloc(&sname) {
            return Err(format!("{sname}: initializers are called through the class (call_static)"));
        }
        let prev = self.cur_owner.replace(h);
        let r = if let Some(a) = Self::async_of(obj.class(), s, false) {
            self.start_async(h, &obj, s, a, args)
        } else {
            match bridge::invoke_ex(&obj, s, args, self, &Invoke { error_out: sname.ends_with("error:"), ..Default::default() }) {
                Ok(o) => Ok(o.value),
                Err(e) => fail(e),
            }
        };
        self.cur_owner.set(prev);
        r
    }

    // ---- async ----

    fn start_async(&self, owner: u64, target: &AnyObject, s: Sel, a: &'static tt::AsyncMember, args: &[Value]) -> Result<Value, String> {
        let (last, rest) = args.split_last().ok_or("missing completion callback")?;
        let Value::Callback { id, .. } = last else {
            return Err("the last argument of an async method must be the completion callback".into());
        };
        let id = *id;
        let settled = Arc::new(AtomicBool::new(false));
        let s2 = settled.clone();
        let deliver: blocks::Deliver = Arc::new(move |args: blocks::Args| {
            // First invocation wins: a block called twice, or after a cancel, is ignored.
            if !s2.swap(true, Ordering::SeqCst) {
                COMPLETIONS.lock().unwrap_or_else(|e| e.into_inner()).push((id, args));
                observe::wake_loop();
            }
        });
        let block = blocks::completion(a.shape, deliver)?;
        // In place before the call: a block may complete on another thread before `invoke` returns.
        self.pending.borrow_mut().insert(id, Pending { owner, settled: settled.clone(), shape: a.shape, progress: None });
        let how = Invoke { block: Some((rest.len(), block.as_ptr())), ..Default::default() };
        match bridge::invoke_ex(target, s, rest, self, &how) {
            Ok(o) => {
                if let Some(p) = o.obj.filter(|p| bridge::is_kind(p, class!(NSProgress))) {
                    if let Some(e) = self.pending.borrow_mut().get_mut(&id) {
                        e.progress = Some(p);
                    }
                }
                drop(block);
                Ok(Value::Null)
            }
            Err(e) => {
                settled.store(true, Ordering::SeqCst);
                self.pending.borrow_mut().remove(&id);
                fail(e)
            }
        }
    }

    /// The host aborted the operation behind completion `id` (`{ signal }`): cancels an `NSProgress`
    /// result and rejects now with NSUserCancelledError; a completion that arrives later is dropped.
    pub(super) fn type_cancel(&self, id: u64) -> bool {
        let Some(p) = self.pending.borrow_mut().remove(&id) else { return false };
        if p.settled.swap(true, Ordering::SeqCst) {
            // The block already ran: its queued completion is delivered by the next hop (pending is gone,
            // so put the entry back for it).
            self.pending.borrow_mut().insert(id, p);
            return false;
        }
        if let Some(pr) = &p.progress {
            let _ = bridge::catch("cancel", || unsafe {
                let _: () = msg_send![&**pr, cancel];
            });
        }
        let e = NativeError { domain: "NSCocoaErrorDomain".into(), code: Value::I64(3072), message: "cancelled".into() };
        self.complete(id, Value::Error(Box::new(e)));
        true
    }

    /// The owner of pending operations died: forget them (the host rejects them with `disposed`).
    pub(super) fn drop_pending_of(&self, owner: u64) {
        self.pending.borrow_mut().retain(|_, p| {
            if p.owner == owner && owner != 0 {
                p.settled.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        });
    }

    fn complete(&self, id: u64, v: Value) {
        if let Some(host) = self.host() {
            let raw = RawValues::new(std::slice::from_ref(&v));
            let mut out = svl_value::null();
            unsafe { (host.invoke_callback)(host.self_, id, raw.as_ptr(), raw.len(), &mut out) };
        }
    }

    /// On the main thread, from the run loop: completions queued by blocks.
    pub(super) fn type_hops(&self) {
        let done = std::mem::take(&mut *COMPLETIONS.lock().unwrap_or_else(|e| e.into_inner()));
        for (id, args) in done {
            let Some(p) = self.pending.borrow_mut().remove(&id) else { continue };
            let (err_at, res_at) = blocks::roles(p.shape);
            let prev = self.cur_owner.replace(p.owner);
            let err = err_at.and_then(|i| match args.0.get(i) {
                Some(blocks::BlockArg::Obj(Some(e))) => Some(bridge::native_error(e)),
                _ => None,
            });
            let v = match err {
                Some(e) => Value::Error(Box::new(e)),
                None => match res_at.and_then(|i| args.0.get(i)) {
                    Some(blocks::BlockArg::Obj(o)) => bridge::object_out(o.as_deref(), self),
                    Some(blocks::BlockArg::Bool(b)) => Value::Bool(*b),
                    Some(blocks::BlockArg::Int(i)) => Value::I64(*i),
                    Some(blocks::BlockArg::F64(f)) => Value::F64(*f),
                    None => Value::Null,
                },
            };
            self.cur_owner.set(prev);
            self.complete(id, v);
            drop(args);
        }
    }

    /// Async operations in flight (tests).
    pub(super) fn pending_async(&self) -> usize {
        self.pending.borrow().len()
    }
}

/// `alloc`/`allocWithZone:` are never callable on an instance.
fn names_owning_alloc(sel: &str) -> bool {
    sel == "alloc" || sel == "allocWithZone:"
}

/// The descriptor of [`T_ASYNC_DONE`].
pub(super) fn async_done_desc() -> TypeDesc {
    TypeDesc {
        name: "(result: unknown) => void".into(),
        kind: SVL_CALLBACK,
        scope: SVL_CB_ASYNC,
        members: vec![],
        fields: vec![SlotDesc::new(SVL_NULL).named("result")],
        ret: SlotDesc::new(SVL_NULL),
    }
}
