//! The hand-written COM core of the type layer (Windows only): interface pointers, vtable calls, activation
//! factories, HSTRINGs, boxing through `PropertyValue`, collections read into JS values, HRESULT -> `NativeError`.
//!
//! The rules it keeps (NON-GUI-TYPES.md sections 2.2, 4.1):
//! - Every pointer that crosses is owned by an `IUnknown` (one reference): out pointers are adopted, arguments are
//!   queried for the exact interface the parameter declares and kept alive until the call returns.
//! - Objects become handles only through [`Env::mint`] (identity + thread check), never here.
use super::invoke::{self, Word};
use super::model::{Acc, Ty, TyId};
use super::tables::{CORE, IIDS, TY_STR, TY_U32};
use super::value::{self, R, is_plain, read_plain, size_align, ty};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use svelters_abi::{NativeError, Value};
use windows_core::{GUID, HRESULT, HSTRING, IInspectable, IUnknown, Interface};

windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoGetActivationFactory(activatableclassid: *mut c_void, iid: *const GUID, factory: *mut *mut c_void) -> HRESULT);

pub fn iid(i: u16) -> GUID {
    GUID::from_u128(IIDS[i as usize])
}

/// What the conversions need from the backend: handles <-> objects.
pub trait Env {
    /// The object behind any live handle (an element or a type handle).
    fn object(&self, h: u64) -> R<IUnknown>;
    /// The handle of `obj` (existing, or a new SHARED one of its runtime class, else `declared`), after the thread
    /// check of its kind.
    fn mint(&self, obj: IUnknown, declared: u16) -> R<u64>;
    /// Our handle of an object, if it has one (elements are never minted here).
    fn known(&self, obj: &IUnknown) -> Option<u64>;
}

// ---- errors -----------------------------------------------------------------------------------------------------

/// Names of the HRESULTs a JS caller may want to branch on (`err.code`); others cross as `0x8xxxxxxx`.
fn hresult_name(hr: u32) -> Option<&'static str> {
    Some(match hr {
        0x80070057 => "invalid-arg",
        0x8000000B => "bounds",
        0x80004001 => "not-implemented",
        0x80004002 => "no-interface",
        0x80004004 => "aborted",
        0x80004005 => "fail",
        0x80070005 => "access-denied",
        0x80070002 => "file-not-found",
        0x80070003 => "path-not-found",
        0x800700B7 => "already-exists",
        0x80070020 => "sharing-violation",
        0x800704C7 => "cancelled",
        0x8001010E => "wrong-thread",
        0x8000000E => "illegal-method-call",
        0x8000000C => "changed-state",
        0x80000013 => "closed",
        0x80040154 => "class-not-registered",
        0x8000FFFF => "unexpected",
        _ => return None,
    })
}

/// A failed WinRT call as the `ERROR` value JS receives (`NativeError { domain: 'winrt', code, message }`).
pub fn native_error(e: &windows_core::Error) -> Value {
    let hr = e.code().0 as u32;
    let code = hresult_name(hr).map_or_else(|| Value::Str(format!("0x{hr:08X}")), |n| Value::Str(n.into()));
    let msg = e.message();
    let message = if msg.trim().is_empty() { format!("HRESULT 0x{hr:08X}") } else { format!("{} (HRESULT 0x{hr:08X})", msg.trim()) };
    Value::Error(Box::new(NativeError { domain: "winrt".into(), code, message }))
}

pub fn err_text(e: &windows_core::Error) -> String {
    match native_error(e) {
        Value::Error(n) => n.message,
        _ => unreachable!(),
    }
}

/// A WinRT failure, or our own conversion error (a plain message).
pub enum CallErr {
    Win(windows_core::Error),
    Msg(String),
}

impl From<String> for CallErr {
    fn from(s: String) -> CallErr {
        CallErr::Msg(s)
    }
}

impl From<&str> for CallErr {
    fn from(s: &str) -> CallErr {
        CallErr::Msg(s.to_string())
    }
}

