//! Dynamic WinRT delegates (Windows only): one COM object shape for every delegate type the bound surface uses,
//! answering `QueryInterface` for the delegate's IID (computed by the generator, including parameterized ones such as
//! `TypedEventHandler<DispatcherQueueTimer, Object>` or `AsyncOperationCompletedHandler<StorageFile>`).
//!
//! - **Events** (`Target::Event`): the handler captures the handle and the event id, never a COM reference to the
//!   source or a JS value. Arguments are converted on the UI thread; a handler WinRT calls on another thread copies
//!   them (a reference per object, a copy per string) and hops through the UI `DispatcherQueue` first (section 4.1
//!   rule 4).
//! - **Async completions** (`Target::Completion`): always hop to the UI thread before anything is converted (a
//!   completion can also run synchronously inside `put_Completed`, which must not re-enter the host). Then
//!   `GetResults` (its failure carries the operation's error info), `IAsyncInfo.Close`, and the host's one-shot
//!   callback.
//!
//! The objects are agile (`IAgileObject`, plus the free-threaded marshaler for `IMarshal`), since WinRT keeps
//! delegates of agile sources in agile references.
use super::invoke::MAX_DELEGATE_ARGS;
use super::model::TyId;
use crate::bindings::Microsoft::UI::Dispatching::{DispatcherQueue, DispatcherQueueHandler};
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use windows_core::{GUID, HRESULT, IUnknown, Interface};

/// Where a delegate invocation goes.
pub enum Target {
    /// Post event `ev` of handle `h` with the converted arguments (`args`: the Invoke parameter types).
    Event { h: u64, ev: u32, args: &'static [TyId] },
    /// Settle the host's async callback `done` with the operation's result of type `of` (`GetResults` at `results`).
    Completion { done: u64, of: TyId, results: u16 },
}

#[repr(C)]
struct Vtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    invoke: *const c_void,
}

struct Vtbls([Vtbl; MAX_DELEGATE_ARGS + 1]);
// SAFETY: immutable function pointers.
unsafe impl Sync for Vtbls {}
unsafe impl Send for Vtbls {}

fn vtbls() -> &'static Vtbls {
    static V: OnceLock<Vtbls> = OnceLock::new();
    V.get_or_init(|| {
        let thunks = crate::__svl_delegate_thunks!(forward);
        Vtbls(thunks.map(|invoke| Vtbl { query_interface, add_ref, release, invoke }))
    })
}

#[repr(C)]
struct Delegate {
    vtbl: *const Vtbl,
    refs: AtomicU32,
    iid: GUID,
    ui: std::thread::ThreadId,
    queue: DispatcherQueue,
    target: Target,
}

const IAGILE_OBJECT: GUID = GUID::from_u128(0x94ea2b94_e9cc_49e0_c0ff_ee64ca8f5b90);

unsafe extern "system" fn query_interface(this: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() || iid.is_null() {
        return HRESULT(0x80004003u32 as i32); // E_POINTER
    }
    // SAFETY: `this` is a Delegate (it is only reachable through our vtables).
    let d = unsafe { &*(this as *const Delegate) };
    let iid = unsafe { *iid };
    if iid == IUnknown::IID || iid == IAGILE_OBJECT || iid == d.iid {
        d.refs.fetch_add(1, Ordering::Relaxed);
        unsafe { *out = this };
        return HRESULT(0);
    }
    if iid == windows_core::imp::IMarshal::IID {
        d.refs.fetch_add(1, Ordering::Relaxed);
        // SAFETY: we hand the marshaler an owned reference to ourselves as its outer object.
        let outer = unsafe { IUnknown::from_raw(this) };
        return unsafe { windows_core::imp::marshaler(outer, out) };
    }
    unsafe { *out = std::ptr::null_mut() };
    HRESULT(0x80004002u32 as i32) // E_NOINTERFACE
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let d = unsafe { &*(this as *const Delegate) };
    d.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let d = unsafe { &*(this as *const Delegate) };
    let n = d.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if n == 0 {
        // SAFETY: the last reference: the Box made in `new` is ours again.
        drop(unsafe { Box::from_raw(this as *mut Delegate) });
    }
    n
}

