//! The backend ops on type kinds and SHARED handles (Windows only): `resolve`, `describe`, `construct`,
//! `call_static`, `call`, `get_prop` / `set_prop`, `listen`, `observe`, `cancel`, and minting with the thread check.
//!
//! Lifetime (decision Q1/Q8): every runtime class object is a `SHARED` handle holding one strong reference, keyed by
//! its IUnknown identity in the same table as the elements (so an element passed back is its element handle).
//! `destroy` drops our reference, never `Close()`s (section 4.1 rule 6: `close()` is an explicit method).
//!
//! Threading (section 4.1): one UI thread. A `dispatcher`-class object is checked at minting (its `DispatcherQueue`
//! must have thread access here), and in debug builds on every call; `ui`-class objects need the UI thread. Async
//! completions and off-thread events hop to the UI thread before any conversion (`delegate.rs`).
use super::delegate::{self, Part, Target};
use super::invoke::Word;
use super::model::{self, *};
use super::rt::{self, CR, CallErr, Env, Keep, call_acc, factory, iid, identity, native_error, qi, vcall};
use super::tables::{CORE, KINDS, METHODS};
use super::value::{R, ty};
use crate::backend::{Entry, Inner, WinUiBackend, current, host_of};
use crate::sink::{Registration, Sink};
use std::collections::HashMap;
use svelters_abi::*;
use windows_core::{IInspectable, IUnknown, Interface};

/// The completion callback type the host appends to an async member (`SVL_M_ASYNC`), as in GTK (NT1).
pub const T_ASYNC_DONE: u32 = SVL_T_FIRST_BACKEND + 0x00FF_0000;

/// An async operation in flight: its `IAsyncInfo` (for `cancel`) and the handle it was started on (0 = static).
pub struct Pending {
    pub owner: u64,
    pub info: IUnknown,
}

pub struct BackendEnv<'a>(pub &'a Inner);

impl Env for BackendEnv<'_> {
    fn object(&self, h: u64) -> R<IUnknown> {
        match self.0.handles.borrow().get(&h) {
            Some(e) => e.obj.cast::<IUnknown>().map_err(|e| rt::err_text(&e)),
            None => Err(format!("stale or foreign handle {h}")),
        }
    }

    fn mint(&self, obj: IUnknown, declared: u16) -> R<u64> {
        mint(self.0, obj, declared)
    }

    fn known(&self, obj: &IUnknown) -> Option<u64> {
        self.0.by_ptr.borrow().get(&identity(obj)).copied()
    }
}

/// The handle of `obj`: an existing one (identity), else a new SHARED handle of its runtime class when that class is
/// bound, else of `declared` (index into `KINDS`; 0 = the IInspectable root).
pub fn mint(inner: &Inner, obj: IUnknown, declared: u16) -> R<u64> {
    let id = identity(&obj);
    if let Some(h) = inner.by_ptr.borrow().get(&id) {
        return Ok(*h);
    }
    let kind = rt::runtime_class_name(&obj).map(|n| model::kind_by_name(&n)).filter(|k| *k != 0).unwrap_or(KIND_BASE + declared as u32);
    thread_check(inner, &obj, kind)?;
    let insp = obj.cast::<IInspectable>().map_err(|e| format!("not a WinRT object: {}", rt::err_text(&e)))?;
    let h = inner.next.get();
    inner.next.set(h + 1);
    inner.by_ptr.borrow_mut().insert(id, h);
    inner.handles.borrow_mut().insert(h, Entry::shared(insp, kind));
    Ok(h)
}

