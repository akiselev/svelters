//! GI closures by scope and async completions (N1 of plans/research/NON-GUI-TYPES.md).
//!
//! **JS callbacks** (`CALLBACK` arguments of typelib functions) become libffi closures made by
//! `gi_callable_info_create_closure`. Each [`NativeClosure`] owns its `ffi_cif` storage, the closure,
//! a reference to the `GICallbackInfo` and the host callback id; the trampoline converts the native
//! arguments with the callback's signature, runs the JS function through `host.invoke_callback` and
//! converts its result back. When the closure is freed depends on the GIR scope:
//!
//! | scope | freed | host id released |
//! |---|---|---|
//! | `call` | when the native call returns (`Keep::drop`) | by the host after the call |
//! | `async` | after its first invocation (idle) | by the host after the invocation |
//! | `notified` | by the `GDestroyNotify` the callee calls (idle) | by the closure when freed |
//! | `forever` | never (like GJS) | when the owner handle dies (the host sees `notified`) |
//!
//! A closure whose native call never ran (an argument failed to convert) is freed at once.
//!
//! Lifetime rules, from the 2021 crashes (docs/notes/gtk-v1-archaeology.md): a closure is never freed
//! while its trampoline runs (frees are deferred to an idle; a destroy notify that arrives during an
//! invocation only marks it), the trampoline holds the backend weakly and checks the thread before
//! touching it, and a callback whose JS function was released (owner gone, component destroyed)
//! returns a zero value instead of failing natively.
//!
//! **Async** (`foo_async` + `foo_finish`): [`AsyncOp`] is the user data of a plain
//! `GAsyncReadyCallback` ([`async_ready`]). GIO calls it on the main loop of the thread that started
//! the operation, never synchronously; it runs the finish function, folds the out parameters into one
//! object, and completes the host's one-shot callback. Cancellation goes through the operation's
//! `GCancellable` (`backend.cancel(id)`).

use crate::gi::{self, read, to_arg, CbSig, CbSpec, GIArgument, Host, Info, Keep, Owned, Ty, ZERO};
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::ffi::{c_int, c_void};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use svelters_abi::*;

static LIVE_CLOSURES: AtomicUsize = AtomicUsize::new(0);
static LIVE_ASYNC: AtomicUsize = AtomicUsize::new(0);

/// Native closures not yet freed (all scopes), process-wide. For tests.
pub fn live_closures() -> usize {
    LIVE_CLOSURES.load(Ordering::SeqCst)
}

/// Async operations started and not yet completed natively, process-wide. For tests.
pub fn live_async_ops() -> usize {
    LIVE_ASYNC.load(Ordering::SeqCst)
}

/// A raw pointer moved to the main context for a deferred free.
struct SendPtr(*mut NativeClosure);
unsafe impl Send for SendPtr {}

pub struct NativeClosure {
    info: Rc<Owned>,
    sig: Rc<CbSig>,
    /// `ffi_cif` storage (larger than any platform's `ffi_cif`); must outlive the closure.
    _cif: Box<[u64; 32]>,
    closure: *mut c_void,
    /// The C function pointer native code calls.
    pub(crate) native: *mut c_void,
    id: u64,
    scope: c_int,
    owner: u64,
    host: Weak<dyn Host>,
    thread: std::thread::ThreadId,
    running: Cell<u32>,
    /// A free was requested; done once the trampoline is no longer running.
    doomed: Cell<bool>,
    /// The temporaries of the last result (a transfer-none string or object must outlive the return).
    last: RefCell<Option<Keep>>,
}

impl NativeClosure {
    /// A closure calling the host callback `id` with the signature of `spec`.
    pub(crate) fn create(spec: &CbSpec, id: u64, host: Weak<dyn Host>, owner: u64) -> Result<*mut NativeClosure, String> {
        let mut cif = Box::new([0u64; 32]);
        let cif_ptr = cif.as_mut_ptr() as *mut c_void;
        let c = Box::into_raw(Box::new(NativeClosure {
            info: spec.info.clone(),
            sig: spec.sig.clone(),
            _cif: cif,
            closure: std::ptr::null_mut(),
            native: std::ptr::null_mut(),
            id,
            scope: spec.scope,
            owner,
            host,
            thread: std::thread::current().id(),
            running: Cell::new(0),
            doomed: Cell::new(false),
            last: RefCell::new(None),
        }));
        let info: Info = spec.info.0;
        let clo = unsafe { gi::gi_callable_info_create_closure(info, cif_ptr, trampoline, c as *mut c_void) };
        if clo.is_null() {
            drop(unsafe { Box::from_raw(c) });
            return Err(format!("cannot create a native closure for {}", spec.name));
        }
        unsafe {
            (*c).closure = clo;
            (*c).native = gi::gi_callable_info_get_closure_native_address(info, clo);
        }
        LIVE_CLOSURES.fetch_add(1, Ordering::SeqCst);
        Ok(c)
    }

