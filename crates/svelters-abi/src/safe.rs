//! Safe wrappers over the raw ABI types.
//!
//! - [`Value`]: owned value enum with conversion to and from `svl_value`.
//! - [`BackendRef`] / [`HostRef`]: callers' views of a vtable (the host drives a backend; a
//!   backend drives the host callbacks).
//! - [`Backend`] + [`BackendVtable`]: implement a backend as a Rust trait and get an
//!   `svl_backend` vtable.
//! - [`HostCallbacks`] + [`HostVtable`]: the same for the `svl_host` callbacks.
//!
//! All trait methods take `&self`. Backends are re-entered by design (a backend's `run` calls
//! `host.checkpoint`, which calls back into the backend's `create`/`set_prop`), so a `&mut self`
//! API would alias. Backends use interior mutability (`RefCell`, `Cell`).

use crate::*;
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

/// Name scopes for [`Backend::resolve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    Kind = 0,
    Prop = 1,
    Signal = 2,
    Method = 3,
    Slot = 4,
}

impl NameKind {
    pub fn from_u32(v: u32) -> Option<NameKind> {
        Some(match v {
            SVL_KIND => NameKind::Kind,
            SVL_PROP => NameKind::Prop,
            SVL_SIGNAL => NameKind::Signal,
            SVL_METHOD => NameKind::Method,
            SVL_SLOT => NameKind::Slot,
            _ => return None,
        })
    }
}

/// Failure reported through a negative return code plus `last_error`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbiError {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for AbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for AbiError {}

pub type AbiResult<T> = std::result::Result<T, AbiError>;

/// Owned counterpart of `svl_value`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    Handle(svl_handle),
    List(Vec<Value>),
}

/// Raw values built from a [`Value`] slice. Pointers inside stay valid while this lives and the
/// source slice is not mutated or dropped.
pub struct RawValues<'a> {
    raw: Vec<svl_value>,
    _lists: Vec<Vec<svl_value>>,
    _src: PhantomData<&'a [Value]>,
}

fn build_raw(v: &Value, lists: &mut Vec<Vec<svl_value>>) -> svl_value {
    match v {
        Value::Null => svl_value {
            tag: SVL_NULL,
            u: svl_value_u { i: 0 },
        },
        Value::Bool(b) => svl_value {
            tag: SVL_BOOL,
            u: svl_value_u { b: *b },
        },
        Value::I64(i) => svl_value {
            tag: SVL_I64,
            u: svl_value_u { i: *i },
        },
        Value::F64(f) => svl_value {
            tag: SVL_F64,
            u: svl_value_u { f: *f },
        },
        Value::Handle(h) => svl_value {
            tag: SVL_HANDLE,
            u: svl_value_u { h: *h },
        },
        Value::Str(s) => svl_value {
            tag: SVL_STR,
            u: svl_value_u {
                s: svl_str {
                    ptr: s.as_ptr().cast(),
                    len: s.len(),
                },
            },
        },
        Value::List(items) => {
            let inner: Vec<svl_value> = items.iter().map(|x| build_raw(x, lists)).collect();
            // The Vec's heap buffer does not move when the Vec itself is moved into `lists`.
            let l = svl_list {
                ptr: inner.as_ptr(),
                len: inner.len(),
            };
            lists.push(inner);
            svl_value {
                tag: SVL_LIST,
                u: svl_value_u { l },
            }
        }
    }
}

