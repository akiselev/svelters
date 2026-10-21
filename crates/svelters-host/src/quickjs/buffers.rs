//! E2 buffers on QuickJS-NG (`docs/notes/egui-rust-exports.md` §5.4, probe Part A).
//!
//! - **Rust → JS.** An owned buffer becomes an `ArrayBuffer` over the Rust allocation (our free
//!   callback drops it when JS collects the buffer); a shared one becomes an *immutable*
//!   `ArrayBuffer` holding an `Arc` clone (released on GC). A transferred buffer goes back as the very
//!   `ArrayBuffer` it came from. No copy in any case.
//! - **JS → Rust, moved (`Buffer<T>`).** A buffer over Rust memory (one of ours: `alloc`, or a buffer
//!   Rust returned) is *reclaimed*: we detach it, and our free callback hands the allocation back
//!   instead of dropping it. Any other buffer is moved with `ArrayBuffer.prototype.transfer()`: the
//!   caller's buffer is detached, the new one is kept in a table private to the prelude, and Rust gets
//!   a [`TransferToken`] for its memory. Dropping the token (any thread) queues the release; the JS
//!   thread deletes the table entry at the next checkpoint and the engine frees the memory there.
//! - **JS → Rust, read-only (`Shared<[T]>`).** A view we made gives back its `Arc`; anything else is
//!   copied once.
//!
//! Our external buffers are registered by data address in a per-thread table (the engine and its
//! free callbacks run on one thread), which is how a buffer passed back is recognised as ours.

use super::conv::QjsConv;
use crate::exports::buffers::{self, Take};
use crate::exports::offload::Completions;
use rquickjs::qjs;
use rquickjs::{Array, Ctx, Function, Object, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use svelters_abi::{BufData, BufSource, BufferValue, Elem, TransferHome, TransferToken};

type R<T> = Result<T, String>;

// ---- external ArrayBuffers over Rust memory ----

enum ExtSrc {
    Owned(Box<dyn BufSource>),
    Shared(Arc<dyn BufSource>),
}

impl ExtSrc {
    fn ptr(&self) -> *mut u8 {
        match self {
            ExtSrc::Owned(b) => b.as_ptr(),
            ExtSrc::Shared(a) => a.as_ptr(),
        }
    }
    fn byte_len(&self) -> usize {
        match self {
            ExtSrc::Owned(b) => b.byte_len(),
            ExtSrc::Shared(a) => a.byte_len(),
        }
    }
}

/// The opaque of one external `ArrayBuffer`.
struct Ext {
    addr: usize,
    registered: bool,
    src: ExtSrc,
}

/// What the per-thread table knows about a live external buffer.
enum Known {
    /// Rust memory JS owns: reclaimable.
    Owned { byte_len: usize },
    /// Immutable views of one `Arc` (several may be alive; each holds a strong reference).
    Shared { src: std::sync::Weak<dyn BufSource>, views: usize },
}

thread_local! {
    static KNOWN: RefCell<HashMap<usize, Known>> = RefCell::new(HashMap::new());
    /// The address being reclaimed: the free callback keeps that allocation instead of dropping it.
    static RECLAIM: Cell<usize> = const { Cell::new(0) };
    static RECLAIMED: RefCell<Option<Box<dyn BufSource>>> = const { RefCell::new(None) };
}

/// Ids of transferred buffers: process-wide, so a release from another runtime never hits ours.
static NEXT_TRANSFER: AtomicU64 = AtomicU64::new(1);

/// Runtimes leaked at shutdown or reload because a worker still held one of their transferred
/// buffers (freeing them would free memory under the worker).
pub(crate) static LEAKED_RUNTIMES: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn ext_free(_rt: *mut qjs::JSRuntime, opaque: *mut c_void, _ptr: *mut c_void, size: qjs::size_t) -> *mut c_void {
    if size != 0 {
        // A resize (`transfer(newLength)`): not supported for Rust memory.
        return std::ptr::null_mut();
    }
    // SAFETY: `opaque` is the `Box<Ext>` made in `new_external`; QuickJS calls the free once.
    let ext = unsafe { Box::from_raw(opaque as *mut Ext) };
    // A panic must not unwind into C.
    let _ = catch_unwind(AssertUnwindSafe(move || {
        let Ext { addr, registered, src } = *ext;
        if registered {
            let _ = KNOWN.try_with(|k| {
                let mut k = k.borrow_mut();
                let gone = match k.get_mut(&addr) {
                    Some(Known::Shared { views, .. }) => {
                        *views -= 1;
                        *views == 0
                    }
                    Some(Known::Owned { .. }) => true,
                    None => false,
                };
                if gone {
                    k.remove(&addr);
                }
            });
        }
        match src {
            ExtSrc::Owned(b) => {
                if addr != 0 && RECLAIM.try_with(|r| r.get() == addr).unwrap_or(false) {
                    let _ = RECLAIMED.try_with(|r| *r.borrow_mut() = Some(b));
                } else {
                    drop(b);
                }
            }
            ExtSrc::Shared(a) => drop(a),
        }
    }));
    std::ptr::null_mut()
}

fn take_exception(ctx: &Ctx<'_>) -> String {
    let ex = ctx.catch();
    match ex.as_object().and_then(|o| rquickjs::Exception::from_object(o.clone())) {
        Some(e) => e.message().unwrap_or_else(|| "JS exception".into()),
        None => "JS exception".into(),
    }
}

/// An `ArrayBuffer` over `src` (immutable for shared memory). Ours: registered by address.
fn new_external<'js>(ctx: &Ctx<'js>, src: ExtSrc) -> R<Value<'js>> {
    let len = src.byte_len();
    if len > i32::MAX as usize {
        return Err(format!("RangeError: a buffer of {len} bytes exceeds the engine's 2 GiB limit"));
    }
    let ptr = src.ptr();
    let immutable = matches!(src, ExtSrc::Shared(_));
    let addr = ptr as usize;
    // Empty allocations share dangling addresses: never registered (and never worth reclaiming).
    let registered = len > 0;
    if registered {
        KNOWN.with(|k| {
            let mut k = k.borrow_mut();
            match (&src, k.get_mut(&addr)) {
                (ExtSrc::Shared(_), Some(Known::Shared { views, .. })) => *views += 1,
                (ExtSrc::Shared(a), _) => {
                    k.insert(addr, Known::Shared { src: Arc::downgrade(a), views: 1 });
                }
                (ExtSrc::Owned(_), _) => {
                    k.insert(addr, Known::Owned { byte_len: len });
                }
            }
        });
    }
    let opaque = Box::into_raw(Box::new(Ext { addr, registered, src })) as *mut c_void;
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: the memory stays valid until `ext_free` runs (the source lives in the opaque).
    let val = unsafe { qjs::JS_NewArrayBuffer(raw, ptr, len as _, 0, Some(ext_free), opaque, false) };
    if unsafe { qjs::JS_IsException(val) } {
        // SAFETY: QuickJS did not take the opaque; free it as the engine would have.
        unsafe { ext_free(std::ptr::null_mut(), opaque, ptr.cast(), 0) };
        return Err(take_exception(ctx));
    }
    if immutable {
        // SAFETY: `val` is the ArrayBuffer just made.
        unsafe { qjs::JS_SetImmutableArrayBuffer(val, true) };
    }
    // SAFETY: we own the new reference.
    Ok(unsafe { Value::from_raw(ctx.clone(), val) })
}

