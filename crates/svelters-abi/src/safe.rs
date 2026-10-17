//! Safe wrappers over the raw ABI types.
//!
//! - [`Value`]: owned value enum with conversion to and from `svl_value`.
//! - [`ValueRef`] / [`ArgsRef`]: borrowed views of raw values, so a Rust backend can read strings
//!   and lists without copying them into a [`Value`].
//! - [`MemberDesc`] / [`TypeDesc`] / [`SlotDesc`]: owned counterparts of the type descriptors.
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
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
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

/// A native error carried by an `SVL_ERROR` value (GError, mapped C++ exception).
#[derive(Clone, Debug, PartialEq)]
pub struct NativeError {
    pub domain: String,
    /// `Value::Enum`, `Value::I64` or `Value::Str`.
    pub code: Value,
    pub message: String,
}

/// Failure reported through a negative return code plus `last_error` (and, optionally, an
/// `SVL_ERROR` value in the out slot).
#[derive(Clone, Debug, PartialEq)]
pub struct AbiError {
    pub code: i32,
    pub message: String,
    pub native: Option<NativeError>,
}

impl AbiError {
    pub fn plain(code: i32, message: impl Into<String>) -> AbiError {
        AbiError {
            code,
            message: message.into(),
            native: None,
        }
    }
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
    /// `I64` with `SVL_F_UNSIGNED`: values above `i64::MAX`.
    U64(u64),
    F64(f64),
    /// UTF-8 text. UTF-16 input is transcoded when copied into an owned value.
    Str(String),
    Handle(svl_handle),
    List(Vec<Value>),
    Enum { ty: svl_id, value: i64 },
    Flags { ty: svl_id, mask: u64 },
    Record { ty: svl_id, fields: Vec<Value> },
    Bytes(Vec<u8>),
    Boxed { ty: svl_id, handle: svl_handle },
    Callback { id: u64, ty: svl_id },
    /// Alternating keys and values on the wire; keys are strings.
    Map(Vec<(String, Value)>),
    /// `[domain, code, message]` on the wire.
    Error(Box<NativeError>),
}

/// Raw values built from a [`Value`] slice. Pointers inside stay valid while this lives and the
/// source slice is not mutated or dropped.
pub struct RawValues<'a> {
    raw: Vec<svl_value>,
    _lists: Vec<Vec<svl_value>>,
    _src: PhantomData<&'a [Value]>,
}

fn list_of(items: Vec<svl_value>, lists: &mut Vec<Vec<svl_value>>) -> svl_list {
    // The Vec's heap buffer does not move when the Vec itself is moved into `lists`.
    let l = svl_list {
        ptr: items.as_ptr(),
        len: items.len(),
    };
    lists.push(items);
    l
}