impl From<windows_core::Error> for CallErr {
    fn from(e: windows_core::Error) -> CallErr {
        CallErr::Win(e)
    }
}

impl CallErr {
    pub fn text(&self) -> String {
        match self {
            CallErr::Win(e) => err_text(e),
            CallErr::Msg(m) => m.clone(),
        }
    }
}

pub type CR<T> = Result<T, CallErr>;

// ---- interface pointers -------------------------------------------------------------------------------------------

/// `QueryInterface` for `iid`; the result owns one reference to that interface pointer.
pub fn qi(obj: &IUnknown, iid: &GUID) -> windows_core::Result<IUnknown> {
    let mut out = std::ptr::null_mut();
    // SAFETY: `obj` is a live COM pointer; QueryInterface writes an owned pointer or null.
    let hr = unsafe { (obj.vtable().QueryInterface)(obj.as_raw(), iid, &mut out) };
    hr.ok()?;
    if out.is_null() {
        return Err(windows_core::Error::from_hresult(HRESULT(0x80004002u32 as i32)));
    }
    // SAFETY: an owned interface pointer; every COM interface starts with IUnknown's vtable.
    Ok(unsafe { IUnknown::from_raw(out) })
}

/// The identity of a COM object: its IUnknown pointer (stable while we hold a reference).
pub fn identity(obj: &IUnknown) -> usize {
    qi(obj, &IUnknown::IID).map(|u| u.as_raw() as usize).unwrap_or(obj.as_raw() as usize)
}

/// Calls vtable slot `slot` of the interface pointer `iface`.
///
/// # Safety
/// `words` must match the slot's ABI signature, and every pointer in them must stay valid for the call.
pub unsafe fn vcall(iface: &IUnknown, slot: u16, words: &[Word]) -> windows_core::Result<()> {
    let hr = unsafe { invoke::call_slot(iface.as_raw(), slot, words) }
        .map_err(|e| windows_core::Error::new(HRESULT(0x80004001u32 as i32), e))?;
    HRESULT(hr).ok()
}

/// Queries `obj` for interface `acc.iid` and calls `acc.slot` with `words`.
pub unsafe fn call_acc(obj: &IUnknown, acc: Acc, words: &[Word]) -> windows_core::Result<()> {
    let i = qi(obj, &iid(acc.iid))?;
    unsafe { vcall(&i, acc.slot, words) }
}

/// A zeroed, 8-byte aligned buffer for an out value of type `t`.
pub fn out_buf(t: Ty) -> Vec<u64> {
    vec![0u64; size_align(t).0.div_ceil(8).max(1)]
}

fn bytes(buf: &[u64]) -> &[u8] {
    // SAFETY: a u64 buffer viewed as bytes.
    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 8) }
}

fn raw_ptr(buf: &[u64]) -> *mut c_void {
    buf[0] as usize as *mut c_void
}

/// Calls a getter-shaped slot (`HRESULT get(this, T* out)`) and converts the result.
pub fn get_value(env: &dyn Env, obj: &IUnknown, slot: u16, t: TyId, pre: &[Word]) -> CR<Value> {
    let mut buf = out_buf(ty(t));
    let mut words = pre.to_vec();
    words.push(Word::ptr(buf.as_mut_ptr()));
    unsafe { vcall(obj, slot, &words)? };
    // SAFETY: the callee wrote an owned value of type `t` into the buffer.
    unsafe { take_out(env, t, &buf) }
}

// ---- activation ---------------------------------------------------------------------------------------------------

thread_local! {
    static FACTORIES: RefCell<HashMap<(String, u128), IUnknown>> = RefCell::default();
}