fn ta_type(elem: Elem) -> qjs::JSTypedArrayEnum {
    match elem {
        Elem::U8 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT8,
        Elem::I8 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT8,
        Elem::U16 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT16,
        Elem::I16 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT16,
        Elem::U32 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT32,
        Elem::I32 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT32,
        Elem::U64 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_BIG_UINT64,
        Elem::I64 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_BIG_INT64,
        Elem::F32 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_FLOAT32,
        Elem::F64 => qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_FLOAT64,
    }
}

/// `new Float32Array(ab, byteOffset, length)` with the intrinsic constructor.
fn typed_array<'js>(ctx: &Ctx<'js>, ab: &Value<'js>, elem: Elem, byte_offset: usize, len: usize) -> R<Value<'js>> {
    let off = Value::new_float(ctx.clone(), byte_offset as f64);
    let n = Value::new_float(ctx.clone(), len as f64);
    let mut argv = [ab.as_raw(), off.as_raw(), n.as_raw()];
    // SAFETY: the arguments are live values; the result is a new reference we own.
    let val = unsafe { qjs::JS_NewTypedArray(ctx.as_raw().as_ptr(), 3, argv.as_mut_ptr(), ta_type(elem)) };
    if unsafe { qjs::JS_IsException(val) } {
        return Err(take_exception(ctx));
    }
    Ok(unsafe { Value::from_raw(ctx.clone(), val) })
}