/// Section 4.1 rules 2-3: a `dispatcher` object's queue must have thread access on this (the UI) thread; `ui`
/// objects need the UI thread.
pub fn thread_check(inner: &Inner, obj: &IUnknown, kind: u32) -> R<()> {
    let Some(k) = model::kind(kind) else { return Ok(()) };
    match k.thread {
        T_DISPATCHER if k.thread_getter.is_some() => {
            let mut q: *mut std::ffi::c_void = std::ptr::null_mut();
            unsafe { call_acc(obj, k.thread_getter, &[Word::ptr(&mut q)]) }.map_err(|e| format!("{}: DispatcherQueue: {}", k.name, rt::err_text(&e)))?;
            if q.is_null() {
                return Ok(());
            }
            // SAFETY: an owned IDispatcherQueue pointer.
            let q = unsafe { IUnknown::from_raw(q) };
            let mut access = 0u64;
            unsafe { call_acc(&q, CORE.dq_has_thread_access, &[Word::ptr(&mut access)]) }.map_err(|e| rt::err_text(&e))?;
            if access as u8 == 0 {
                return Err(format!(
                    "{}: the object belongs to another UI thread (its DispatcherQueue has no thread access here); \
                     svelters runs every XAML and Composition object on its one UI thread",
                    k.name
                ));
            }
            Ok(())
        }
        T_UI if !inner.xaml.queue.HasThreadAccess().unwrap_or(false) => Err(format!("{}: XAML objects are used on the UI thread only", k.name)),
        _ => Ok(()),
    }
}

fn kind_name(kind: u32) -> &'static str {
    model::kind(kind).map_or("?", |k| k.name)
}

fn err_value(e: CallErr) -> StrResult<Value> {
    match e {
        CallErr::Win(e) => Ok(native_error(&e)),
        CallErr::Msg(m) => Err(m),
    }
}

// ---- resolve / describe -----------------------------------------------------------------------------------------

pub fn resolve(what: NameKind, kind: u32, name: &str) -> u32 {
    match what {
        NameKind::Kind => model::kind_by_name(name),
        NameKind::Method => model::group_id(kind, name),
        NameKind::Prop => model::prop_id(kind, name),
        NameKind::Signal => model::event_id(kind, name),
        _ => 0,
    }
}

fn dyn_slot(name: &str) -> SlotDesc {
    SlotDesc::new(SVL_NULL).typed(SVL_T_VARIANT).named(name)
}

pub fn describe(what: NameKind, kind: u32, member: u32) -> Option<MemberDesc> {
    match what {
        NameKind::Kind => {
            let k = model::kind(if member != 0 { member } else { kind })?;
            let parent = if k.parent != 0 { KIND_BASE + k.parent as u32 - 1 } else { 0 };
            Some(MemberDesc { name: k.name.to_string(), params: vec![], ret: SlotDesc::new(SVL_NULL).typed(parent), flags: 0 })
        }
        NameKind::Method => {
            let g = model::group(member)?;
            if !model::applies(kind, g.kind) {
                return None;
            }
            let (min, max) = model::arity_range(g);
            let widest = g.methods.iter().map(|m| &METHODS[*m as usize]).max_by_key(|m| m.params.len())?;
            let mut params: Vec<SlotDesc> = widest
                .names
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let s = dyn_slot(n);
                    if i >= min { s.flags(SVL_D_OPTIONAL) } else { s }
                })
                .collect();
            let _ = max;
            let mut flags = SVL_M_THROWS;
            if g.flags & G_STATIC != 0 {
                flags |= SVL_M_STATIC;
            }
            if g.flags & G_CTOR != 0 {
                flags |= SVL_M_CTOR;
            }
            if g.methods.iter().any(|m| METHODS[*m as usize].flags & M_ASYNC != 0) {
                flags |= SVL_M_ASYNC;
                params.push(SlotDesc::new(SVL_CALLBACK).typed(T_ASYNC_DONE).named("done"));
            }
            Some(MemberDesc { name: g.name.to_string(), params, ret: dyn_slot("result"), flags })
        }
        NameKind::Prop => {
            let p = model::prop(member)?;
            if !model::applies(kind, p.kind) {
                return None;
            }
            Some(MemberDesc {
                name: p.name.to_string(),
                params: if p.set.is_some() { vec![dyn_slot(p.name)] } else { vec![] },
                ret: if p.get.is_some() { dyn_slot(p.name) } else { SlotDesc::new(SVL_NULL) },
                flags: SVL_M_THROWS | if p.dp.is_some() && p.get.is_some() { SVL_M_OBSERVABLE } else { 0 },
            })
        }
        _ => None,
    }
}