fn build_raw(v: &Value, lists: &mut Vec<Vec<svl_value>>) -> svl_value {
    match v {
        Value::Null => svl_value::null(),
        Value::Bool(b) => svl_value::new(SVL_BOOL, svl_value_u { b: *b }),
        Value::I64(i) => svl_value::new(SVL_I64, svl_value_u { i: *i }),
        Value::U64(u) => {
            let mut r = svl_value::new(SVL_I64, svl_value_u { u: *u });
            r.flags = SVL_F_UNSIGNED;
            r
        }
        Value::F64(f) => svl_value::new(SVL_F64, svl_value_u { f: *f }),
        Value::Handle(h) => svl_value::new(SVL_HANDLE, svl_value_u { h: *h }),
        Value::Str(s) => svl_value::new(
            SVL_STR,
            svl_value_u {
                s: svl_str {
                    ptr: s.as_ptr().cast(),
                    len: s.len(),
                },
            },
        ),
        Value::List(items) => {
            let inner: Vec<svl_value> = items.iter().map(|x| build_raw(x, lists)).collect();
            svl_value::new(SVL_LIST, svl_value_u { l: list_of(inner, lists) })
        }
        Value::Enum { ty, value } => svl_value::typed(SVL_ENUM, *ty, svl_value_u { i: *value }),
        Value::Flags { ty, mask } => svl_value::typed(SVL_FLAGS, *ty, svl_value_u { u: *mask }),
        Value::Record { ty, fields } => {
            let inner: Vec<svl_value> = fields.iter().map(|x| build_raw(x, lists)).collect();
            svl_value::typed(SVL_RECORD, *ty, svl_value_u { l: list_of(inner, lists) })
        }
        Value::Bytes(b) => svl_value::new(
            SVL_BYTES,
            svl_value_u {
                bytes: svl_bytes {
                    ptr: b.as_ptr(),
                    len: b.len(),
                },
            },
        ),
        Value::Boxed { ty, handle } => svl_value::typed(SVL_BOXED, *ty, svl_value_u { h: *handle }),
        Value::Callback { id, ty } => svl_value::typed(SVL_CALLBACK, *ty, svl_value_u { u: *id }),
        Value::Map(entries) => {
            let mut inner = Vec::with_capacity(entries.len() * 2);
            for (k, x) in entries {
                inner.push(str_raw(k));
                inner.push(build_raw(x, lists));
            }
            svl_value::new(SVL_MAP, svl_value_u { l: list_of(inner, lists) })
        }
        Value::Error(e) => {
            let inner = vec![str_raw(&e.domain), build_raw(&e.code, lists), str_raw(&e.message)];
            svl_value::new(SVL_ERROR, svl_value_u { l: list_of(inner, lists) })
        }
    }
}

fn str_raw(s: &str) -> svl_value {
    svl_value::new(
        SVL_STR,
        svl_value_u {
            s: svl_str {
                ptr: s.as_ptr().cast(),
                len: s.len(),
            },
        },
    )
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
    pub fn as_slice(&self) -> &[svl_value] {
        &self.raw
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

/// Reads the text of a `STR` value (UTF-8 or UTF-16), replacing invalid sequences.
///
/// # Safety
/// `v` must be a `SVL_STR` value whose pointer is valid for `len` code units.
pub unsafe fn str_of_raw<'a>(v: &'a svl_value) -> Cow<'a, str> {
    let s = unsafe { v.u.s };
    if s.ptr.is_null() || s.len == 0 {
        return Cow::Borrowed("");
    }
    if v.flags & SVL_F_UTF16 != 0 {
        let units = unsafe { std::slice::from_raw_parts(s.ptr.cast::<u16>(), s.len) };
        Cow::Owned(String::from_utf16_lossy(units))
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr.cast::<u8>(), s.len) };
        String::from_utf8_lossy(bytes)
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
                SVL_I64 if v.flags & SVL_F_UNSIGNED != 0 && v.u.u > i64::MAX as u64 => {
                    Value::U64(v.u.u)
                }
                SVL_I64 if v.flags & SVL_F_UNSIGNED != 0 => Value::I64(v.u.u as i64),
                SVL_I64 => Value::I64(v.u.i),
                SVL_F64 => Value::F64(v.u.f),
                SVL_HANDLE => Value::Handle(v.u.h),
                SVL_STR => Value::Str(str_of_raw(v).into_owned()),
                SVL_LIST => Value::List(Value::vec_from_raw(v.u.l.ptr, v.u.l.len)),
                SVL_ENUM => Value::Enum {
                    ty: v.type_,
                    value: v.u.i,
                },
                SVL_FLAGS => Value::Flags {
                    ty: v.type_,
                    mask: v.u.u,
                },
                SVL_RECORD => Value::Record {
                    ty: v.type_,
                    fields: Value::vec_from_raw(v.u.l.ptr, v.u.l.len),
                },
                SVL_BYTES => {
                    let b = v.u.bytes;
                    if b.ptr.is_null() || b.len == 0 {
                        Value::Bytes(Vec::new())
                    } else {
                        Value::Bytes(std::slice::from_raw_parts(b.ptr, b.len).to_vec())
                    }
                }
                SVL_BOXED => Value::Boxed {
                    ty: v.type_,
                    handle: v.u.h,
                },
                SVL_CALLBACK => Value::Callback {
                    id: v.u.u,
                    ty: v.type_,
                },
                SVL_MAP => {
                    let items = Value::vec_from_raw(v.u.l.ptr, v.u.l.len);
                    let mut it = items.into_iter();
                    let mut out = Vec::new();
                    while let (Some(k), Some(x)) = (it.next(), it.next()) {
                        let k = match k {
                            Value::Str(s) => s,
                            _ => String::new(),
                        };
                        out.push((k, x));
                    }
                    Value::Map(out)
                }
                SVL_ERROR => {
                    let mut it = Value::vec_from_raw(v.u.l.ptr, v.u.l.len).into_iter();
                    let domain = match it.next() {
                        Some(Value::Str(s)) => s,
                        _ => String::new(),
                    };
                    let code = it.next().unwrap_or(Value::Null);
                    let message = match it.next() {
                        Some(Value::Str(s)) => s,
                        _ => String::new(),
                    };
                    Value::Error(Box::new(NativeError {
                        domain,
                        code,
                        message,
                    }))
                }
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
    svl_value::null()
}