/// The activation factory of `class` for interface `iid` (cached per thread). `RoGetActivationFactory` first (the
/// Windows App SDK bootstrapper makes the framework package's classes visible to it), then, for a class that is not
/// registered, `DllGetActivationFactory` of `A.B.dll`, `A.dll` (windows-rs's reg-free fallback).
pub fn factory(class: &str, iid_: &GUID) -> windows_core::Result<IUnknown> {
    let key = (class.to_string(), iid_.to_u128());
    if let Some(f) = FACTORIES.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(f);
    }
    let name = HSTRING::from(class);
    let mut out = std::ptr::null_mut();
    // SAFETY: a valid HSTRING (borrowed for the call) and out pointer.
    let hr = unsafe { RoGetActivationFactory(std::mem::transmute_copy(&name), iid_, &mut out) };
    let f = if hr.is_ok() && !out.is_null() {
        // SAFETY: an owned pointer of interface `iid_`.
        unsafe { IUnknown::from_raw(out) }
    } else if hr.0 as u32 == 0x80040154 {
        let generic = reg_free_factory(class, &name).ok_or_else(|| windows_core::Error::from_hresult(hr))?;
        qi(&generic, iid_)?
    } else {
        return Err(windows_core::Error::from(hr));
    };
    FACTORIES.with(|c| c.borrow_mut().insert(key, f.clone()));
    Ok(f)
}

fn reg_free_factory(class: &str, name: &HSTRING) -> Option<IUnknown> {
    use crate::bindings::Windows::Win32::{GetProcAddress, LoadLibraryW};
    type Get = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let mut path = class;
    while let Some(i) = path.rfind('.') {
        path = &path[..i];
        let wide: Vec<u16> = format!("{path}.dll").encode_utf16().chain([0]).collect();
        // SAFETY: a NUL-terminated wide string.
        let m = unsafe { LoadLibraryW(windows_core::PCWSTR(wide.as_ptr())) };
        if m.0.is_null() {
            continue;
        }
        // SAFETY: a module handle and a NUL-terminated name.
        let Some(f) = (unsafe { GetProcAddress(m, windows_core::s!("DllGetActivationFactory")) }) else { continue };
        // SAFETY: the export has this signature.
        let f: Get = unsafe { std::mem::transmute(f) };
        let mut out = std::ptr::null_mut();
        // SAFETY: as above.
        if unsafe { f(std::mem::transmute_copy(name), &mut out) }.is_ok() && !out.is_null() {
            return Some(unsafe { IUnknown::from_raw(out) });
        }
    }
    None
}

// ---- strings --------------------------------------------------------------------------------------------------

/// The ABI word of an HSTRING we keep alive (null for the empty string).
pub fn hstring_word(h: &HSTRING) -> Word {
    // SAFETY: HSTRING is a pointer-sized handle; copying it does not transfer ownership.
    Word::I(unsafe { std::mem::transmute_copy::<HSTRING, usize>(h) } as u64)
}

/// Adopts an HSTRING the callee returned.
///
/// # Safety
/// `raw` must be an owned HSTRING (or null).
pub unsafe fn take_hstring(raw: *mut c_void) -> String {
    // SAFETY: per the contract.
    let h: HSTRING = unsafe { std::mem::transmute(raw) };
    h.to_string_lossy()
}

/// A copy of a borrowed HSTRING (delegate arguments).
///
/// # Safety
/// `raw` must be a valid HSTRING for the duration of the call.
pub unsafe fn copy_hstring(raw: *mut c_void) -> String {
    // SAFETY: borrowed: wrapped in ManuallyDrop so it is not freed.
    let h = std::mem::ManuallyDrop::new(unsafe { std::mem::transmute::<*mut c_void, HSTRING>(raw) });
    h.to_string_lossy()
}

pub fn runtime_class_name(obj: &IUnknown) -> Option<String> {
    let i = obj.cast::<IInspectable>().ok()?;
    i.GetRuntimeClassName().ok().map(|h| h.to_string_lossy())
}

// ---- arguments ----------------------------------------------------------------------------------------------------

/// What an argument list keeps alive until the call returns.
#[derive(Default)]
pub struct Keep {
    pub strings: Vec<HSTRING>,
    pub objs: Vec<IUnknown>,
    pub bufs: Vec<Vec<u64>>,
}

