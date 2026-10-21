//! Buffers (E2, `docs/notes/egui-rust-exports.md` §5.4, §6.3): typed arrays that cross without a copy.
//!
//! | Rust | JS | TS | |
//! | --- | --- | --- | --- |
//! | [`Buffer<T>`] parameter | a typed array (`Float32Array`) | `Float32Array` | **moved**: the JS array is detached (length 0) once the call starts |
//! | [`Buffer<T>`] result | a new typed array over the same memory | `Float32Array` | JS owns it; a buffer that came from JS goes back as the same memory |
//! | [`Shared<[T]>`] result | a read-only view of the `Arc` | `ReadonlyFloat32Array` | the `Arc` is released when JS collects the view |
//! | [`Shared<[T]>`] parameter | any typed array of `T` | `ReadonlyFloat32Array` | a view this host made gives back its `Arc` (no copy); anything else is copied |
//!
//! A JS-allocated buffer moved into a [`Buffer<T>`] stays in JS's allocator: Rust holds a
//! `TransferToken`, and dropping the buffer (on any thread: a worker that finished, panicked or was
//! abandoned after an abort) posts the release to the JS thread, which frees the memory there. A
//! buffer that Rust allocated (`alloc(Float32Array, n)` in JS, or a `Buffer<T>` that Rust returned)
//! is reclaimed instead: Rust gets its allocation back.
//!
//! Workers may own and mutate a `Buffer<T>` (it is `Send`), but nothing in JS can reach its memory
//! meanwhile: no data race, and no `SharedArrayBuffer`.

use super::{CallCx, FromJs, IntoJs, JsType, NativeError, TypeBuilder, TypeRef, Value, host_error};
use std::any::Any;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use svelters_abi::{BufData, BufSource, BufferValue, Elem, SVL_BYTES, SlotDesc, svl_id};

// ---- slot descriptors ----

/// Type ids of buffer slots: `SVL_BYTES` slots typed in this reserved app-range block (never a
/// backend's), bit 4 = moved (`Buffer<T>`), bits 0..4 = the element code.
const BUF_TY: svl_id = super::APP_PACK | 0x10_0000;

/// How a buffer slot takes its JS argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Take {
    /// `Buffer<T>`: moved (JS buffer transferred, or a Rust allocation reclaimed).
    Move,
    /// `Shared<[T]>`: read-only (a view of ours gives back its `Arc`, other arrays are copied).
    View,
}

pub(crate) fn slot_ty(elem: Elem, take: Take) -> svl_id {
    BUF_TY | if take == Take::Move { 0x10 } else { 0 } | elem.code()
}

/// The element and mode of a buffer slot type, `None` for other types.
pub(crate) fn spec(ty: svl_id) -> Option<(Elem, Take)> {
    if ty & !0x1F != BUF_TY {
        return None;
    }
    Some((Elem::from_code(ty & 0xF)?, if ty & 0x10 != 0 { Take::Move } else { Take::View }))
}

/// The generated read-only interface of a typed array (`@svelters/types/core`).
pub(crate) fn readonly_ts(elem: Elem) -> String {
    format!("Readonly{}", elem.js_name())
}

// ---- elements ----

mod sealed {
    pub trait Sealed {}
}

/// An element type of a typed array: `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`.
pub trait Element: sealed::Sealed + Copy + Default + Send + Sync + std::fmt::Debug + 'static {
    const ELEM: Elem;
}

macro_rules! element {
    ($($t:ty => $e:ident),*) => {$(
        impl sealed::Sealed for $t {}
        impl Element for $t {
            const ELEM: Elem = Elem::$e;
        }
    )*};
}
element!(u8 => U8, i8 => I8, u16 => U16, i16 => I16, u32 => U32, i32 => I32, u64 => U64, i64 => I64, f32 => F32, f64 => F64);

/// A `Vec<T>` as owned buffer memory.
struct VecSource<T: Element>(Vec<T>);

