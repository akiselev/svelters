//! `Value::Buffer` (E2, `docs/notes/egui-rust-exports.md` §5.4): a typed array that crosses without a
//! copy between the host's Rust exports and the JS engine.
//!
//! Only the host and Rust code see this variant. On the raw `svl_value` path (a C backend) it
//! degrades to `BYTES` borrowed from the same memory, so the C ABI does not change.
//!
//! A [`BufferValue`] is one of:
//!
//! - **owned** ([`BufData::Owned`]): Rust memory JS may get exclusive access to (an `ArrayBuffer`
//!   over the allocation, freed when JS collects it);
//! - **shared** ([`BufData::Shared`]): immutable memory behind an `Arc`; JS gets a read-only view,
//!   and the `Arc` is released when JS collects it;
//! - **transferred** ([`BufData::Transferred`]): a JS buffer moved out of JS (`ArrayBuffer.prototype
//!   .transfer()`); the memory belongs to the JS engine, which frees it on its own thread after the
//!   [`TransferToken`] is dropped or handed back.
//!
//! Cloning a `BufferValue` is cheap and shares the payload: whichever clone is converted first takes
//! an owned or transferred payload (a shared one is never taken).

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The element type of a typed array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Elem {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl Elem {
    pub const ALL: [Elem; 10] = [Elem::U8, Elem::I8, Elem::U16, Elem::I16, Elem::U32, Elem::I32, Elem::U64, Elem::I64, Elem::F32, Elem::F64];

    /// Bytes per element.
    pub fn size(self) -> usize {
        match self {
            Elem::U8 | Elem::I8 => 1,
            Elem::U16 | Elem::I16 => 2,
            Elem::U32 | Elem::I32 | Elem::F32 => 4,
            Elem::U64 | Elem::I64 | Elem::F64 => 8,
        }
    }

    /// The JS typed array class (`Float32Array`).
    pub fn js_name(self) -> &'static str {
        match self {
            Elem::U8 => "Uint8Array",
            Elem::I8 => "Int8Array",
            Elem::U16 => "Uint16Array",
            Elem::I16 => "Int16Array",
            Elem::U32 => "Uint32Array",
            Elem::I32 => "Int32Array",
            Elem::U64 => "BigUint64Array",
            Elem::I64 => "BigInt64Array",
            Elem::F32 => "Float32Array",
            Elem::F64 => "Float64Array",
        }
    }

    /// The element type of a typed array class name.
    pub fn from_js_name(name: &str) -> Option<Elem> {
        Elem::ALL.into_iter().find(|e| e.js_name() == name)
    }

    /// A small stable code (0..10).
    pub fn code(self) -> u32 {
        Elem::ALL.iter().position(|e| *e == self).unwrap_or(0) as u32
    }

    pub fn from_code(c: u32) -> Option<Elem> {
        Elem::ALL.get(c as usize).copied()
    }
}

/// Memory that can back a JS `ArrayBuffer`.
///
/// # Safety
///
/// `as_ptr()` must return a pointer valid for reads (and, for an owned source, writes) of
/// `byte_len()` bytes until the source is dropped, and moving the source must not move the memory.
pub unsafe trait BufSource: Send + Sync + 'static {
    fn as_ptr(&self) -> *mut u8;
    fn byte_len(&self) -> usize;
    /// For downcasts back to the concrete source (`Vec<T>`, `Arc<[T]>` wrappers).
    fn as_any(&self) -> &dyn Any;
}

/// Where a [`TransferToken`] reports that it is gone: implemented by the engine that made it.
pub trait TransferHome: Send + Sync {
    /// The token `id` was dropped (on any thread): its memory may be freed, on the JS thread.
    /// `returned` is true when the token was consumed to hand the memory back to JS instead.
    fn token_gone(&self, id: u64, returned: bool);
}

/// A JS buffer moved out of JS: the memory stays owned by the engine (pinned, unreachable from JS)
/// until the token is dropped or handed back. `Send`: a worker may use the memory, but never frees
/// it; dropping the token only posts the release to the JS thread's queue.
pub struct TransferToken {
    id: u64,
    ptr: *mut u8,
    byte_len: usize,
    home: Option<Arc<dyn TransferHome>>,
}

// SAFETY: the memory is reachable from no JS value while the token lives (the engine keeps it in a
// table only it reads), so the token is its only user; the engine frees it only after `token_gone`.
unsafe impl Send for TransferToken {}
unsafe impl Sync for TransferToken {}