    /// Frees now. Only when the trampoline cannot be running (a call-scope closure after its call, a
    /// closure the callee never saw) or from the deferred idle.
    pub(crate) unsafe fn free(c: *mut NativeClosure) {
        let b = unsafe { Box::from_raw(c) };
        unsafe { gi::gi_callable_info_destroy_closure(b.info.0, b.closure) };
        LIVE_CLOSURES.fetch_sub(1, Ordering::SeqCst);
        if b.thread == std::thread::current().id() {
            if let Some(h) = b.host.upgrade() {
                h.release_callback(b.id);
            }
            drop(b);
        } else {
            // Rc-backed fields must not be touched off their thread: leak them.
            std::mem::forget(b);
        }
    }

    /// Frees on the next main-loop idle (never inside a native call that may still run it).
    fn free_later(c: *mut NativeClosure) {
        let p = SendPtr(c);
        glib::idle_add_once(move || {
            let p = p;
            let c = p.0;
            unsafe {
                if (*c).running.get() > 0 {
                    (*c).doomed.set(true);
                } else {
                    NativeClosure::free(c);
                }
            }
        });
    }
}

/// `GDestroyNotify` passed with a `notified`-scope closure (its user data is the closure).
pub(crate) unsafe extern "C" fn closure_destroy(data: *mut c_void) {
    if !data.is_null() {
        NativeClosure::free_later(data as *mut NativeClosure);
    }
}

/// Reads a native argument (pointed to by libffi) into a GIArgument of its type.
unsafe fn load(ty: &Ty, p: *mut c_void) -> GIArgument {
    let mut a = ZERO;
    unsafe {
        match ty {
            Ty::Bool => a.v_boolean = *(p as *const c_int),
            Ty::I8 => a.v_int8 = *(p as *const i8),
            Ty::U8 => a.v_uint8 = *(p as *const u8),
            Ty::I16 => a.v_int16 = *(p as *const i16),
            Ty::U16 => a.v_uint16 = *(p as *const u16),
            Ty::I32 | Ty::Enum(_) => a.v_int32 = *(p as *const i32),
            Ty::U32 | Ty::Flags(_) | Ty::Unichar => a.v_uint32 = *(p as *const u32),
            Ty::I64 => a.v_int64 = *(p as *const i64),
            Ty::U64 => a.v_uint64 = *(p as *const u64),
            Ty::F32 => a.v_float = *(p as *const f32),
            Ty::F64 => a.v_double = *(p as *const f64),
            Ty::Gtype => a.v_uint64 = *(p as *const usize) as u64,
            _ => a.v_pointer = *(p as *const *mut c_void),
        }
    }
    a
}

/// Writes a return value where libffi expects it (integers widened to a full `ffi_arg`).
unsafe fn store(ty: &Ty, a: GIArgument, ret: *mut c_void) {
    unsafe {
        match ty {
            Ty::Void => {}
            Ty::Bool => *(ret as *mut i64) = a.v_boolean as i64,
            Ty::I8 => *(ret as *mut i64) = a.v_int8 as i64,
            Ty::U8 => *(ret as *mut u64) = a.v_uint8 as u64,
            Ty::I16 => *(ret as *mut i64) = a.v_int16 as i64,
            Ty::U16 => *(ret as *mut u64) = a.v_uint16 as u64,
            Ty::I32 | Ty::Enum(_) => *(ret as *mut i64) = a.v_int32 as i64,
            Ty::U32 | Ty::Flags(_) | Ty::Unichar => *(ret as *mut u64) = a.v_uint32 as u64,
            Ty::I64 => *(ret as *mut i64) = a.v_int64,
            Ty::U64 => *(ret as *mut u64) = a.v_uint64,
            Ty::F32 => *(ret as *mut f32) = a.v_float,
            Ty::F64 => *(ret as *mut f64) = a.v_double,
            Ty::Gtype => *(ret as *mut usize) = a.v_uint64 as usize,
            _ => *(ret as *mut *mut c_void) = a.v_pointer,
        }
    }
}

unsafe extern "C" fn trampoline(_cif: *mut c_void, ret: *mut c_void, args: *mut *mut c_void, data: *mut c_void) {
    let c = unsafe { &*(data as *const NativeClosure) };
    let sig = c.sig.clone();
    unsafe { store(&sig.ret.ty, ZERO, ret) };
    if c.thread != std::thread::current().id() {
        // JS lives on the UI thread; a callback from a worker thread gets the zero value.
        eprintln!("svelters-gtk: a callback was invoked off the main thread; ignored");
        return;
    }
    c.running.set(c.running.get() + 1);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { run(c, &sig, ret, args) }));
    c.running.set(c.running.get() - 1);
    if let Err(_) | Ok(Err(_)) = &r {
        if let Ok(Err(e)) = &r {
            if !e.is_empty() {
                eprintln!("svelters-gtk: callback {}: {e}", c.id);
            }
        }
    }
    if c.scope == gi::SCOPE_ASYNC || (c.doomed.get() && c.running.get() == 0) {
        NativeClosure::free_later(data as *mut NativeClosure);
    }
}