// SAFETY: the Vec's heap buffer does not move while the Vec is not touched, and it is only touched by
// being dropped or taken back whole (`into_vec`).
unsafe impl<T: Element> BufSource for VecSource<T> {
    fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr() as *mut u8
    }
    fn byte_len(&self) -> usize {
        std::mem::size_of_val(&self.0[..])
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// An `Arc<[T]>` as read-only buffer memory.
struct ArcSource<T: Element>(Arc<[T]>);

// SAFETY: the Arc's slice does not move while a clone lives, and no one writes it (JS gets an
// immutable view).
unsafe impl<T: Element> BufSource for ArcSource<T> {
    fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr() as *mut u8
    }
    fn byte_len(&self) -> usize {
        std::mem::size_of_val(&self.0[..])
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Copies `bytes` into a new `Arc<[T]>` source (one copy; for engines and arrays that are not ours).
pub(crate) fn shared_copy(elem: Elem, bytes: &[u8]) -> Arc<dyn BufSource> {
    fn make<T: Element>(bytes: &[u8]) -> Arc<dyn BufSource> {
        let n = bytes.len() / std::mem::size_of::<T>();
        let mut a: Arc<[MaybeUninit<T>]> = Arc::new_uninit_slice(n);
        let dst = Arc::get_mut(&mut a).expect("unique");
        // SAFETY: `dst` has room for `n` elements, every bit pattern is a valid `T`, and the byte
        // ranges do not overlap.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.as_mut_ptr() as *mut u8, n * std::mem::size_of::<T>()) };
        // SAFETY: every element was written above.
        Arc::new(ArcSource(unsafe { a.assume_init() }))
    }
    match elem {
        Elem::U8 => make::<u8>(bytes),
        Elem::I8 => make::<i8>(bytes),
        Elem::U16 => make::<u16>(bytes),
        Elem::I16 => make::<i16>(bytes),
        Elem::U32 => make::<u32>(bytes),
        Elem::I32 => make::<i32>(bytes),
        Elem::U64 => make::<u64>(bytes),
        Elem::I64 => make::<i64>(bytes),
        Elem::F32 => make::<f32>(bytes),
        Elem::F64 => make::<f64>(bytes),
    }
}

/// A zeroed owned source of `n` elements (`alloc(Float32Array, n)` in JS).
pub(crate) fn owned_zeroed(elem: Elem, n: usize) -> Box<dyn BufSource> {
    fn make<T: Element>(n: usize) -> Box<dyn BufSource> {
        Box::new(VecSource(vec![T::default(); n]))
    }
    match elem {
        Elem::U8 => make::<u8>(n),
        Elem::I8 => make::<i8>(n),
        Elem::U16 => make::<u16>(n),
        Elem::I16 => make::<i16>(n),
        Elem::U32 => make::<u32>(n),
        Elem::I32 => make::<i32>(n),
        Elem::U64 => make::<u64>(n),
        Elem::I64 => make::<i64>(n),
        Elem::F32 => make::<f32>(n),
        Elem::F64 => make::<f64>(n),
    }
}

// ---- Buffer<T> ----

/// An owned typed array: a JS `Float32Array` (etc.) **moved** into Rust, or Rust memory handed to JS.
///
/// - As a parameter, the caller's typed array is detached (length 0 in JS) when the call starts;
///   nothing is copied. A worker may mutate it in place and return it: JS gets the same memory back.
/// - As a result, JS gets a new typed array over the memory (no copy) and owns it.
///
/// ```ignore
/// #[svelters::export(module = "dsp")]
/// #[js(worker)]
/// pub fn gain(mut samples: Buffer<f32>, k: f32) -> Buffer<f32> {
///     for x in samples.iter_mut() { *x *= k; }
///     samples                                   // JS: the same memory, as a new Float32Array
/// }
/// ```
///
/// `Buffer<T>` is `Send`: it may move to a worker. Memory that came from JS is freed on the JS
/// thread when the buffer is dropped (wherever that happens).
pub struct Buffer<T: Element> {
    data: Option<BufData>,
    /// Byte offset of element 0 in `data`.
    offset: usize,
    len: usize,
    _t: PhantomData<T>,
}

impl<T: Element> Buffer<T> {
    /// A buffer over `v` (no copy).
    pub fn from_vec(v: Vec<T>) -> Buffer<T> {
        let len = v.len();
        Buffer { data: Some(BufData::Owned(Box::new(VecSource(v)))), offset: 0, len, _t: PhantomData }
    }

    /// `n` zeroed elements.
    pub fn zeroed(n: usize) -> Buffer<T> {
        Buffer::from_vec(vec![T::default(); n])
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when the memory belongs to the JS engine (a transferred JS buffer), false for Rust memory.
    pub fn is_js_memory(&self) -> bool {
        matches!(self.data, Some(BufData::Transferred(_)))
    }

    /// The address of element 0 (to show that no copy happened).
    pub fn as_ptr(&self) -> *const T {
        self.as_slice().as_ptr()
    }

    pub fn as_slice(&self) -> &[T] {
        match &self.data {
            // SAFETY: the range was checked when the buffer was made (`from_value`), the memory is
            // aligned for `T`, and `&self` gives shared access to exclusively owned memory.
            Some(d) if self.len > 0 => unsafe { std::slice::from_raw_parts(d.as_ptr().add(self.offset) as *const T, self.len) },
            _ => &[],
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match &self.data {
            // SAFETY: as `as_slice`, and `&mut self` gives exclusive access (JS cannot reach the memory).
            Some(d) if self.len > 0 => unsafe { std::slice::from_raw_parts_mut(d.as_ptr().add(self.offset) as *mut T, self.len) },
            _ => &mut [],
        }
    }

    /// The elements as a `Vec<T>`: no copy for Rust memory made from a whole `Vec`, a copy otherwise
    /// (JS memory is then released on the JS thread).
    pub fn into_vec(mut self) -> Vec<T> {
        if self.offset == 0 {
            if let Some(BufData::Owned(src)) = &self.data {
                if src.as_any().is::<VecSource<T>>() && src.byte_len() == self.len * std::mem::size_of::<T>() {
                    let Some(BufData::Owned(src)) = self.data.take() else { unreachable!() };
                    let raw = Box::into_raw(src) as *mut VecSource<T>;
                    // SAFETY: the box holds a `VecSource<T>` (checked by `is` above).
                    return unsafe { Box::from_raw(raw) }.0;
                }
            }
        }
        self.as_slice().to_vec()
    }

    /// From a buffer payload; copies when the memory is not aligned for `T` (never the case for typed
    /// arrays) or is shared.
    fn from_value(b: BufferValue) -> Result<Buffer<T>, NativeError> {
        if b.elem() != T::ELEM {
            return Err(host_error("type", format!("expected {}, got {}", T::ELEM.js_name(), b.elem().js_name())));
        }
        let (offset, len) = (b.offset(), b.len());
        if b.shared().is_some() {
            // Read-only memory is never moved: the buffer gets a copy.
            return Ok(Buffer::from_vec(b.with_bytes(copy_vec::<T>)));
        }
        match b.take() {
            Some(d) => {
                let aligned = len == 0 || (d.as_ptr() as usize + offset) % std::mem::align_of::<T>() == 0;
                let buf = Buffer { data: Some(d), offset, len, _t: PhantomData };
                if aligned { Ok(buf) } else { Ok(Buffer::from_vec(buf.as_bytes_unaligned(copy_vec::<T>))) }
            }
            None => Err(host_error("type", "the buffer was moved already")),
        }
    }

    fn as_bytes_unaligned<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        match &self.data {
            // SAFETY: the range was checked at construction; bytes have no alignment requirement.
            Some(d) if self.len > 0 => f(unsafe { std::slice::from_raw_parts(d.as_ptr().add(self.offset), self.len * std::mem::size_of::<T>()) }),
            _ => f(&[]),
        }
    }
}

fn copy_vec<T: Element>(bytes: &[u8]) -> Vec<T> {
    let n = bytes.len() / std::mem::size_of::<T>();
    let mut v: Vec<T> = Vec::with_capacity(n);
    // SAFETY: the capacity is `n`, every bit pattern is a valid `T`, the ranges do not overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), v.as_mut_ptr() as *mut u8, n * std::mem::size_of::<T>());
        v.set_len(n);
    }
    v
}

impl<T: Element> std::ops::Deref for Buffer<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Element> std::ops::DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T: Element> From<Vec<T>> for Buffer<T> {
    fn from(v: Vec<T>) -> Self {
        Buffer::from_vec(v)
    }
}

impl<T: Element> std::fmt::Debug for Buffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Buffer<{:?}>({} elements{})", T::ELEM, self.len, if self.is_js_memory() { ", JS memory" } else { "" })
    }
}