impl<'a> RawValues<'a> {
    pub fn new(values: &'a [Value]) -> RawValues<'a> {
        let mut lists = Vec::new();
        let raw = values.iter().map(|v| build_raw(v, &mut lists)).collect();
        RawValues {
            raw,
            _lists: lists,
            _src: PhantomData,
        }
    }
    pub fn as_ptr(&self) -> *const svl_value {
        self.raw.as_ptr()
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

impl Value {
    /// Calls `f` with a raw view of this value.
    pub fn with_raw<R>(&self, f: impl FnOnce(&svl_value) -> R) -> R {
        let mut lists = Vec::new();
        let raw = build_raw(self, &mut lists);
        f(&raw)
    }

    /// Copies a raw value. Unknown tags become `Null`; invalid UTF-8 is replaced lossily.
    ///
    /// # Safety
    /// `v` must be a valid `svl_value` whose pointers are valid for reads.
    pub unsafe fn from_raw(v: &svl_value) -> Value {
        unsafe {
            match v.tag {
                SVL_BOOL => Value::Bool(v.u.b),
                SVL_I64 => Value::I64(v.u.i),
                SVL_F64 => Value::F64(v.u.f),
                SVL_HANDLE => Value::Handle(v.u.h),
                SVL_STR => {
                    let s = v.u.s;
                    if s.ptr.is_null() || s.len == 0 {
                        Value::Str(String::new())
                    } else {
                        let bytes = std::slice::from_raw_parts(s.ptr.cast::<u8>(), s.len);
                        Value::Str(String::from_utf8_lossy(bytes).into_owned())
                    }
                }
                SVL_LIST => Value::List(Value::vec_from_raw(v.u.l.ptr, v.u.l.len)),
                _ => Value::Null,
            }
        }
    }

    /// Copies `n` raw values.
    ///
    /// # Safety
    /// `ptr` must be null (only if `n == 0`) or point to `n` valid values.
    pub unsafe fn vec_from_raw(ptr: *const svl_value, n: usize) -> Vec<Value> {
        if ptr.is_null() || n == 0 {
            return Vec::new();
        }
        unsafe {
            std::slice::from_raw_parts(ptr, n)
                .iter()
                .map(|v| Value::from_raw(v))
                .collect()
        }
    }
}

fn null_value() -> svl_value {
    svl_value {
        tag: SVL_NULL,
        u: svl_value_u { i: 0 },
    }
}

/// A thread-safe handle to a backend's `wake`.
///
/// Created with [`BackendRef::wake_handle`]. The backend must outlive every use.
#[derive(Clone, Copy)]
pub struct WakeHandle {
    wake: unsafe extern "C" fn(*mut c_void),
    self_: *mut c_void,
}

// SAFETY: `wake` is the one entry point the ABI documents as thread-safe.
unsafe impl Send for WakeHandle {}
unsafe impl Sync for WakeHandle {}

impl WakeHandle {
    pub fn wake(&self) {
        unsafe { (self.wake)(self.self_) }
    }
}

/// The host's view of a backend vtable.
#[derive(Clone, Copy)]
pub struct BackendRef<'a> {
    raw: *const svl_backend,
    _p: PhantomData<&'a svl_backend>,
}

impl<'a> BackendRef<'a> {
    /// # Safety
    /// `raw` must point to a valid vtable that outlives `'a`.
    pub unsafe fn from_raw(raw: *const svl_backend) -> AbiResult<BackendRef<'a>> {
        let v = unsafe { (*raw).abi_version };
        if v != SVL_ABI_VERSION {
            return Err(AbiError {
                code: -1,
                message: format!("backend ABI version {v}, host expects {SVL_ABI_VERSION}"),
            });
        }
        Ok(BackendRef {
            raw,
            _p: PhantomData,
        })
    }

    /// Drops the borrow lifetime, for code that stores the reference where a lifetime parameter
    /// cannot go.
    ///
    /// # Safety
    /// The backend must outlive every use of the result.
    pub unsafe fn erase_lifetime(self) -> BackendRef<'static> {
        BackendRef {
            raw: self.raw,
            _p: PhantomData,
        }
    }

    pub fn as_raw(&self) -> *const svl_backend {
        self.raw
    }

    fn b(&self) -> &svl_backend {
        unsafe { &*self.raw }
    }

    pub fn last_error(&self) -> String {
        let b = self.b();
        let p = unsafe { (b.last_error)(b.self_) };
        if p.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }

    fn check(&self, rc: c_int) -> AbiResult<()> {
        if rc < 0 {
            Err(AbiError {
                code: rc,
                message: self.last_error(),
            })
        } else {
            Ok(())
        }
    }