/// `PropertyType` values (a WinRT-defined enum, Windows.Foundation.PropertyType) for the scalar types we box.
fn property_type(t: Ty) -> Option<i64> {
    Some(match t {
        Ty::U8 => 1,
        Ty::I16 => 2,
        Ty::U16 => 3,
        Ty::I32 | Ty::Enum(_) => 4,
        Ty::U32 => 5,
        Ty::I64 => 6,
        Ty::U64 => 7,
        Ty::F32 => 8,
        Ty::F64 => 9,
        Ty::Char16 => 10,
        Ty::Bool => 11,
        Ty::Str => 12,
        Ty::DateTime => 14,
        Ty::TimeSpan => 15,
        Ty::Guid => 16,
        _ => return None,
    })
}

fn pv_type_ty(code: i64) -> Option<Ty> {
    Some(match code {
        1 => Ty::U8,
        2 => Ty::I16,
        3 => Ty::U16,
        4 => Ty::I32,
        5 => Ty::U32,
        6 => Ty::I64,
        7 => Ty::U64,
        8 => Ty::F32,
        9 => Ty::F64,
        10 => Ty::Char16,
        11 => Ty::Bool,
        12 => Ty::Str,
        14 => Ty::DateTime,
        15 => Ty::TimeSpan,
        16 => Ty::Guid,
        _ => return None,
    })
}

/// `PropertyValue.Create<T>(v)` for a scalar type: an IInspectable (implementing `IReference<T>`).
fn box_scalar(t: Ty, v: &Value) -> CR<IUnknown> {
    let code = property_type(t).ok_or_else(|| format!("cannot box a {t:?}"))?;
    let slot = CORE.pv_create.iter().find(|(c, _)| *c as i64 == code).map(|(_, s)| *s).ok_or("no PropertyValue.Create slot")?;
    let f = factory("Windows.Foundation.PropertyValue", &iid(CORE.pv_statics))?;
    let mut keep = Keep::default();
    // An enum boxes as its Int32 value.
    let t2 = if let Ty::Enum(_) = t { Ty::I32 } else { t };
    let word = if let Ty::Enum(e) = t {
        Word::I(value::enum_in(super::model::enum_t(e), v)? as i32 as u32 as u64)
    } else if t2 == Ty::Str {
        let h = HSTRING::from(value::to_str(v)?);
        let w = hstring_word(&h);
        keep.strings.push(h);
        w
    } else {
        value::plain_word(t2, v, &mut keep.bufs)?
    };
    let mut out: *mut c_void = std::ptr::null_mut();
    unsafe { vcall(&f, slot, &[word, Word::ptr(&mut out)])? };
    if out.is_null() {
        return Err("PropertyValue returned null".into());
    }
    // SAFETY: an owned IInspectable.
    Ok(unsafe { IUnknown::from_raw(out) })
}

/// An `object` argument: JS scalars box as Boolean / Double / String / DateTime (the JS projection's rule), handles
/// pass their object, null is a null pointer.
pub fn box_any(env: &dyn Env, v: &Value) -> CR<Option<IUnknown>> {
    Ok(Some(match v {
        Value::Null => return Ok(None),
        Value::Bool(_) => box_scalar(Ty::Bool, v)?,
        Value::I64(_) | Value::U64(_) | Value::F64(_) => box_scalar(Ty::F64, v)?,
        Value::Str(_) | Value::Str16(_) => box_scalar(Ty::Str, v)?,
        Value::DateTime(_) => box_scalar(Ty::DateTime, v)?,
        Value::Handle(h) | Value::Boxed { handle: h, .. } => env.object(*h)?,
        other => return Err(format!("cannot pass {other:?} as a WinRT object").into()),
    }))
}