pub fn describe_type(t: u32) -> Option<TypeDesc> {
    (t == T_ASYNC_DONE).then(|| TypeDesc {
        name: "(result: unknown) => void".into(),
        kind: SVL_CALLBACK,
        scope: SVL_CB_ASYNC,
        members: vec![],
        fields: vec![SlotDesc::new(SVL_NULL).named("result")],
        ret: SlotDesc::new(SVL_NULL),
    })
}

// ---- calls --------------------------------------------------------------------------------------------------------

enum Recv<'a> {
    Instance(&'a IUnknown),
    Static,
}

/// The completion callback the host appended to an async call, and the JS arguments before it.
fn split_done(g: &Group, args: &[Value]) -> R<(Vec<Value>, Option<u64>)> {
    if !g.methods.iter().any(|m| METHODS[*m as usize].flags & M_ASYNC != 0) {
        return Ok((args.to_vec(), None));
    }
    match args.split_last() {
        Some((Value::Callback { id, .. }, rest)) => Ok((rest.to_vec(), Some(*id))),
        _ => Err(format!("{}: the last argument of an async method must be the completion callback", g.name)),
    }
}

fn invoke(inner: &Inner, recv: Recv<'_>, m: &Method, args: &[Value], done: Option<u64>, owner: u64) -> CR<Value> {
    let env = BackendEnv(inner);
    let iface = match recv {
        Recv::Instance(obj) => qi(obj, &iid(m.iid))?,
        Recv::Static => factory(KINDS[m.kind as usize].name, &iid(m.iid))?,
    };
    let mut keep = Keep::default();
    let mut words = Vec::with_capacity(m.params.len() + m.outs.len() + 3);
    for (i, (t, v)) in m.params.iter().zip(args).enumerate() {
        let w = rt::arg_in(&env, *t, v, &mut keep).map_err(|e| CallErr::Msg(format!("argument {} ({}): {}", i + 1, m.names[i], e.text())))?;
        words.push(w);
    }
    let mut composed_inner: *mut std::ffi::c_void = std::ptr::null_mut();
    if m.flags & M_COMPOSABLE != 0 {
        words.push(Word::I(0));
        words.push(Word::ptr(&mut composed_inner));
    }
    let mut outs: Vec<Vec<u64>> = m.outs.iter().map(|t| rt::out_buf(ty(*t))).collect();
    for b in outs.iter_mut() {
        words.push(Word::ptr(b.as_mut_ptr()));
    }
    let mut ret = rt::out_buf(ty(m.ret));
    if ty(m.ret) != Ty::Void {
        words.push(Word::ptr(ret.as_mut_ptr()));
    }
    let r = unsafe { vcall(&iface, m.slot, &words) };
    drop(keep);
    if !composed_inner.is_null() {
        // SAFETY: the non-delegating inner of a composed object we did not aggregate: release it.
        drop(unsafe { IUnknown::from_raw(composed_inner) });
    }
    r?;
    if let Ty::Async { of, handler_iid, progress } = ty(m.ret) {
        let done = done.ok_or("an async member needs the completion callback")?;
        let p = ret[0] as usize as *mut std::ffi::c_void;
        if p.is_null() {
            return Err("the async operation is null".into());
        }
        // SAFETY: an owned IAsyncOperation / IAsyncAction pointer.
        let op = unsafe { IUnknown::from_raw(p) };
        start_async(inner, &op, of, handler_iid, progress, done, owner)?;
        return Ok(Value::Null);
    }
    // SAFETY: the callee wrote owned values of these types.
    let value = unsafe { rt::take_out(&env, m.ret, &ret) }?;
    if m.outs.is_empty() {
        return Ok(value);
    }
    let mut list = vec![value];
    for (t, b) in m.outs.iter().zip(&outs) {
        list.push(unsafe { rt::take_out(&env, *t, b) }?);
    }
    Ok(Value::List(list))
}