/// A typed array over Rust memory (`alloc(Float32Array, n)` from `@svelters/types/core`): passing it
/// as a `Buffer<T>` gives Rust the allocation back.
pub(crate) fn alloc<'js>(ctx: &Ctx<'js>, home: &QjsHome, elem: Elem, n: usize) -> R<Value<'js>> {
    if n.checked_mul(elem.size()).is_none_or(|b| b > i32::MAX as usize) {
        return Err(format!("RangeError: alloc({}, {n}) exceeds the engine's 2 GiB limit", elem.js_name()));
    }
    let ab = new_external(ctx, ExtSrc::Owned(buffers::owned_zeroed(elem, n)))?;
    home.stats.owned_out.fetch_add(1, Ordering::Relaxed);
    typed_array(ctx, &ab, elem, 0, n)
}

// ---- transfer tokens ----

/// Counters of the buffer paths (see [`crate::Host::buffer_stats`]).
#[derive(Default)]
pub(crate) struct Counters {
    pub transferred: AtomicU64,
    pub reclaimed: AtomicU64,
    pub returned: AtomicU64,
    pub released: AtomicU64,
    pub owned_out: AtomicU64,
    pub shared_out: AtomicU64,
    pub shared_in: AtomicU64,
    pub copies: AtomicU64,
    pub copied_bytes: AtomicU64,
}

/// The engine side of the transfer tokens of one runtime: how many are alive (memory a worker may
/// be using) and where a dropped token's release goes.
pub(crate) struct QjsHome {
    pub live: AtomicUsize,
    queue: Arc<Completions>,
    pub stats: Counters,
}

impl QjsHome {
    pub(crate) fn new(queue: Arc<Completions>) -> Arc<QjsHome> {
        Arc::new(QjsHome { live: AtomicUsize::new(0), queue, stats: Counters::default() })
    }

    pub(crate) fn snapshot(&self) -> crate::engine::BufferStats {
        let s = &self.stats;
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        crate::engine::BufferStats {
            transferred: g(&s.transferred),
            reclaimed: g(&s.reclaimed),
            returned: g(&s.returned),
            released: g(&s.released),
            owned_out: g(&s.owned_out),
            shared_out: g(&s.shared_out),
            shared_in: g(&s.shared_in),
            copies: g(&s.copies),
            copied_bytes: g(&s.copied_bytes),
            tokens_live: self.live.load(Ordering::Acquire) as u64,
            leaked_runtimes: LEAKED_RUNTIMES.load(Ordering::Relaxed) as u64,
        }
    }
}

impl TransferHome for QjsHome {
    fn token_gone(&self, id: u64, returned: bool) {
        if !returned {
            // The memory is freed on the JS thread: the next checkpoint deletes the table entry.
            self.queue.push_release(id);
        }
        self.live.fetch_sub(1, Ordering::AcqRel);
    }
}

// ---- the converter's buffer operations ----

/// A typed array checked by `stage_buffer`, moved by `commit_buffers`.
pub(crate) struct Staged<'js> {
    ab: Value<'js>,
    elem: Elem,
    take: Take,
    byte_offset: usize,
    len: usize,
}