unsafe fn run(c: &NativeClosure, sig: &CbSig, ret: *mut c_void, args: *mut *mut c_void) -> Result<(), String> {
    // The backend is gone (shut down): the zero value.
    let Some(host) = c.host.upgrade() else { return Ok(()) };
    let mut vals = Vec::with_capacity(sig.args.len());
    for (i, a) in sig.args.iter().enumerate() {
        let Some((name, slot)) = a else { continue };
        let ga = unsafe { load(&slot.ty, *args.add(i)) };
        let r = read(slot, ga, None, name)?;
        vals.push(host.ret_to_value(r, c.owner));
    }
    // A released function (its owner died) is a clean error: the zero value goes back.
    let v = match host.invoke_callback(c.id, &vals) {
        Ok(v) => v,
        Err(_) => return Err(String::new()),
    };
    if sig.ret.ty == Ty::Void {
        return Ok(());
    }
    let gi = host.gi().ok_or("no typelib repository")?;
    let mut keep = Keep::default();
    let a = v.with_raw(|raw| to_arg(&sig.ret, raw, "callback result", &mut keep, host.as_env(), &gi))?;
    unsafe { store(&sig.ret.ty, a, ret) };
    *c.last.borrow_mut() = Some(keep);
    Ok(())
}

/// One running `_async` call: the user data of [`async_ready`].
pub struct AsyncOp {
    finish: Rc<gi::DynMethod>,
    recv: Option<glib::Object>,
    done: u64,
    host: Weak<dyn Host>,
    owner: u64,
    thread: std::thread::ThreadId,
}

impl AsyncOp {
    pub(crate) fn new(finish: Rc<gi::DynMethod>, recv: Option<glib::Object>, done: u64, host: Weak<dyn Host>, owner: u64) -> *mut AsyncOp {
        LIVE_ASYNC.fetch_add(1, Ordering::SeqCst);
        Box::into_raw(Box::new(AsyncOp { finish, recv, done, host, owner, thread: std::thread::current().id() }))
    }

    /// The operation never started: free it.
    pub(crate) unsafe fn discard(op: *mut AsyncOp) {
        LIVE_ASYNC.fetch_sub(1, Ordering::SeqCst);
        drop(unsafe { Box::from_raw(op) });
    }
}

/// Folds a finish result: the plain value, or `{ret?, out...}` when the finish function has out
/// parameters (the S2 out-parameter shape, keys as the native parameter names).
fn fold(host: &dyn Host, finish: &gi::DynMethod, called: gi::Called, owner: u64) -> Value {
    let (has_ret, names) = finish.result_shape();
    if names.is_empty() {
        return host.ret_to_value(called.ret, owner);
    }
    let mut m = Vec::new();
    if has_ret {
        m.push(("ret".to_string(), host.ret_to_value(called.ret, owner)));
    }
    for (n, o) in names.into_iter().zip(called.outs) {
        m.push((n, host.ret_to_value(o, owner)));
    }
    Value::Map(m)
}

/// The `GAsyncReadyCallback` of every folded async call.
pub(crate) unsafe extern "C" fn async_ready(_source: *mut glib::gobject_ffi::GObject, res: *mut c_void, data: *mut c_void) {
    let op = unsafe { Box::from_raw(data as *mut AsyncOp) };
    LIVE_ASYNC.fetch_sub(1, Ordering::SeqCst);
    if op.thread != std::thread::current().id() {
        eprintln!("svelters-gtk: an async operation completed off the main thread; dropped");
        std::mem::forget(op);
        return;
    }
    let Some(host) = op.host.upgrade() else { return };
    if !host.async_finished(op.done) {
        return;
    }
    let Some(gi) = host.gi() else { return };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| op.finish.finish_with(op.recv.as_ref(), res, host.as_env(), &gi)));
    let v = match r {
        Ok(Ok(called)) => fold(&*host, &op.finish, called, op.owner),
        Ok(Err(gi::GiErr::Gerror(e))) => host.error_value(&e),
        Ok(Err(gi::GiErr::Plain(m))) => Value::Error(Box::new(NativeError { domain: "svelters".into(), code: Value::Null, message: m })),
        Err(_) => Value::Error(Box::new(NativeError { domain: "svelters".into(), code: Value::Null, message: "internal panic in svelters-gtk".into() })),
    };
    // A completion whose callback is gone (owner destroyed) is dropped by the host.
    let _ = host.invoke_callback(op.done, &[v]);
}