fn start_async(inner: &Inner, op: &IUnknown, of: TyId, handler_iid: u16, progress: bool, done: u64, owner: u64) -> CR<()> {
    let (put, results) = if progress { (CORE.put_completed_progress, CORE.get_results_progress) } else { (CORE.put_completed, CORE.get_results) };
    let info = qi(op, &iid(CORE.async_info))?;
    let d = delegate::new(iid(handler_iid), 2, inner.xaml.queue.clone(), Target::Completion { done, of, results })?;
    inner.pending.borrow_mut().insert(done, Pending { owner, info });
    if let Err(e) = unsafe { vcall(op, put, &[Word::I(d.as_raw() as usize as u64)]) } {
        inner.pending.borrow_mut().remove(&done);
        return Err(e.into());
    }
    Ok(())
}

/// An async completion, on the UI thread (posted by the delegate).
pub fn complete(done: u64, of: TyId, results: u16, op: Option<IUnknown>, status: i32) {
    let Some(inner) = current() else { return };
    let Some(p) = inner.pending.borrow_mut().remove(&done) else { return };
    let env = BackendEnv(&inner);
    let value = match op {
        None => Value::Error(Box::new(NativeError { domain: "winrt".into(), code: Value::Str("fail".into()), message: "async completion without an operation".into() })),
        Some(op) if status == CORE.status_completed => match rt::get_value(&env, &op, results, of, &[]) {
            Ok(v) => v,
            Err(CallErr::Win(e)) => native_error(&e),
            Err(CallErr::Msg(m)) => Value::Error(Box::new(NativeError { domain: "svelters".into(), code: Value::Str("conversion".into()), message: m })),
        },
        Some(_) if status == CORE.status_canceled => {
            Value::Error(Box::new(NativeError { domain: "winrt".into(), code: Value::Str("cancelled".into()), message: "operation cancelled".into() }))
        }
        Some(op) => {
            // GetResults fails with the operation's error (and its error info).
            match rt::get_value(&env, &op, results, of, &[]) {
                Err(CallErr::Win(e)) => native_error(&e),
                _ => {
                    let mut code = 0u64;
                    let _ = unsafe { vcall(&p.info, CORE.info_error_code, &[Word::ptr(&mut code)]) };
                    native_error(&windows_core::Error::from_hresult(windows_core::HRESULT(code as u32 as i32)))
                }
            }
        }
    };
    let _ = unsafe { vcall(&p.info, CORE.info_close, &[]) };
    if let Some(host) = host_of(&inner) {
        drop(inner);
        let _ = host.invoke_callback(done, &[value]);
    }
}

/// `backend.cancel(callback)`: `IAsyncInfo.Cancel`; the completion then rejects with `cancelled`.
pub fn cancel(inner: &Inner, done: u64) -> bool {
    let info = inner.pending.borrow().get(&done).map(|p| p.info.clone());
    match info {
        Some(i) => unsafe { vcall(&i, CORE.info_cancel, &[]) }.is_ok(),
        None => false,
    }
}

/// The owner of in-flight operations died: cancel them and forget their completions (the host settles them).
pub fn cancel_owned(inner: &Inner, h: u64) {
    let gone: Vec<Pending> = {
        let mut p = inner.pending.borrow_mut();
        let ids: Vec<u64> = p.iter().filter(|(_, x)| x.owner == h).map(|(id, _)| *id).collect();
        ids.into_iter().filter_map(|id| p.remove(&id)).collect()
    };
    for p in gone {
        let _ = unsafe { vcall(&p.info, CORE.info_cancel, &[]) };
    }
}