/// A borrowed, read-only view of a raw value. Cheap to copy; nothing is copied out of the
/// underlying buffers until you ask for an owned [`Value`].
#[derive(Clone, Copy)]
pub struct ValueRef<'a> {
    raw: &'a svl_value,
}

impl<'a> ValueRef<'a> {
    /// # Safety
    /// `raw` must satisfy the ABI contract: pointers valid for the tag, for `'a`.
    pub unsafe fn from_raw(raw: &'a svl_value) -> ValueRef<'a> {
        ValueRef { raw }
    }

    pub fn raw(&self) -> &'a svl_value {
        self.raw
    }
    pub fn tag(&self) -> u16 {
        self.raw.tag
    }
    pub fn flags(&self) -> u16 {
        self.raw.flags
    }
    pub fn type_id(&self) -> svl_id {
        self.raw.type_
    }
    pub fn is_null(&self) -> bool {
        self.raw.tag == SVL_NULL
    }
    pub fn as_bool(&self) -> Option<bool> {
        (self.raw.tag == SVL_BOOL).then(|| unsafe { self.raw.u.b })
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self.raw.tag {
            SVL_I64 | SVL_ENUM => Some(unsafe { self.raw.u.i }),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self.raw.tag {
            SVL_I64 if self.raw.flags & SVL_F_UNSIGNED != 0 => Some(unsafe { self.raw.u.u }),
            SVL_I64 => u64::try_from(unsafe { self.raw.u.i }).ok(),
            SVL_FLAGS | SVL_CALLBACK => Some(unsafe { self.raw.u.u }),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self.raw.tag {
            SVL_F64 => Some(unsafe { self.raw.u.f }),
            SVL_I64 => Some(unsafe { self.raw.u.i } as f64),
            _ => None,
        }
    }
    pub fn as_handle(&self) -> Option<svl_handle> {
        matches!(self.raw.tag, SVL_HANDLE | SVL_BOXED).then(|| unsafe { self.raw.u.h })
    }
    /// The text of a `STR`. Borrows when the value is UTF-8 and valid.
    pub fn as_str(&self) -> Option<Cow<'a, str>> {
        (self.raw.tag == SVL_STR).then(|| unsafe { str_of_raw(self.raw) })
    }
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        if self.raw.tag != SVL_BYTES {
            return None;
        }
        let b = unsafe { self.raw.u.bytes };
        Some(if b.ptr.is_null() || b.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(b.ptr, b.len) }
        })
    }
    /// The elements of a `LIST`/`RECORD`/`MAP`/`ERROR` (a MAP alternates keys and values).
    pub fn items(&self) -> &'a [svl_value] {
        match self.raw.tag {
            SVL_LIST | SVL_RECORD | SVL_MAP | SVL_ERROR => {
                let l = unsafe { self.raw.u.l };
                if l.ptr.is_null() || l.len == 0 {
                    &[]
                } else {
                    unsafe { std::slice::from_raw_parts(l.ptr, l.len) }
                }
            }
            _ => &[],
        }
    }
    pub fn item(&self, i: usize) -> Option<ValueRef<'a>> {
        self.items().get(i).map(|r| ValueRef { raw: r })
    }
    pub fn to_value(&self) -> Value {
        unsafe { Value::from_raw(self.raw) }
    }
}