impl TransferToken {
    /// # Safety
    /// `ptr` must stay valid for `byte_len` bytes of reads and writes, and unused by anything else,
    /// until `home.token_gone(id, _)` is called.
    pub unsafe fn new(id: u64, ptr: *mut u8, byte_len: usize, home: Arc<dyn TransferHome>) -> TransferToken {
        TransferToken { id, ptr, byte_len, home: Some(home) }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Consumes the token to hand its memory back to JS (the engine restores the buffer).
    pub fn into_returned(mut self) -> u64 {
        if let Some(h) = self.home.take() {
            h.token_gone(self.id, true);
        }
        self.id
    }
}

impl Drop for TransferToken {
    fn drop(&mut self) {
        if let Some(h) = self.home.take() {
            h.token_gone(self.id, false);
        }
    }
}

impl fmt::Debug for TransferToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransferToken#{}({} bytes)", self.id, self.byte_len)
    }
}

/// The memory behind a [`BufferValue`].
pub enum BufData {
    /// Rust memory; JS gets exclusive access.
    Owned(Box<dyn BufSource>),
    /// Immutable memory; JS gets a read-only view.
    Shared(Arc<dyn BufSource>),
    /// A JS buffer moved out of JS.
    Transferred(TransferToken),
}

impl BufData {
    pub fn as_ptr(&self) -> *mut u8 {
        match self {
            BufData::Owned(b) => b.as_ptr(),
            BufData::Shared(a) => a.as_ptr(),
            BufData::Transferred(t) => t.as_ptr(),
        }
    }

    pub fn byte_len(&self) -> usize {
        match self {
            BufData::Owned(b) => b.byte_len(),
            BufData::Shared(a) => a.byte_len(),
            BufData::Transferred(t) => t.byte_len(),
        }
    }

    pub fn mode(&self) -> BufMode {
        match self {
            BufData::Owned(_) => BufMode::Owned,
            BufData::Shared(_) => BufMode::Shared,
            BufData::Transferred(_) => BufMode::Transferred,
        }
    }
}

/// [`BufData`]'s variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufMode {
    Owned,
    Shared,
    Transferred,
}

struct Inner {
    elem: Elem,
    /// Byte offset of the first element in the source.
    offset: usize,
    /// Elements.
    len: usize,
    mode: BufMode,
    data: Mutex<Option<BufData>>,
}

/// The payload of `Value::Buffer`: `len` elements of type `elem` at byte `offset` of the memory.
#[derive(Clone)]
pub struct BufferValue(Arc<Inner>);

impl BufferValue {
    /// A buffer over `data`, `len` elements from byte `offset`. Panics when the range does not fit
    /// the memory.
    pub fn new(elem: Elem, data: BufData, offset: usize, len: usize) -> BufferValue {
        let end = len.checked_mul(elem.size()).and_then(|n| n.checked_add(offset));
        assert!(end.is_some_and(|e| e <= data.byte_len()), "buffer range {offset}+{len}x{} exceeds {} bytes", elem.size(), data.byte_len());
        BufferValue(Arc::new(Inner { elem, offset, len, mode: data.mode(), data: Mutex::new(Some(data)) }))
    }

    /// A buffer over all of `data`.
    pub fn whole(elem: Elem, data: BufData) -> BufferValue {
        let len = data.byte_len() / elem.size();
        BufferValue::new(elem, data, 0, len)
    }

    pub fn elem(&self) -> Elem {
        self.0.elem
    }