/// The argument word of `v` for a parameter of type `t`.
pub fn arg_in(env: &dyn Env, t: TyId, v: &Value, keep: &mut Keep) -> CR<Word> {
    let t = ty(t);
    if is_plain(t) {
        return Ok(value::plain_word(t, v, &mut keep.bufs)?);
    }
    let obj: Option<IUnknown> = match t {
        Ty::Str => {
            let h = HSTRING::from(value::to_str(v)?);
            let w = hstring_word(&h);
            keep.strings.push(h);
            return Ok(w);
        }
        Ty::Variant => box_any(env, v)?.map(|o| qi(&o, &IInspectable::IID)).transpose()?,
        Ty::Object { iid: i, .. } | Ty::Widget { iid: i } => match v {
            Value::Null => None,
            Value::Handle(h) | Value::Boxed { handle: h, .. } => {
                let o = env.object(*h)?;
                Some(qi(&o, &iid(i)).map_err(|e| CallErr::Msg(format!("handle {h} is not a {}: {}", iid(i).to_u128(), err_text(&e))))?)
            }
            other => return Err(format!("expected an object (handle), got {other:?}").into()),
        },
        Ty::Nullable { of, iid: i } => match v {
            Value::Null => None,
            other => Some(qi(&box_scalar(ty(of), other)?, &iid(i))?),
        },
        other => return Err(format!("{other:?} is not supported as an argument").into()),
    };
    let w = Word::I(obj.as_ref().map_or(0, |o| o.as_raw() as usize as u64));
    if let Some(o) = obj {
        keep.objs.push(o);
    }
    Ok(w)
}

// ---- results ------------------------------------------------------------------------------------------------------

/// Converts an owned out value of type `t` from its buffer (adopting pointers and strings).
///
/// # Safety
/// `buf` must hold an out value of type `t` written by the callee (owned references for pointer types).
pub unsafe fn take_out(env: &dyn Env, t: TyId, buf: &[u64]) -> CR<Value> {
    let tt = ty(t);
    if is_plain(tt) {
        return Ok(read_plain(tt, bytes(buf)));
    }
    if tt == Ty::Void {
        return Ok(Value::Null);
    }
    if tt == Ty::Str {
        return Ok(Value::Str(unsafe { take_hstring(raw_ptr(buf)) }));
    }
    let p = raw_ptr(buf);
    if p.is_null() {
        return Ok(Value::Null);
    }
    // SAFETY: an owned interface pointer.
    let obj = unsafe { IUnknown::from_raw(p) };
    object_value(env, t, obj)
}

/// A borrowed delegate argument of type `t` (copies strings, adds a reference to objects).
///
/// # Safety
/// `word` must be a valid argument of type `t` for the duration of the call.
pub unsafe fn borrowed_value(env: &dyn Env, t: TyId, word: u64) -> CR<Value> {
    let tt = ty(t);
    if is_plain(tt) {
        let (size, _) = size_align(tt);
        if matches!(tt, Ty::Record(_) | Ty::Guid) && !value::struct_in_register(size) {
            // SAFETY: a struct larger than 8 bytes is passed by pointer.
            let b = unsafe { std::slice::from_raw_parts(word as usize as *const u8, size) };
            return Ok(read_plain(tt, b));
        }
        return Ok(read_plain(tt, &word.to_le_bytes()));
    }
    if tt == Ty::Str {
        return Ok(Value::Str(unsafe { copy_hstring(word as usize as *mut c_void) }));
    }
    let p = word as usize as *mut c_void;
    if p.is_null() {
        return Ok(Value::Null);
    }
    // SAFETY: a borrowed interface pointer: add our own reference.
    let obj = unsafe { IUnknown::from_raw_borrowed(&p) }.cloned().ok_or("null")?;
    object_value(env, t, obj)
}