/// A borrowed argument list.
#[derive(Clone, Copy)]
pub struct ArgsRef<'a> {
    raw: &'a [svl_value],
}

impl<'a> ArgsRef<'a> {
    /// # Safety
    /// `ptr` must be null (only if `n == 0`) or point to `n` valid values for `'a`.
    pub unsafe fn from_raw(ptr: *const svl_value, n: usize) -> ArgsRef<'a> {
        let raw = if ptr.is_null() || n == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(ptr, n) }
        };
        ArgsRef { raw }
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<ValueRef<'a>> {
        self.raw.get(i).map(|r| ValueRef { raw: r })
    }
    pub fn iter(&self) -> impl Iterator<Item = ValueRef<'a>> + 'a {
        self.raw.iter().map(|r| ValueRef { raw: r })
    }
    pub fn to_values(&self) -> Vec<Value> {
        self.iter().map(|v| v.to_value()).collect()
    }
}

// ---- descriptors (owned) ----

/// Owned counterpart of `svl_slot_desc`.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotDesc {
    pub name: Option<String>,
    pub tag: u16,
    pub flags: u16,
    pub ty: svl_id,
    /// `NaN` when unbounded.
    pub min: f64,
    pub max: f64,
}

impl SlotDesc {
    pub fn new(tag: u16) -> SlotDesc {
        SlotDesc {
            name: None,
            tag,
            flags: 0,
            ty: 0,
            min: f64::NAN,
            max: f64::NAN,
        }
    }
    pub fn named(mut self, name: &str) -> SlotDesc {
        self.name = Some(name.to_string());
        self
    }
    pub fn flags(mut self, f: u16) -> SlotDesc {
        self.flags |= f;
        self
    }
    pub fn typed(mut self, ty: svl_id) -> SlotDesc {
        self.ty = ty;
        self
    }
    pub fn range(mut self, min: f64, max: f64) -> SlotDesc {
        self.min = min;
        self.max = max;
        self
    }
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
    pub fn is_void(&self) -> bool {
        self.tag == SVL_NULL && self.name.is_none()
    }

    unsafe fn from_c(d: &svl_slot_desc) -> SlotDesc {
        SlotDesc {
            name: unsafe { cstr_opt(d.name) },
            tag: d.tag,
            flags: d.flags,
            ty: d.type_,
            min: d.min,
            max: d.max,
        }
    }
}

unsafe fn cstr_opt(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

/// Owned counterpart of `svl_member_desc`.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberDesc {
    pub name: String,
    pub params: Vec<SlotDesc>,
    pub ret: SlotDesc,
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumMember {
    pub name: String,
    pub value: i64,
}

/// Owned counterpart of `svl_type_desc`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDesc {
    pub name: String,
    /// `SVL_ENUM | SVL_FLAGS | SVL_RECORD | SVL_BOXED | SVL_CALLBACK | SVL_LIST`
    pub kind: u16,
    pub members: Vec<EnumMember>,
    pub fields: Vec<SlotDesc>,
    pub ret: SlotDesc,
}

/// Owned storage behind the C structs of one descriptor, kept alive for the backend's lifetime.
pub struct CMember {
    _strings: Vec<CString>,
    _params: Vec<svl_slot_desc>,
    desc: svl_member_desc,
}

pub struct CType {
    _strings: Vec<CString>,
    _members: Vec<svl_enum_member>,
    _fields: Vec<svl_slot_desc>,
    desc: svl_type_desc,
}

fn cstring(strings: &mut Vec<CString>, s: &str) -> *const c_char {
    let c = CString::new(s.replace('\0', " ")).unwrap_or_default();
    let p = c.as_ptr();
    // A CString's buffer does not move when the CString does.
    strings.push(c);
    p
}