impl WinUiBackend {
    fn type_entry(&self, h: u64) -> R<(IUnknown, u32)> {
        let inner = self.inner();
        let (obj, kind) = inner.get(h)?;
        let unk = obj.cast::<IUnknown>().map_err(|e| rt::err_text(&e))?;
        if cfg!(debug_assertions) {
            thread_check(inner, &unk, kind)?;
        }
        Ok((unk, kind))
    }

    pub(crate) fn type_call(&self, h: u64, method: u32, args: &[Value]) -> StrResult<Value> {
        let (obj, kind) = self.type_entry(h)?;
        let g = model::group(method).ok_or_else(|| format!("unknown method id {method}"))?;
        if !model::applies(kind, g.kind) {
            return Err(format!("{} has no method {:?}", kind_name(kind), g.name));
        }
        if g.flags & G_STATIC != 0 {
            return self.type_call_static(kind, method, args);
        }
        let (args, done) = split_done(g, args)?;
        let m = model::pick_for(g, &args)?;
        invoke(self.inner(), Recv::Instance(&obj), m, &args[..m.params.len()], done, h).or_else(|e| err_value(e)).map_err(|e| format!("{}(): {e}", g.name))
    }

    pub(crate) fn type_call_static(&self, kind: u32, method: u32, args: &[Value]) -> StrResult<Value> {
        let g = model::group(method).ok_or_else(|| format!("unknown method id {method}"))?;
        if !model::applies(kind, g.kind) {
            return Err(format!("{} has no static {:?}", kind_name(kind), g.name));
        }
        if g.flags & G_STATIC == 0 {
            return Err(format!("{}.{} is an instance method", kind_name(kind), g.name));
        }
        let (args, done) = split_done(g, args)?;
        let m = model::pick_for(g, &args)?;
        invoke(self.inner(), Recv::Static, m, &args[..m.params.len()], done, 0).or_else(|e| err_value(e)).map_err(|e| format!("{}.{}(): {e}", kind_name(kind), g.name))
    }

    pub(crate) fn type_construct(&self, kind: u32, props: &[u32], values: &[Value]) -> StrResult<Value> {
        let g = model::group(model::group_id(kind, "new")).ok_or_else(|| format!("{} cannot be constructed", kind_name(kind)))?;
        if g.flags & G_CTOR == 0 || g.kind as u32 + KIND_BASE != kind {
            return Err(format!("{} has no constructor of its own", kind_name(kind)));
        }
        let m = model::pick(g, 0).map_err(|_| format!("{} has no default constructor (use `new` with arguments)", kind_name(kind)))?;
        let v = invoke(self.inner(), Recv::Static, m, &[], None, 0).or_else(|e| err_value(e))?;
        let Value::Handle(h) = v else { return Ok(v) };
        for (p, v) in props.iter().zip(values) {
            if let Err(e) = self.type_set(h, *p, v) {
                self.destroy_type(h);
                return Err(e);
            }
        }
        Ok(Value::Handle(h))
    }

    pub(crate) fn type_get(&self, h: u64, prop: u32) -> StrResult<Value> {
        let (obj, kind) = self.type_entry(h)?;
        let p = model::prop(prop).ok_or_else(|| format!("unknown prop id {prop}"))?;
        if !model::applies(kind, p.kind) {
            return Err(format!("{} has no prop {:?}", kind_name(kind), p.name));
        }
        if !p.get.is_some() {
            return Err(format!("{:?} is write-only", p.name));
        }
        let inner = self.inner();
        let r = (|| -> CR<Value> {
            let i = qi(&obj, &iid(p.get.iid))?;
            rt::get_value(&BackendEnv(inner), &i, p.get.slot, p.ty, &[])
        })();
        r.or_else(err_value).map_err(|e| format!("{}: {e}", p.name))
    }