/// An object result of type `t` (we own `obj`): a handle, an unboxed scalar, a nullable's value, or a collection
/// read into a JS value.
pub fn object_value(env: &dyn Env, t: TyId, obj: IUnknown) -> CR<Value> {
    match ty(t) {
        Ty::Object { kind, .. } => Ok(Value::Handle(env.mint(obj, kind)?)),
        Ty::Widget { .. } => Ok(env.known(&obj).map_or(Value::Null, Value::Handle)),
        Ty::Variant => unbox(env, obj),
        Ty::Nullable { of, .. } => get_value(env, &obj, CORE.reference_value, of, &[]),
        Ty::View { of } => {
            let n = match get_value(env, &obj, CORE.view_size, TY_U32, &[])? {
                Value::I64(n) => n as u32,
                _ => 0,
            };
            let mut out = Vec::with_capacity(n as usize);
            for i in 0..n {
                out.push(get_value(env, &obj, CORE.view_get_at, of, &[Word::I(i as u64)])?);
            }
            Ok(Value::List(out))
        }
        Ty::Iterable { of } => iterate(env, &obj, |env, it| get_value(env, it, CORE.iterator_current, of, &[])).map(Value::List),
        Ty::MapView { of, iter_iid } => {
            let it = qi(&obj, &iid(iter_iid))?;
            let pairs = iterate(env, &it, |env, iter| {
                let mut buf = [0u64; 1];
                unsafe { vcall(iter, CORE.iterator_current, &[Word::ptr(buf.as_mut_ptr())])? };
                if buf[0] == 0 {
                    return Ok(Value::Null);
                }
                // SAFETY: an owned IKeyValuePair pointer.
                let pair = unsafe { IUnknown::from_raw(buf[0] as usize as *mut c_void) };
                let k = get_value(env, &pair, CORE.pair_key, TY_STR, &[])?;
                let v = get_value(env, &pair, CORE.pair_value, of, &[])?;
                Ok(Value::List(vec![k, v]))
            })?;
            let mut m = Vec::new();
            for p in pairs {
                if let Value::List(mut kv) = p {
                    let v = kv.pop().unwrap_or(Value::Null);
                    if let Some(Value::Str(k)) = kv.pop() {
                        m.push((k, v));
                    }
                }
            }
            Ok(Value::Map(m))
        }
        Ty::Pair { key, of } => {
            let k = get_value(env, &obj, CORE.pair_key, key, &[])?;
            let v = get_value(env, &obj, CORE.pair_value, of, &[])?;
            Ok(Value::Map(vec![("key".into(), k), ("value".into(), v)]))
        }
        other => Err(format!("{other:?} cannot be returned here").into()),
    }
}

fn iterate(env: &dyn Env, iterable: &IUnknown, mut item: impl FnMut(&dyn Env, &IUnknown) -> CR<Value>) -> CR<Vec<Value>> {
    let mut buf = [0u64; 1];
    unsafe { vcall(iterable, CORE.iterable_first, &[Word::ptr(buf.as_mut_ptr())])? };
    if buf[0] == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: an owned IIterator pointer.
    let it = unsafe { IUnknown::from_raw(buf[0] as usize as *mut c_void) };
    let mut out = Vec::new();
    loop {
        let mut has = 0u64;
        unsafe { vcall(&it, CORE.iterator_has_current, &[Word::ptr(&mut has)])? };
        if has as u8 == 0 {
            break;
        }
        out.push(item(env, &it)?);
        let mut more = 0u64;
        unsafe { vcall(&it, CORE.iterator_move_next, &[Word::ptr(&mut more)])? };
        if more as u8 == 0 {
            break;
        }
    }
    Ok(out)
}

/// An `object` result: a boxed scalar (`IPropertyValue`) unboxes; anything else is a handle of its runtime class.
pub fn unbox(env: &dyn Env, obj: IUnknown) -> CR<Value> {
    if let Ok(pv) = qi(&obj, &iid(CORE.property_value)) {
        let mut code = [0u64; 1];
        unsafe { vcall(&pv, CORE.pv_type, &[Word::ptr(code.as_mut_ptr())])? };
        let code = code[0] as u32 as i64;
        if code == 0 {
            return Ok(Value::Null);
        }
        if let (Some(t), Some((_, slot))) = (pv_type_ty(code), CORE.pv_get.iter().find(|(c, _)| *c as i64 == code)) {
            let mut buf = out_buf(t);
            unsafe { vcall(&pv, *slot, &[Word::ptr(buf.as_mut_ptr())])? };
            return Ok(if t == Ty::Str { Value::Str(unsafe { take_hstring(raw_ptr(&buf)) }) } else { read_plain(t, bytes(&buf)) });
        }
    }
    Ok(Value::Handle(env.mint(obj, 0)?))
}