    /// Byte offset of the first element.
    pub fn offset(&self) -> usize {
        self.0.offset
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    pub fn byte_len(&self) -> usize {
        self.0.len * self.0.elem.size()
    }

    pub fn mode(&self) -> BufMode {
        self.0.mode
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<BufData>> {
        self.0.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes the payload (owned and transferred buffers move to exactly one taker). `None` once
    /// taken.
    pub fn take(&self) -> Option<BufData> {
        self.lock().take()
    }

    /// A shared payload without taking it (`None` for other modes or once taken).
    pub fn shared(&self) -> Option<Arc<dyn BufSource>> {
        match &*self.lock() {
            Some(BufData::Shared(a)) => Some(a.clone()),
            _ => None,
        }
    }

    /// True once an owned or transferred payload was taken.
    pub fn is_taken(&self) -> bool {
        self.lock().is_none()
    }

    /// Runs `f` with the bytes (empty once taken).
    pub fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        let g = self.lock();
        match &*g {
            Some(d) if self.0.len > 0 => {
                // SAFETY: the source keeps the memory valid while the guard holds it; the range was
                // checked at construction.
                let s = unsafe { std::slice::from_raw_parts(d.as_ptr().add(self.0.offset), self.byte_len()) };
                f(s)
            }
            _ => f(&[]),
        }
    }

    /// The bytes as a raw range, valid while this value (and its payload) lives untaken.
    pub(crate) fn raw_range(&self) -> (*const u8, usize) {
        let g = self.lock();
        match &*g {
            Some(d) if self.0.len > 0 => (unsafe { d.as_ptr().add(self.0.offset) } as *const u8, self.byte_len()),
            _ => (std::ptr::null(), 0),
        }
    }
}

impl fmt::Debug for BufferValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Buffer({:?} {} x {:?}{})", self.0.mode, self.0.len, self.0.elem, if self.is_taken() { ", taken" } else { "" })
    }
}

impl PartialEq for BufferValue {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        self.elem() == other.elem() && self.len() == other.len() && self.with_bytes(|a| other.with_bytes(|b| a == b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SVL_BYTES, Value};

    struct Bytes(Vec<u8>);
    unsafe impl BufSource for Bytes {
        fn as_ptr(&self) -> *mut u8 {
            self.0.as_ptr() as *mut u8
        }
        fn byte_len(&self) -> usize {
            self.0.len()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct Home(Mutex<Vec<(u64, bool)>>);
    impl TransferHome for Home {
        fn token_gone(&self, id: u64, returned: bool) {
            self.0.lock().unwrap().push((id, returned));
        }
    }

    #[test]
    fn raw_path_borrows_the_same_bytes() {
        let src = Bytes((0u8..16).collect());
        let addr = src.0.as_ptr() as usize;
        let v = Value::Buffer(BufferValue::new(Elem::U16, BufData::Owned(Box::new(src)), 4, 3));
        v.with_raw(|r| {
            assert_eq!(r.tag, SVL_BYTES);
            let b = unsafe { r.u.bytes };
            assert_eq!((b.ptr as usize, b.len), (addr + 4, 6), "BYTES over the buffer's own memory");
        });
        // Clones share the payload; a taken payload degrades to empty bytes.
        let Value::Buffer(b) = &v else { unreachable!() };
        let clone = b.clone();
        assert!(matches!(clone.take(), Some(BufData::Owned(_))));
        assert!(b.is_taken() && b.take().is_none());
        v.with_raw(|r| assert_eq!(unsafe { r.u.bytes }.len, 0));
    }

    #[test]
    fn shared_payloads_are_never_taken_and_compare_by_content() {
        let a = BufferValue::whole(Elem::U8, BufData::Shared(Arc::new(Bytes(vec![1, 2, 3]))));
        let b = BufferValue::whole(Elem::U8, BufData::Shared(Arc::new(Bytes(vec![1, 2, 3]))));
        assert_eq!(a, b);
        assert!(a.shared().is_some() && a.shared().is_some());
        assert_eq!(a.len(), 3);
        assert_eq!(format!("{a:?}"), "Buffer(Shared 3 x U8)");
    }

    #[test]
    fn tokens_report_drop_and_return() {
        let home = Arc::new(Home(Mutex::new(Vec::new())));
        let mut mem = [0u8; 8];
        let t1 = unsafe { TransferToken::new(1, mem.as_mut_ptr(), 8, home.clone()) };
        let t2 = unsafe { TransferToken::new(2, mem.as_mut_ptr(), 8, home.clone()) };
        std::thread::spawn(move || drop(t1)).join().unwrap();
        assert_eq!(t2.into_returned(), 2);
        assert_eq!(*home.0.lock().unwrap(), vec![(1, false), (2, true)]);
    }

    #[test]
    #[should_panic(expected = "exceeds")]
    fn ranges_are_checked() {
        let _ = BufferValue::new(Elem::F32, BufData::Owned(Box::new(Bytes(vec![0; 8]))), 4, 2);
    }

    #[test]
    fn elem_names_round_trip() {
        for e in Elem::ALL {
            assert_eq!(Elem::from_js_name(e.js_name()), Some(e));
            assert_eq!(Elem::from_code(e.code()), Some(e));
        }
    }
}