impl<T: Element> JsType for Buffer<T> {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::new(SlotDesc::new(SVL_BYTES).typed(slot_ty(T::ELEM, Take::Move)), T::ELEM.js_name())
    }
}

impl<T: Element> FromJs for Buffer<T> {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        match v {
            Value::Buffer(b) => Buffer::from_value(b),
            // An engine without buffer support passes a copy.
            Value::Bytes(b) => Ok(Buffer::from_vec(copy_vec::<T>(&b))),
            other => Err(host_error("type", format!("expected {}, got {other:?}", T::ELEM.js_name()))),
        }
    }
}

impl<T: Element> IntoJs for Buffer<T> {
    fn into_js(mut self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        let data = self.data.take().ok_or_else(|| host_error("type", "the buffer was moved already"))?;
        Ok(Value::Buffer(BufferValue::new(T::ELEM, data, self.offset, self.len)))
    }
}

// ---- Shared<[T]> ----

/// Immutable data JS reads without a copy: `Shared<[T]>` is an `Arc<[T]>`.
///
/// - As a result, JS gets a read-only typed array (`ReadonlyFloat32Array` in the typings) over the
///   `Arc`'s memory; the `Arc` is released when JS collects the view. Writes to the view are ignored
///   by the engine (no exception), which is why the typings mark it read-only.
/// - As a parameter, a view this host handed out gives back the same `Arc` (no copy); any other
///   typed array of `T` is copied (the caller keeps its array).
///
/// ```ignore
/// pub fn positions(&self) -> Shared<[f32]> { self.snapshot.clone() }
/// ```
pub struct Shared<T: ?Sized>(Arc<T>);