    /// Returns 0 for unknown names.
    pub fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let b = self.b();
        unsafe { (b.resolve)(b.self_, what as u32, kind, name.as_ptr().cast(), name.len()) }
    }

    pub fn create(&self, kind: svl_id) -> AbiResult<svl_handle> {
        let b = self.b();
        let h = unsafe { (b.create)(b.self_, kind) };
        if h == 0 {
            Err(AbiError {
                code: -1,
                message: self.last_error(),
            })
        } else {
            Ok(h)
        }
    }

    pub fn insert(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: svl_id,
    ) -> AbiResult<()> {
        let b = self.b();
        self.check(unsafe { (b.insert)(b.self_, parent, child, before.unwrap_or(0), slot) })
    }

    pub fn remove(&self, parent: svl_handle, child: svl_handle) -> AbiResult<()> {
        let b = self.b();
        self.check(unsafe { (b.remove)(b.self_, parent, child) })
    }

    pub fn destroy(&self, h: svl_handle) {
        let b = self.b();
        unsafe { (b.destroy)(b.self_, h) }
    }

    pub fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> AbiResult<()> {
        let b = self.b();
        self.check(v.with_raw(|raw| unsafe { (b.set_prop)(b.self_, h, prop, raw) }))
    }

    pub fn get_prop(&self, h: svl_handle, prop: svl_id) -> AbiResult<Value> {
        let b = self.b();
        let mut out = null_value();
        self.check(unsafe { (b.get_prop)(b.self_, h, prop, &mut out) })?;
        Ok(unsafe { Value::from_raw(&out) })
    }

    pub fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> AbiResult<Value> {
        let b = self.b();
        let raw = RawValues::new(args);
        let mut out = null_value();
        self.check(unsafe { (b.call)(b.self_, h, method, raw.as_ptr(), raw.len(), &mut out) })?;
        Ok(unsafe { Value::from_raw(&out) })
    }

    pub fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> AbiResult<()> {
        let b = self.b();
        self.check(unsafe { (b.listen)(b.self_, h, signal, on) })
    }

    /// Runs the backend's loop; returns when the app quits.
    pub fn run(&self, host: &HostVtable) -> AbiResult<()> {
        let b = self.b();
        self.check(unsafe { (b.run)(b.self_, host.as_raw()) })
    }

    /// Thread-safe.
    pub fn wake(&self) {
        let b = self.b();
        unsafe { (b.wake)(b.self_) }
    }

    pub fn quit(&self) {
        let b = self.b();
        unsafe { (b.quit)(b.self_) }
    }

    /// Arms the backend's single one-shot wakeup `ms` from now; `ms < 0` cancels. Main thread only.
    pub fn schedule_wake(&self, ms: i64) {
        let b = self.b();
        unsafe { (b.schedule_wake)(b.self_, ms) }
    }

    /// Kind and lifetime class of a live handle; an error for a dead or foreign handle.
    pub fn handle_info(&self, h: svl_handle) -> AbiResult<svl_handle_info> {
        let b = self.b();
        let mut out = svl_handle_info::default();
        self.check(unsafe { (b.handle_info)(b.self_, h, &mut out) })?;
        Ok(out)
    }

    /// A `Send` handle to `wake`, for timer threads.
    ///
    /// The caller must stop using it before the backend is dropped.
    pub fn wake_handle(&self) -> WakeHandle {
        let b = self.b();
        WakeHandle {
            wake: b.wake,
            self_: b.self_,
        }
    }
}

/// A backend's view of the host callbacks. Main thread only.
#[derive(Clone, Copy)]
pub struct HostRef<'a> {
    raw: *const svl_host,
    _p: PhantomData<&'a svl_host>,
}