impl<'js> QjsConv<'js> {
    fn internal_fn(&self, name: &str) -> R<Function<'js>> {
        let internal: Object = self.ctx.globals().get("__svelters_internal").map_err(|e| e.to_string())?;
        internal.get(name).map_err(|e| e.to_string())
    }

    pub(crate) fn stage(&self, v: &Value<'js>, elem: Elem, take: Take) -> R<usize> {
        // SAFETY: reads the class of a live value.
        let t = unsafe { qjs::JS_GetTypedArrayType(v.as_raw()) };
        let want = ta_type(elem) as i32;
        if t != want {
            let got = if t >= 0 {
                "a typed array of another element type".to_string()
            } else if v.is_object() && unsafe { qjs::JS_IsArrayBuffer(v.as_raw()) } {
                "an ArrayBuffer (wrap it: new ".to_string() + elem.js_name() + "(buffer))"
            } else {
                "a non-typed-array value".to_string()
            };
            return Err(format!("expected {}, got {got}", elem.js_name()));
        }
        let info: Array = self.internal_fn("viewInfo")?.call((v.clone(),)).map_err(|e| self.rethrow(e))?;
        let ab: Value = info.get(0).map_err(|e| e.to_string())?;
        let byte_offset: f64 = info.get(1).map_err(|e| e.to_string())?;
        let len: f64 = info.get(2).map_err(|e| e.to_string())?;
        let state: i32 = info.get(3).map_err(|e| e.to_string())?;
        let what = elem.js_name();
        match state {
            1 => return Err(format!("the {what} is detached: it was moved to Rust by an earlier call (a moved array cannot be used again)")),
            3 => return Err(format!("a {what} over a SharedArrayBuffer cannot cross to Rust")),
            4 if take == Take::Move => return Err(format!("a {what} over a resizable ArrayBuffer cannot be moved to Rust")),
            _ => {}
        }
        // SAFETY: `ab` is a live ArrayBuffer.
        let immutable = unsafe { qjs::JS_IsImmutableArrayBuffer(ab.as_raw()) } == 1;
        if take == Take::Move && immutable {
            return Err(format!("a read-only {what} (a Shared view) cannot be moved: pass a copy (view.slice())"));
        }
        let mut staged = self.staged.borrow_mut();
        if take == Take::Move && staged.iter().any(|s| s.take == Take::Move && s.ab == ab) {
            return Err(format!("the same ArrayBuffer is moved twice in one call (each {what} argument must own its buffer)"));
        }
        staged.push(Staged { ab, elem, take, byte_offset: byte_offset as usize, len: len as usize });
        Ok(staged.len() - 1)
    }

    pub(crate) fn commit(&self) -> R<Vec<BufferValue>> {
        let staged = std::mem::take(&mut *self.staged.borrow_mut());
        let mut out = Vec::with_capacity(staged.len());
        for s in staged {
            out.push(match s.take {
                Take::Move => self.commit_move(s)?,
                Take::View => self.commit_view(s)?,
            });
        }
        Ok(out)
    }

    /// The memory of a live, non-detached ArrayBuffer.
    fn ab_memory(&self, ab: &Value<'js>) -> R<(*mut u8, usize)> {
        let mut size: qjs::size_t = 0;
        // SAFETY: `ab` is a live ArrayBuffer (checked not detached at staging, and no JS ran since
        // except our own prelude helpers).
        let p = unsafe { qjs::JS_GetArrayBuffer(self.ctx.as_raw().as_ptr(), &mut size, ab.as_raw()) };
        if p.is_null() && unsafe { qjs::JS_HasException(self.ctx.as_raw().as_ptr()) } {
            return Err(take_exception(&self.ctx));
        }
        Ok((p, size as usize))
    }

    fn home(&self) -> R<&Arc<QjsHome>> {
        self.cbs.as_ref().map(|c| &c.home).ok_or_else(|| "typed arrays cannot be moved to Rust here".to_string())
    }

    fn commit_move(&self, s: Staged<'js>) -> R<BufferValue> {
        let home = self.home()?.clone();
        let (ptr, size) = self.ab_memory(&s.ab)?;
        let addr = ptr as usize;
        let ours = size > 0 && KNOWN.with(|k| matches!(k.borrow().get(&addr), Some(Known::Owned { byte_len }) if *byte_len == size));
        if ours {
            // Rust memory: detach the buffer and keep the allocation our free callback hands back.
            RECLAIM.with(|r| r.set(addr));
            // SAFETY: a live, non-detached ArrayBuffer of ours; the detach runs `ext_free` at once.
            unsafe { qjs::JS_DetachArrayBuffer(self.ctx.as_raw().as_ptr(), s.ab.as_raw()) };
            RECLAIM.with(|r| r.set(0));
            return match RECLAIMED.with(|r| r.borrow_mut().take()) {
                Some(src) => {
                    home.stats.reclaimed.fetch_add(1, Ordering::Relaxed);
                    Ok(BufferValue::new(s.elem, BufData::Owned(src), s.byte_offset, s.len))
                }
                None => Err("internal: a Rust-backed buffer could not be reclaimed".into()),
            };
        }
        // A JS buffer: `transfer()` moves its memory to a new ArrayBuffer only the prelude's table holds.
        let id = NEXT_TRANSFER.fetch_add(1, Ordering::Relaxed);
        let moved: Value = self.internal_fn("transferIn")?.call((s.ab.clone(), id as f64)).map_err(|e| self.rethrow(e))?;
        let (ptr, size) = self.ab_memory(&moved)?;
        home.live.fetch_add(1, Ordering::AcqRel);
        home.stats.transferred.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the table keeps the ArrayBuffer (and its memory) alive and out of JS's reach until
        // `token_gone` queues its release; the release runs on this thread at a checkpoint.
        let token = unsafe { TransferToken::new(id, ptr, size, home) };
        Ok(BufferValue::new(s.elem, BufData::Transferred(token), s.byte_offset, s.len))
    }

    fn commit_view(&self, s: Staged<'js>) -> R<BufferValue> {
        let (ptr, size) = self.ab_memory(&s.ab)?;
        let addr = ptr as usize;
        let known = if size > 0 {
            KNOWN.with(|k| match k.borrow().get(&addr) {
                Some(Known::Shared { src, .. }) => src.upgrade(),
                _ => None,
            })
        } else {
            None
        };
        let stats = self.cbs.as_ref().map(|c| &c.home.stats);
        if let Some(src) = known {
            if let Some(st) = stats {
                st.shared_in.fetch_add(1, Ordering::Relaxed);
            }
            return Ok(BufferValue::new(s.elem, BufData::Shared(src), s.byte_offset, s.len));
        }
        // Not ours: a snapshot (the caller keeps its array).
        let nbytes = s.len * s.elem.size();
        // SAFETY: the view's range lies inside the live buffer (the engine checked it at creation).
        let bytes = if nbytes == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(ptr.add(s.byte_offset), nbytes) } };
        if let Some(st) = stats {
            st.copies.fetch_add(1, Ordering::Relaxed);
            st.copied_bytes.fetch_add(nbytes as u64, Ordering::Relaxed);
        }
        Ok(BufferValue::whole(s.elem, BufData::Shared(buffers::shared_copy(s.elem, bytes))))
    }

    fn rethrow(&self, e: rquickjs::Error) -> String {
        match e {
            rquickjs::Error::Exception => take_exception(&self.ctx),
            other => other.to_string(),
        }
    }

    /// `JsWriter::buffer`: a typed array over the buffer's memory.
    pub(crate) fn write_buffer(&self, b: &BufferValue) -> R<Value<'js>> {
        let ctx = &self.ctx;
        let (elem, off, len) = (b.elem(), b.offset(), b.len());
        if let Some(src) = b.shared() {
            let ab = new_external(ctx, ExtSrc::Shared(src))?;
            bump(self, |s| &s.shared_out);
            return typed_array(ctx, &ab, elem, off, len);
        }
        match b.take() {
            Some(BufData::Owned(src)) => {
                let ab = new_external(ctx, ExtSrc::Owned(src))?;
                bump(self, |s| &s.owned_out);
                typed_array(ctx, &ab, elem, off, len)
            }
            Some(BufData::Transferred(token)) => {
                // The same ArrayBuffer again, if it is in this runtime's table.
                let back: Value = self.internal_fn("transferOut")?.call((token.id() as f64,)).map_err(|e| self.rethrow(e))?;
                if back.is_object() {
                    token.into_returned();
                    bump(self, |s| &s.returned);
                    return typed_array(ctx, &back, elem, off, len);
                }
                // From a runtime that is gone (a reload): copy, and release the old memory.
                let v = BufferValue::new(elem, BufData::Transferred(token), off, len);
                let ab = v.with_bytes(|bytes| {
                    // SAFETY: copies `bytes` into a new JS-owned buffer.
                    let raw = unsafe { qjs::JS_NewArrayBufferCopy(ctx.as_raw().as_ptr(), bytes.as_ptr(), bytes.len() as _) };
                    if unsafe { qjs::JS_IsException(raw) } { Err(take_exception(ctx)) } else { Ok(unsafe { Value::from_raw(ctx.clone(), raw) }) }
                })?;
                bump(self, |s| &s.copies);
                typed_array(ctx, &ab, elem, 0, len)
            }
            Some(BufData::Shared(_)) => unreachable!("shared payloads are never taken"),
            None => Err("the buffer was handed to JS already".into()),
        }
    }
}

fn bump(c: &QjsConv<'_>, f: impl FnOnce(&Counters) -> &AtomicU64) {
    if let Some(cb) = &c.cbs {
        f(&cb.home.stats).fetch_add(1, Ordering::Relaxed);
    }
}

/// Deletes released transfers from the prelude's table (the JS thread frees the memory).
pub(crate) fn release(ctx: &Ctx<'_>, home: &QjsHome, ids: &[u64]) -> rquickjs::Result<()> {
    let internal: Object = ctx.globals().get("__svelters_internal")?;
    let f: Function = internal.get("transferDrop")?;
    let arr = Array::new(ctx.clone())?;
    for (i, id) in ids.iter().enumerate() {
        arr.set(i, *id as f64)?;
    }
    let n: u32 = f.call((arr,))?;
    home.stats.released.fetch_add(n as u64, Ordering::Relaxed);
    Ok(())
}