impl<T: ?Sized> Shared<T> {
    pub fn new(a: Arc<T>) -> Shared<T> {
        Shared(a)
    }

    pub fn arc(&self) -> &Arc<T> {
        &self.0
    }

    pub fn into_arc(self) -> Arc<T> {
        self.0
    }
}

impl<T: ?Sized> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: ?Sized> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> From<Arc<T>> for Shared<T> {
    fn from(a: Arc<T>) -> Self {
        Shared(a)
    }
}

impl<T: Element> From<Vec<T>> for Shared<[T]> {
    fn from(v: Vec<T>) -> Self {
        Shared(v.into())
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Element> JsType for Shared<[T]> {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::new(SlotDesc::new(SVL_BYTES).typed(slot_ty(T::ELEM, Take::View)), readonly_ts(T::ELEM))
    }
}

impl<T: Element> FromJs for Shared<[T]> {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        let b = match v {
            Value::Buffer(b) => b,
            Value::Bytes(b) => return Ok(Shared(copy_vec::<T>(&b).into())),
            other => return Err(host_error("type", format!("expected {}, got {other:?}", T::ELEM.js_name()))),
        };
        if b.elem() != T::ELEM {
            return Err(host_error("type", format!("expected {}, got {}", T::ELEM.js_name(), b.elem().js_name())));
        }
        if let Some(src) = b.shared() {
            if let Some(a) = src.as_any().downcast_ref::<ArcSource<T>>() {
                if b.offset() == 0 && b.len() == a.0.len() {
                    return Ok(Shared(a.0.clone()));
                }
            }
        }
        Ok(Shared(b.with_bytes(copy_vec::<T>).into()))
    }
}

impl<T: Element> IntoJs for Shared<[T]> {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        let len = self.0.len();
        Ok(Value::Buffer(BufferValue::new(T::ELEM, BufData::Shared(Arc::new(ArcSource(self.0))), 0, len)))
    }
}