fn slot_to_c(strings: &mut Vec<CString>, s: &SlotDesc) -> svl_slot_desc {
    svl_slot_desc {
        name: s.name.as_deref().map_or(ptr::null(), |n| cstring(strings, n)),
        tag: s.tag,
        flags: s.flags,
        type_: s.ty,
        min: s.min,
        max: s.max,
    }
}

impl MemberDesc {
    fn to_c(&self) -> Box<CMember> {
        let mut strings = Vec::new();
        let name = cstring(&mut strings, &self.name);
        let params: Vec<svl_slot_desc> = self.params.iter().map(|p| slot_to_c(&mut strings, p)).collect();
        let ret = slot_to_c(&mut strings, &self.ret);
        let desc = svl_member_desc {
            name,
            params: params.as_ptr(),
            n_params: params.len() as u32,
            ret,
            flags: self.flags,
        };
        Box::new(CMember {
            _strings: strings,
            _params: params,
            desc,
        })
    }
}

impl TypeDesc {
    fn to_c(&self) -> Box<CType> {
        let mut strings = Vec::new();
        let name = cstring(&mut strings, &self.name);
        let members: Vec<svl_enum_member> = self
            .members
            .iter()
            .map(|m| svl_enum_member {
                name: cstring(&mut strings, &m.name),
                value: m.value,
            })
            .collect();
        let fields: Vec<svl_slot_desc> = self.fields.iter().map(|f| slot_to_c(&mut strings, f)).collect();
        let ret = slot_to_c(&mut strings, &self.ret);
        let desc = svl_type_desc {
            name,
            kind: self.kind,
            reserved: 0,
            members: members.as_ptr(),
            n_members: members.len() as u32,
            fields: fields.as_ptr(),
            n_fields: fields.len() as u32,
            ret,
        };
        Box::new(CType {
            _strings: strings,
            _members: members,
            _fields: fields,
            desc,
        })
    }

    unsafe fn from_c(d: &svl_type_desc) -> TypeDesc {
        unsafe {
            let members = if d.members.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(d.members, d.n_members as usize)
                    .iter()
                    .map(|m| EnumMember {
                        name: cstr_opt(m.name).unwrap_or_default(),
                        value: m.value,
                    })
                    .collect()
            };
            let fields = if d.fields.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(d.fields, d.n_fields as usize)
                    .iter()
                    .map(|f| SlotDesc::from_c(f))
                    .collect()
            };
            TypeDesc {
                name: cstr_opt(d.name).unwrap_or_default(),
                kind: d.kind,
                members,
                fields,
                ret: SlotDesc::from_c(&d.ret),
            }
        }
    }
}

impl MemberDesc {
    unsafe fn from_c(d: &svl_member_desc) -> MemberDesc {
        unsafe {
            let params = if d.params.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(d.params, d.n_params as usize)
                    .iter()
                    .map(|p| SlotDesc::from_c(p))
                    .collect()
            };
            MemberDesc {
                name: cstr_opt(d.name).unwrap_or_default(),
                params,
                ret: SlotDesc::from_c(&d.ret),
                flags: d.flags,
            }
        }
    }
}

/// Owns the C structs behind descriptors a backend publishes, so the pointers stay valid for the
/// backend's lifetime. Each descriptor is built once, on first request.
#[derive(Default)]
pub struct DescStore {
    members: RefCell<HashMap<(u32, svl_id, svl_id), Option<Box<CMember>>>>,
    types: RefCell<HashMap<svl_id, Option<Box<CType>>>>,
}

impl DescStore {
    /// The C descriptor of `(what, kind, member)`; `build` runs only the first time. Null when
    /// `build` returns `None`.
    pub fn member(
        &self,
        key: (u32, svl_id, svl_id),
        build: impl FnOnce() -> Option<MemberDesc>,
    ) -> *const svl_member_desc {
        let mut cache = self.members.borrow_mut();
        let e = cache.entry(key).or_insert_with(|| build().map(|d| d.to_c()));
        e.as_ref().map_or(ptr::null(), |c| &c.desc as *const _)
    }