impl<'a> HostRef<'a> {
    /// # Safety
    /// `raw` must point to a valid `svl_host` that outlives `'a`.
    pub unsafe fn from_raw(raw: *const svl_host) -> HostRef<'a> {
        HostRef {
            raw,
            _p: PhantomData,
        }
    }

    fn h(&self) -> &svl_host {
        unsafe { &*self.raw }
    }

    /// Queues an event for delivery at the next checkpoint.
    pub fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        let host = self.h();
        let raw = RawValues::new(args);
        unsafe { (host.post_event)(host.self_, h, signal, raw.as_ptr(), raw.len()) }
    }

    /// Runs queued events and JS jobs. See "Checkpoint semantics" in backend-abi.md.
    pub fn checkpoint(&self) {
        let host = self.h();
        unsafe { (host.checkpoint)(host.self_) }
    }

    pub fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> AbiResult<Value> {
        let host = self.h();
        let raw = RawValues::new(args);
        let mut out = null_value();
        let rc =
            unsafe { (host.call_sync)(host.self_, h, signal, raw.as_ptr(), raw.len(), &mut out) };
        if rc < 0 {
            return Err(AbiError {
                code: rc,
                message: "call_sync failed".into(),
            });
        }
        Ok(unsafe { Value::from_raw(&out) })
    }

    /// `reason` is one of the `SVL_GONE_*` constants.
    pub fn on_destroyed(&self, h: svl_handle, reason: u32) {
        let host = self.h();
        unsafe { (host.on_destroyed)(host.self_, h, reason) }
    }
}

pub type Waker = Arc<dyn Fn() + Send + Sync>;
pub type StrResult<T> = std::result::Result<T, String>;

/// Implement this to provide a backend in Rust, then call [`BackendVtable::new`].
///
/// Errors are plain strings; the vtable stores them for `last_error` and returns `-1`.
pub trait Backend: 'static {
    /// Returns 0 for unknown names.
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id;
    fn create(&self, kind: svl_id) -> StrResult<svl_handle>;
    fn insert(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: svl_id,
    ) -> StrResult<()>;
    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()>;
    fn destroy(&self, h: svl_handle);
    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()>;
    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value>;
    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value>;
    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()>;
    /// Kind and lifetime class of a live handle.
    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info>;
    /// Owns the toolkit loop. Returns when the app quits.
    fn run(&self, host: &HostRef<'_>) -> StrResult<()>;
    /// Called once when the vtable is built. The returned closure implements `wake`: it may be
    /// called from any thread and must schedule a host checkpoint on the loop thread.
    fn waker(&self) -> Waker;
    fn quit(&self);
    /// Arms a single one-shot wakeup `ms` from now, replacing any pending one; `ms < 0` cancels.
    /// On expiry the backend calls `host.checkpoint`. Main thread only.
    fn schedule_wake(&self, ms: i64);
}

type OutSlot = RefCell<Option<(Box<Value>, Vec<Vec<svl_value>>)>>;

struct Shim {
    // `wake` reads only this field, possibly from another thread.
    waker: Waker,
    backend: Box<dyn Backend>,
    last_error: RefCell<CString>,
    out: OutSlot,
}

fn write_out(slot: &OutSlot, out: *mut svl_value, v: Value) {
    if out.is_null() {
        return;
    }
    let boxed = Box::new(v);
    let mut lists = Vec::new();
    let raw = build_raw(&boxed, &mut lists);
    *slot.borrow_mut() = Some((boxed, lists));
    unsafe { *out = raw };
}

impl Shim {
    fn fail(&self, msg: &str) -> c_int {
        let cleaned = msg.replace('\0', " ");
        *self.last_error.borrow_mut() = CString::new(cleaned).unwrap_or_default();
        -1
    }

    fn rc(&self, f: impl FnOnce(&dyn Backend) -> StrResult<()>) -> c_int {
        match catch_unwind(AssertUnwindSafe(|| f(&*self.backend))) {
            Ok(Ok(())) => 0,
            Ok(Err(e)) => self.fail(&e),
            Err(_) => self.fail("panic in backend"),
        }
    }
}

unsafe fn shim<'a>(p: *mut c_void) -> &'a Shim {
    unsafe { &*(p as *const Shim) }
}

unsafe extern "C" fn t_resolve(
    s: *mut c_void,
    what: u32,
    kind: svl_id,
    name: *const c_char,
    len: usize,
) -> svl_id {
    let s = unsafe { shim(s) };
    let Some(what) = NameKind::from_u32(what) else {
        return 0;
    };
    let bytes = if name.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(name.cast::<u8>(), len) }
    };
    let Ok(name) = std::str::from_utf8(bytes) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.backend.resolve(what, kind, name))).unwrap_or(0)
}