    pub(crate) fn type_set(&self, h: u64, prop: u32, v: &Value) -> StrResult<()> {
        let (obj, kind) = self.type_entry(h)?;
        let p = model::prop(prop).ok_or_else(|| format!("unknown prop id {prop}"))?;
        if !model::applies(kind, p.kind) {
            return Err(format!("{} has no prop {:?}", kind_name(kind), p.name));
        }
        if !p.set.is_some() {
            return Err(format!("{:?} is read-only", p.name));
        }
        let inner = self.inner();
        let r = (|| -> CR<()> {
            let i = qi(&obj, &iid(p.set.iid))?;
            let mut keep = Keep::default();
            let w = rt::arg_in(&BackendEnv(inner), p.ty, v, &mut keep)?;
            unsafe { vcall(&i, p.set.slot, &[w])? };
            Ok(())
        })();
        r.map_err(|e| format!("{}: {}", p.name, e.text()))
    }

    pub(crate) fn type_listen(&self, h: u64, ev: u32, on: bool) -> StrResult<()> {
        let (obj, kind) = self.type_entry(h)?;
        let e = model::event(ev).ok_or_else(|| format!("unknown event id {ev}"))?;
        if !model::applies(kind, e.kind) {
            return Err(format!("{} has no event {:?}", kind_name(kind), e.name));
        }
        let inner = self.inner();
        let key = (ev, 0);
        if !on {
            let r = inner.handles.borrow_mut().get_mut(&h).and_then(|x| x.listeners.remove(&key));
            drop(r);
            return Ok(());
        }
        if inner.handles.borrow().get(&h).is_some_and(|x| x.listeners.contains_key(&key)) {
            return Ok(());
        }
        let reg = (|| -> CR<Registration> {
            let iface = qi(&obj, &iid(e.iid))?;
            let d = delegate::new(iid(e.delegate_iid), e.args.len(), inner.xaml.queue.clone(), Target::Event { h, ev, args: e.args })?;
            let mut token = 0i64;
            unsafe { vcall(&iface, e.add, &[Word::I(d.as_raw() as usize as u64), Word::ptr(&mut token)])? };
            Ok(Registration::Dynamic { iface, remove: e.remove, token })
        })()
        .map_err(|x| format!("listen {:?}: {}", e.name, x.text()))?;
        if let Some(x) = inner.handles.borrow_mut().get_mut(&h) {
            x.listeners.insert(key, reg);
        }
        Ok(())
    }

    /// ABI v2.6 `observe` of a DependencyProperty-backed prop of a type handle (`RegisterPropertyChangedCallback`,
    /// the R4 mechanism); other props are not observable.
    pub(crate) fn type_observe(&self, h: u64, prop: u32, on: bool) -> Result<(), ObserveError> {
        let inner = self.inner();
        if !on {
            let regs = {
                let mut handles = inner.handles.borrow_mut();
                let Some(e) = handles.get_mut(&h) else { return Ok(()) };
                let Some(o) = e.observers.get_mut(&prop) else { return Ok(()) };
                o.0 -= 1;
                if o.0 > 0 {
                    return Ok(());
                }
                e.observers.remove(&prop)
            };
            drop(regs);
            return Ok(());
        }
        let (obj, kind) = self.type_entry(h).map_err(ObserveError::Failed)?;
        let p = model::prop(prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop}")))?;
        if !model::applies(kind, p.kind) {
            return Err(ObserveError::Failed(format!("{} has no prop {:?}", kind_name(kind), p.name)));
        }
        if !p.dp.is_some() || !p.get.is_some() {
            return Err(ObserveError::Unsupported(format!("{:?} is not a DependencyProperty: WinUI does not announce its changes", p.name)));
        }
        if let Some(o) = inner.handles.borrow_mut().get_mut(&h).and_then(|e| e.observers.get_mut(&prop)) {
            o.0 += 1;
            return Ok(());
        }
        let reg = (|| -> CR<Registration> {
            use crate::bindings::Microsoft::UI::Xaml::DependencyProperty;
            let f = factory(KINDS[p.kind as usize].name, &iid(p.dp.iid))?;
            let mut out: *mut std::ffi::c_void = std::ptr::null_mut();
            unsafe { vcall(&f, p.dp.slot, &[Word::ptr(&mut out)])? };
            if out.is_null() {
                return Err("null DependencyProperty".into());
            }
            // SAFETY: an owned DependencyProperty (IDependencyProperty) pointer.
            let dp: DependencyProperty = unsafe { IInspectable::from_raw(out) }.cast()?;
            let insp = obj.cast::<IInspectable>()?;
            Ok(crate::sink::observe_dp(&insp, &dp, h, prop)?)
        })()
        .map_err(|e| ObserveError::Failed(format!("observe {:?}: {}", p.name, e.text())))?;
        match inner.handles.borrow_mut().get_mut(&h) {
            Some(e) => {
                e.observers.insert(prop, (1, vec![reg]));
                Ok(())
            }
            None => Err(ObserveError::Failed(format!("handle {h} died while its observation was installed"))),
        }
    }