    pub fn type_desc(&self, id: svl_id, build: impl FnOnce() -> Option<TypeDesc>) -> *const svl_type_desc {
        let mut cache = self.types.borrow_mut();
        let e = cache.entry(id).or_insert_with(|| build().map(|d| d.to_c()));
        e.as_ref().map_or(ptr::null(), |c| &c.desc as *const _)
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
            return Err(AbiError::plain(
                -1,
                format!("backend ABI version {v}, host expects {SVL_ABI_VERSION}"),
            ));
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

    fn fail(&self, rc: c_int, out: Option<&svl_value>) -> AbiError {
        let native = out.and_then(|o| {
            (o.tag == SVL_ERROR).then(|| match unsafe { Value::from_raw(o) } {
                Value::Error(e) => *e,
                _ => unreachable!("SVL_ERROR decodes to Value::Error"),
            })
        });
        AbiError {
            code: rc,
            message: match &native {
                Some(n) if !n.message.is_empty() => n.message.clone(),
                _ => self.last_error(),
            },
            native,
        }
    }

    fn check(&self, rc: c_int) -> AbiResult<()> {
        if rc < 0 { Err(self.fail(rc, None)) } else { Ok(()) }
    }

    /// Capability bits (`SVL_CAP_*`).
    pub fn caps(&self) -> u32 {
        self.b().caps
    }

    /// Returns 0 for unknown names.
    pub fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let b = self.b();
        unsafe { (b.resolve)(b.self_, what as u32, kind, name.as_ptr().cast(), name.len()) }
    }

    /// The descriptor of a member (prop, signal, method), if the backend publishes any.
    pub fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        let b = self.b();
        let f = b.describe?;
        let p = unsafe { f(b.self_, what as u32, kind, member) };
        (!p.is_null()).then(|| unsafe { MemberDesc::from_c(&*p) })
    }

    pub fn describe_type(&self, ty: svl_id) -> Option<TypeDesc> {
        let b = self.b();
        let f = b.describe_type?;
        let p = unsafe { f(b.self_, ty) };
        (!p.is_null()).then(|| unsafe { TypeDesc::from_c(&*p) })
    }

    /// Async: cancel the operation that will complete `callback`. False if already done or
    /// unsupported.
    pub fn cancel(&self, callback: u64) -> bool {
        let b = self.b();
        match b.cancel {
            Some(f) => unsafe { f(b.self_, callback) > 0 },
            None => false,
        }
    }

    pub fn create(&self, kind: svl_id) -> AbiResult<svl_handle> {
        let b = self.b();
        let h = unsafe { (b.create)(b.self_, kind) };
        if h == 0 {
            Err(AbiError::plain(-1, self.last_error()))
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
        v.with_raw(|raw| self.set_prop_raw(h, prop, raw))
    }

    /// Sets a property from a raw value; the host's borrowed-string path.
    pub fn set_prop_raw(&self, h: svl_handle, prop: svl_id, v: &svl_value) -> AbiResult<()> {
        let b = self.b();
        self.check(unsafe { (b.set_prop)(b.self_, h, prop, v) })
    }

    pub fn get_prop(&self, h: svl_handle, prop: svl_id) -> AbiResult<Value> {
        let raw = self.get_prop_raw(h, prop)?;
        Ok(unsafe { Value::from_raw(&raw) })
    }

    /// Reads a property into a raw value, valid until the next backend call.
    pub fn get_prop_raw(&self, h: svl_handle, prop: svl_id) -> AbiResult<svl_value> {
        let b = self.b();
        let mut out = null_value();
        let rc = unsafe { (b.get_prop)(b.self_, h, prop, &mut out) };
        if rc < 0 {
            return Err(self.fail(rc, Some(&out)));
        }
        Ok(out)
    }

    pub fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> AbiResult<Value> {
        let raw = RawValues::new(args);
        let out = self.call_raw(h, method, raw.as_slice())?;
        Ok(unsafe { Value::from_raw(&out) })
    }

    /// Calls a method with raw arguments; the result is valid until the next backend call.
    pub fn call_raw(&self, h: svl_handle, method: svl_id, args: &[svl_value]) -> AbiResult<svl_value> {
        let b = self.b();
        let mut out = null_value();
        let rc = unsafe { (b.call)(b.self_, h, method, args.as_ptr(), args.len(), &mut out) };
        if rc < 0 {
            return Err(self.fail(rc, Some(&out)));
        }
        Ok(out)
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
            return Err(AbiError::plain(rc, "call_sync failed"));
        }
        Ok(unsafe { Value::from_raw(&out) })
    }

    pub fn on_destroyed(&self, h: svl_handle) {
        let host = self.h();
        unsafe { (host.on_destroyed)(host.self_, h) }
    }

    /// Runs the JS function behind a `CALLBACK` value. A dead id is an error, not a crash.
    pub fn invoke_callback(&self, callback: u64, args: &[Value]) -> AbiResult<Value> {
        let host = self.h();
        let raw = RawValues::new(args);
        let mut out = null_value();
        let rc = unsafe {
            (host.invoke_callback)(host.self_, callback, raw.as_ptr(), raw.len(), &mut out)
        };
        if rc < 0 {
            return Err(AbiError::plain(rc, "invoke_callback failed"));
        }
        Ok(unsafe { Value::from_raw(&out) })
    }

    /// The native side dropped its reference to a callback. Idempotent.
    pub fn release_callback(&self, callback: u64) {
        let host = self.h();
        unsafe { (host.release_callback)(host.self_, callback) }
    }
}