unsafe extern "C" fn t_create(s: *mut c_void, kind: svl_id) -> svl_handle {
    let s = unsafe { shim(s) };
    match catch_unwind(AssertUnwindSafe(|| s.backend.create(kind))) {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            s.fail(&e);
            0
        }
        Err(_) => {
            s.fail("panic in backend");
            0
        }
    }
}

unsafe extern "C" fn t_insert(
    s: *mut c_void,
    parent: svl_handle,
    child: svl_handle,
    before: svl_handle,
    slot: svl_id,
) -> c_int {
    let s = unsafe { shim(s) };
    s.rc(|b| b.insert(parent, child, (before != 0).then_some(before), slot))
}

unsafe extern "C" fn t_remove(s: *mut c_void, parent: svl_handle, child: svl_handle) -> c_int {
    let s = unsafe { shim(s) };
    s.rc(|b| b.remove(parent, child))
}

unsafe extern "C" fn t_destroy(s: *mut c_void, h: svl_handle) {
    let s = unsafe { shim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.backend.destroy(h)));
}

unsafe extern "C" fn t_set_prop(
    s: *mut c_void,
    h: svl_handle,
    prop: svl_id,
    v: *const svl_value,
) -> c_int {
    let s = unsafe { shim(s) };
    if v.is_null() {
        return s.fail("set_prop: null value");
    }
    let v = unsafe { Value::from_raw(&*v) };
    s.rc(|b| b.set_prop(h, prop, &v))
}

unsafe extern "C" fn t_get_prop(
    s: *mut c_void,
    h: svl_handle,
    prop: svl_id,
    out: *mut svl_value,
) -> c_int {
    let s = unsafe { shim(s) };
    let mut res = None;
    let rc = s.rc(|b| {
        res = Some(b.get_prop(h, prop)?);
        Ok(())
    });
    if let Some(v) = res {
        write_out(&s.out, out, v);
    }
    rc
}

unsafe extern "C" fn t_call(
    s: *mut c_void,
    h: svl_handle,
    method: svl_id,
    args: *const svl_value,
    n: usize,
    out: *mut svl_value,
) -> c_int {
    let s = unsafe { shim(s) };
    let args = unsafe { Value::vec_from_raw(args, n) };
    let mut res = None;
    let rc = s.rc(|b| {
        res = Some(b.call(h, method, &args)?);
        Ok(())
    });
    if let Some(v) = res {
        write_out(&s.out, out, v);
    }
    rc
}

unsafe extern "C" fn t_listen(s: *mut c_void, h: svl_handle, signal: svl_id, on: bool) -> c_int {
    let s = unsafe { shim(s) };
    s.rc(|b| b.listen(h, signal, on))
}

unsafe extern "C" fn t_run(s: *mut c_void, host: *const svl_host) -> c_int {
    let s = unsafe { shim(s) };
    let host = unsafe { HostRef::from_raw(host) };
    s.rc(|b| b.run(&host))
}

unsafe extern "C" fn t_wake(s: *mut c_void) {
    // Touch only the `waker` field: the other fields are not thread-safe.
    let waker: &Waker = unsafe { &*ptr::addr_of!((*(s as *const Shim)).waker) };
    waker();
}

unsafe extern "C" fn t_handle_info(
    s: *mut c_void,
    h: svl_handle,
    out: *mut svl_handle_info,
) -> c_int {
    let s = unsafe { shim(s) };
    if out.is_null() {
        return s.fail("handle_info: null out");
    }
    s.rc(|b| {
        unsafe { *out = b.handle_info(h)? };
        Ok(())
    })
}

unsafe extern "C" fn t_schedule_wake(s: *mut c_void, ms: i64) {
    let s = unsafe { shim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.backend.schedule_wake(ms)));
}

unsafe extern "C" fn t_quit(s: *mut c_void) {
    let s = unsafe { shim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.backend.quit()));
}

unsafe extern "C" fn t_last_error(s: *mut c_void) -> *const c_char {
    let s = unsafe { shim(s) };
    // The CString buffer stays put until the next error replaces it.
    unsafe { (*s.last_error.as_ptr()).as_ptr() }
}