    /// `destroy` of a SHARED handle: drop the observations and listeners, cancel its operations, drop our reference.
    pub(crate) fn destroy_type(&self, h: u64) {
        let inner = self.inner();
        let Some(e) = inner.handles.borrow_mut().remove(&h) else { return };
        inner.by_ptr.borrow_mut().retain(|_, v| *v != h);
        cancel_owned(inner, h);
        let Entry { observers, listeners, .. } = e;
        drop(observers);
        drop(listeners);
    }
}

// ---- event delivery (UI thread) -------------------------------------------------------------------------------

fn post_values(inner: &Inner, h: u64, ev: u32, values: Vec<Value>) {
    if !inner.handles.borrow().contains_key(&h) {
        return;
    }
    let _ = Sink { h, ev };
    if let Some(host) = host_of(inner) {
        host.post_event(h, ev, &values);
    }
}

/// An event raised on the UI thread: convert the borrowed arguments now.
pub fn deliver_event(h: u64, ev: u32, tys: &'static [TyId], args: &[u64]) {
    let Some(inner) = current() else { return };
    if !inner.handles.borrow().contains_key(&h) {
        return;
    }
    let env = BackendEnv(&inner);
    let values = tys
        .iter()
        .zip(args)
        .map(|(t, w)| unsafe { rt::borrowed_value(&env, *t, *w) }.unwrap_or_else(|e| Value::Str(format!("<{}>", e.text()))))
        .collect();
    post_values(&inner, h, ev, values);
}

/// An event raised on another thread, after the hop: convert the captured arguments.
pub fn deliver_captured(h: u64, ev: u32, tys: &'static [TyId], parts: Vec<Part>) {
    let Some(inner) = current() else { return };
    if !inner.handles.borrow().contains_key(&h) {
        return;
    }
    let env = BackendEnv(&inner);
    let values = tys
        .iter()
        .zip(parts)
        .map(|(t, p)| {
            let r: CR<Value> = match p {
                Part::Word(w) => Ok(super::value::read_plain(ty(*t), &w.to_le_bytes())),
                Part::Bytes(b) => Ok(super::value::read_plain(ty(*t), &b)),
                Part::Str(s) => Ok(Value::Str(s)),
                Part::Obj(None) => Ok(Value::Null),
                Part::Obj(Some(o)) => rt::object_value(&env, *t, o),
            };
            r.unwrap_or_else(|e| Value::Str(format!("<{}>", e.text())))
        })
        .collect();
    post_values(&inner, h, ev, values);
}

/// Unused-import guard for `HashMap` when the pending map type changes.
pub type PendingMap = HashMap<u64, Pending>;