pub type Waker = Arc<dyn Fn() + Send + Sync>;
pub type StrResult<T> = std::result::Result<T, String>;

/// Implement this to provide a backend in Rust, then call [`BackendVtable::new`].
///
/// Errors are plain strings; the vtable stores them for `last_error` and returns `-1`. A method
/// that returns `Ok(Value::Error(..))` is turned into rc `-1` plus an `SVL_ERROR` out value.
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
    /// Owns the toolkit loop. Returns when the app quits.
    fn run(&self, host: &HostRef<'_>) -> StrResult<()>;
    /// Called once when the vtable is built. The returned closure implements `wake`: it may be
    /// called from any thread and must schedule a host checkpoint on the loop thread.
    fn waker(&self) -> Waker;
    fn quit(&self);

    // ---- v3 ----

    /// `SVL_CAP_*` bits.
    fn caps(&self) -> u32 {
        0
    }
    /// Type descriptor of a prop, signal or method. `None`: the host converts dynamically.
    fn describe(&self, _what: NameKind, _kind: svl_id, _member: svl_id) -> Option<MemberDesc> {
        None
    }
    fn describe_type(&self, _ty: svl_id) -> Option<TypeDesc> {
        None
    }
    /// Cancel the async operation completing `callback`; true if it was still running.
    fn cancel(&self, _callback: u64) -> bool {
        false
    }
    /// Borrowed variant of [`Backend::set_prop`]: override to read strings and lists without
    /// copying them into an owned [`Value`]. The default copies.
    fn set_prop_ref(&self, h: svl_handle, prop: svl_id, v: ValueRef<'_>) -> StrResult<()> {
        self.set_prop(h, prop, &v.to_value())
    }
    /// Borrowed variant of [`Backend::call`].
    fn call_ref(&self, h: svl_handle, method: svl_id, args: ArgsRef<'_>) -> StrResult<Value> {
        self.call(h, method, &args.to_values())
    }
}

type OutSlot = RefCell<Option<(Box<Value>, Vec<Vec<svl_value>>)>>;

struct Shim {
    // `wake` reads only this field, possibly from another thread.
    waker: Waker,
    backend: Box<dyn Backend>,
    last_error: RefCell<CString>,
    out: OutSlot,
    descs: DescStore,
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