/// A new delegate (one owned reference) for `iid` whose `Invoke` takes `nargs` integer-class arguments.
pub fn new(iid: GUID, nargs: usize, queue: DispatcherQueue, target: Target) -> Result<IUnknown, String> {
    if nargs > MAX_DELEGATE_ARGS {
        return Err(format!("a delegate with {nargs} arguments (at most {MAX_DELEGATE_ARGS})"));
    }
    let d = Box::new(Delegate { vtbl: &vtbls().0[nargs], refs: AtomicU32::new(1), iid, ui: std::thread::current().id(), queue, target });
    // SAFETY: a COM object whose first field is its vtable pointer; one reference.
    Ok(unsafe { IUnknown::from_raw(Box::into_raw(d) as *mut c_void) })
}

/// A value that may cross to the UI thread: owned COM references and copied strings only.
struct Hop<T>(T);
// SAFETY: the captured COM objects are agile (WinRT runtime classes and our delegates) or plain data.
unsafe impl<T> Send for Hop<T> {}

fn post(queue: &DispatcherQueue, f: impl FnOnce() + 'static) {
    let cell = std::sync::Mutex::new(Some(Hop(Box::new(f) as Box<dyn FnOnce()>)));
    let h = DispatcherQueueHandler::new(move || {
        if let Some(Hop(f)) = cell.lock().ok().and_then(|mut c| c.take()) {
            f();
        }
        Ok(())
    });
    if !queue.TryEnqueue(&h).unwrap_or(false) {
        eprintln!("svelters-winui: the UI dispatcher queue is shut down; a delegate invocation was dropped");
    }
}


unsafe fn forward(this: *mut c_void, args: &[u64]) -> i32 {
    // SAFETY: `this` is a live Delegate (WinRT holds a reference while it invokes).
    let d = unsafe { &*(this as *const Delegate) };
    match &d.target {
        Target::Event { h, ev, args: tys } => {
            let (h, ev, tys) = (*h, *ev, *tys);
            if std::thread::current().id() == d.ui {
                super::ops::deliver_event(h, ev, tys, args);
            } else {
                let caps: Vec<Part> = tys.iter().zip(args).map(|(t, w)| unsafe { capture(*t, *w) }).collect();
                let caps = Hop(caps);
                post(&d.queue, move || super::ops::deliver_captured(h, ev, tys, caps.0));
            }
        }
        Target::Completion { done, of, results } => {
            let (done, of, results) = (*done, *of, *results);
            let op = args.first().and_then(|p| {
                let p = *p as usize as *mut c_void;
                // SAFETY: the asyncInfo argument: add our own reference for the hop.
                unsafe { IUnknown::from_raw_borrowed(&p) }.cloned()
            });
            let status = args.get(1).copied().unwrap_or(0) as u32 as i32;
            let op = Hop(op);
            post(&d.queue, move || super::ops::complete(done, of, results, op.0, status));
        }
    }
    0
}

/// An argument captured off the UI thread (owned references and copies), converted after the hop.
pub enum Part {
    Word(u64),
    Bytes(Vec<u8>),
    Obj(Option<IUnknown>),
    Str(String),
}

unsafe fn capture(t: TyId, w: u64) -> Part {
    use super::model::Ty;
    use super::value::{is_plain, size_align, struct_in_register, ty};
    let tt = ty(t);
    if is_plain(tt) {
        let (size, _) = size_align(tt);
        if matches!(tt, Ty::Record(_) | Ty::Guid) && !struct_in_register(size) {
            // SAFETY: passed by pointer, valid during Invoke.
            return Part::Bytes(unsafe { std::slice::from_raw_parts(w as usize as *const u8, size) }.to_vec());
        }
        return Part::Word(w);
    }
    if tt == Ty::Str {
        return Part::Str(unsafe { super::rt::copy_hstring(w as usize as *mut c_void) });
    }
    let p = w as usize as *mut c_void;
    Part::Obj(unsafe { IUnknown::from_raw_borrowed(&p) }.cloned())
}