/// An `svl_backend` vtable backed by a [`Backend`] implementation.
///
/// Heap-allocated, so the vtable address is stable when this value moves.
pub struct BackendVtable {
    raw: Box<svl_backend>,
    shim: *mut Shim,
}

impl BackendVtable {
    pub fn new<B: Backend>(backend: B) -> BackendVtable {
        let waker = backend.waker();
        let shim = Box::into_raw(Box::new(Shim {
            waker,
            backend: Box::new(backend),
            last_error: RefCell::new(CString::default()),
            out: RefCell::new(None),
        }));
        let raw = Box::new(svl_backend {
            abi_version: SVL_ABI_VERSION,
            self_: shim.cast(),
            resolve: t_resolve,
            create: t_create,
            insert: t_insert,
            remove: t_remove,
            destroy: t_destroy,
            set_prop: t_set_prop,
            get_prop: t_get_prop,
            call: t_call,
            listen: t_listen,
            handle_info: t_handle_info,
            run: t_run,
            wake: t_wake,
            schedule_wake: t_schedule_wake,
            quit: t_quit,
            last_error: t_last_error,
        });
        BackendVtable { raw, shim }
    }

    pub fn as_raw(&self) -> *const svl_backend {
        &*self.raw
    }

    pub fn backend_ref(&self) -> BackendRef<'_> {
        BackendRef {
            raw: self.as_raw(),
            _p: PhantomData,
        }
    }
}

impl Drop for BackendVtable {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.shim) });
    }
}

/// Implement this to provide the host side of the ABI, then call [`HostVtable::new`].
pub trait HostCallbacks: 'static {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]);
    fn checkpoint(&self);
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> StrResult<Value>;
    fn on_destroyed(&self, h: svl_handle, reason: u32);
}

struct HostShim {
    cb: Box<dyn HostCallbacks>,
    out: OutSlot,
}

unsafe fn hshim<'a>(p: *mut c_void) -> &'a HostShim {
    unsafe { &*(p as *const HostShim) }
}

unsafe extern "C" fn h_post_event(
    s: *mut c_void,
    h: svl_handle,
    sig: svl_id,
    args: *const svl_value,
    n: usize,
) {
    let s = unsafe { hshim(s) };
    let args = unsafe { Value::vec_from_raw(args, n) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.cb.post_event(h, sig, &args)));
}

unsafe extern "C" fn h_checkpoint(s: *mut c_void) {
    let s = unsafe { hshim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.cb.checkpoint()));
}

unsafe extern "C" fn h_call_sync(
    s: *mut c_void,
    h: svl_handle,
    sig: svl_id,
    args: *const svl_value,
    n: usize,
    out: *mut svl_value,
) -> c_int {
    let s = unsafe { hshim(s) };
    let args = unsafe { Value::vec_from_raw(args, n) };
    match catch_unwind(AssertUnwindSafe(|| s.cb.call_sync(h, sig, &args))) {
        Ok(Ok(v)) => {
            write_out(&s.out, out, v);
            0
        }
        _ => -1,
    }
}

unsafe extern "C" fn h_on_destroyed(s: *mut c_void, h: svl_handle, reason: u32) {
    let s = unsafe { hshim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.cb.on_destroyed(h, reason)));
}

/// An `svl_host` backed by a [`HostCallbacks`] implementation.
pub struct HostVtable {
    raw: Box<svl_host>,
    shim: *mut HostShim,
}

impl HostVtable {
    pub fn new<C: HostCallbacks>(cb: C) -> HostVtable {
        let shim = Box::into_raw(Box::new(HostShim {
            cb: Box::new(cb),
            out: RefCell::new(None),
        }));
        let raw = Box::new(svl_host {
            self_: shim.cast(),
            post_event: h_post_event,
            checkpoint: h_checkpoint,
            call_sync: h_call_sync,
            on_destroyed: h_on_destroyed,
        });
        HostVtable { raw, shim }
    }

    pub fn as_raw(&self) -> *const svl_host {
        &*self.raw
    }

    pub fn host_ref(&self) -> HostRef<'_> {
        HostRef {
            raw: self.as_raw(),
            _p: PhantomData,
        }
    }
}

impl Drop for HostVtable {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.shim) });
    }
}