    /// Runs a value-returning backend call and writes its result (or `SVL_ERROR`) to `out`.
    fn rc_value(&self, out: *mut svl_value, f: impl FnOnce(&dyn Backend) -> StrResult<Value>) -> c_int {
        match catch_unwind(AssertUnwindSafe(|| f(&*self.backend))) {
            Ok(Ok(Value::Error(e))) => {
                let rc = self.fail(&e.message);
                write_out(&self.out, out, Value::Error(e));
                rc
            }
            Ok(Ok(v)) => {
                write_out(&self.out, out, v);
                0
            }
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
    let v = unsafe { ValueRef::from_raw(&*v) };
    s.rc(|b| b.set_prop_ref(h, prop, v))
}

unsafe extern "C" fn t_get_prop(
    s: *mut c_void,
    h: svl_handle,
    prop: svl_id,
    out: *mut svl_value,
) -> c_int {
    let s = unsafe { shim(s) };
    s.rc_value(out, |b| b.get_prop(h, prop))
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
    let args = unsafe { ArgsRef::from_raw(args, n) };
    s.rc_value(out, |b| b.call_ref(h, method, args))
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

unsafe extern "C" fn t_quit(s: *mut c_void) {
    let s = unsafe { shim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.backend.quit()));
}

unsafe extern "C" fn t_last_error(s: *mut c_void) -> *const c_char {
    let s = unsafe { shim(s) };
    // The CString buffer stays put until the next error replaces it.
    unsafe { (*s.last_error.as_ptr()).as_ptr() }
}

unsafe extern "C" fn t_describe(
    s: *mut c_void,
    what: u32,
    kind: svl_id,
    member: svl_id,
) -> *const svl_member_desc {
    let s = unsafe { shim(s) };
    let Some(w) = NameKind::from_u32(what) else {
        return ptr::null();
    };
    s.descs.member((what, kind, member), || {
        catch_unwind(AssertUnwindSafe(|| s.backend.describe(w, kind, member)))
            .ok()
            .flatten()
    })
}

unsafe extern "C" fn t_describe_type(s: *mut c_void, ty: svl_id) -> *const svl_type_desc {
    let s = unsafe { shim(s) };
    s.descs.type_desc(ty, || {
        catch_unwind(AssertUnwindSafe(|| s.backend.describe_type(ty)))
            .ok()
            .flatten()
    })
}

unsafe extern "C" fn t_cancel(s: *mut c_void, callback: u64) -> c_int {
    let s = unsafe { shim(s) };
    catch_unwind(AssertUnwindSafe(|| s.backend.cancel(callback))).unwrap_or(false) as c_int
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
        let caps = backend.caps();
        let shim = Box::into_raw(Box::new(Shim {
            waker,
            backend: Box::new(backend),
            last_error: RefCell::new(CString::default()),
            out: RefCell::new(None),
            descs: DescStore::default(),
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
            run: t_run,
            wake: t_wake,
            quit: t_quit,
            last_error: t_last_error,
            reserved_v2_schedule_wake: ptr::null(),
            reserved_v2_handle_info: ptr::null(),
            caps,
            describe: Some(t_describe),
            describe_type: Some(t_describe_type),
            cancel: Some(t_cancel),
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
    fn on_destroyed(&self, h: svl_handle);
    /// Run the JS function behind a callback id. The default refuses.
    fn invoke_callback(&self, _callback: u64, _args: &[Value]) -> StrResult<Value> {
        Err("callbacks are not supported by this host".into())
    }
    /// The native side dropped its reference to a callback.
    fn release_callback(&self, _callback: u64) {}
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

unsafe extern "C" fn h_on_destroyed(s: *mut c_void, h: svl_handle) {
    let s = unsafe { hshim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.cb.on_destroyed(h)));
}

unsafe extern "C" fn h_invoke_callback(
    s: *mut c_void,
    id: u64,
    args: *const svl_value,
    n: usize,
    out: *mut svl_value,
) -> c_int {
    let s = unsafe { hshim(s) };
    let args = unsafe { Value::vec_from_raw(args, n) };
    match catch_unwind(AssertUnwindSafe(|| s.cb.invoke_callback(id, &args))) {
        Ok(Ok(v)) => {
            write_out(&s.out, out, v);
            0
        }
        _ => -1,
    }
}

unsafe extern "C" fn h_release_callback(s: *mut c_void, id: u64) {
    let s = unsafe { hshim(s) };
    let _ = catch_unwind(AssertUnwindSafe(|| s.cb.release_callback(id)));
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
            invoke_callback: h_invoke_callback,
            release_callback: h_release_callback,
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
