//! Dynamic invocation through girepository-2.0 (plans/milestones/X1-borrowed-gi.md, extended by the
//! NT0 pilot of plans/research/NON-GUI-TYPES.md).
//!
//! `resolve` finds a function in the loaded typelibs (Gtk, Gdk, Gsk, Gio, GLib, GObject, Pango,
//! Graphene) and builds an argument/return plan once per `(kind, name)`; `call` converts the ABI
//! values with that plan and invokes the function with `gi_function_info_invoke`, which drives
//! libffi. A plan that uses an unsupported feature is kept (with its error) so that `call` can say
//! exactly what is unsupported.
//!
//! Supported: scalars, strings, enums and flags, objects, boxed structs (by pointer), GStrv,
//! GType (as a native type name), unichar, GList/GSList and C arrays (zero-terminated, with a length
//! argument, or byte arrays) of strings, objects, boxed values and numbers, out and inout
//! parameters (callee-allocated and caller-allocated structs), static functions and constructors,
//! and receivers that are objects or boxed values. Not supported (named in the error): callbacks,
//! unions, structs without a boxed GType or passed by value, GVariant, hash tables, GArray and
//! GPtrArray, fundamental classes such as GskRenderNode.

use crate::convert::{as_str, to_value, Env};
use crate::types::{self, TypeTable};
use gtk::glib::{self, gobject_ffi, translate::*, Type};
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::rc::Rc;
use svelters_abi::*;

pub(crate) type Info = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) union GIArgument {
    pub(crate) v_boolean: c_int,
    pub(crate) v_int8: i8,
    pub(crate) v_uint8: u8,
    pub(crate) v_int16: i16,
    pub(crate) v_uint16: u16,
    pub(crate) v_int32: i32,
    pub(crate) v_uint32: u32,
    pub(crate) v_int64: i64,
    pub(crate) v_uint64: u64,
    pub(crate) v_float: f32,
    pub(crate) v_double: f64,
    pub(crate) v_pointer: *mut c_void,
}

pub(crate) const ZERO: GIArgument = GIArgument { v_uint64: 0 };

const TAG_VOID: c_int = 0;
const TAG_BOOLEAN: c_int = 1;
const TAG_INT8: c_int = 2;
const TAG_UINT8: c_int = 3;
const TAG_INT16: c_int = 4;
const TAG_UINT16: c_int = 5;
const TAG_INT32: c_int = 6;
const TAG_UINT32: c_int = 7;
const TAG_INT64: c_int = 8;
const TAG_UINT64: c_int = 9;
const TAG_FLOAT: c_int = 10;
const TAG_DOUBLE: c_int = 11;
const TAG_GTYPE: c_int = 12;
const TAG_UTF8: c_int = 13;
const TAG_FILENAME: c_int = 14;
const TAG_ARRAY: c_int = 15;
const TAG_INTERFACE: c_int = 16;
const TAG_GLIST: c_int = 17;
const TAG_GSLIST: c_int = 18;
const TAG_GHASH: c_int = 19;
const TAG_ERROR: c_int = 20;
const TAG_UNICHAR: c_int = 21;

const DIR_IN: c_int = 0;
const DIR_OUT: c_int = 1;
const DIR_INOUT: c_int = 2;
const TRANSFER_CONTAINER: c_int = 1;
const TRANSFER_EVERYTHING: c_int = 2;
const FN_IS_CONSTRUCTOR: c_int = 1 << 1;

/// The typelibs the pilot binds: (namespace, version). Gtk is required, the rest are best effort.
const NAMESPACES: &[(&str, &str)] =
    &[("Gtk", "4.0"), ("Gdk", "4.0"), ("Gsk", "4.0"), ("Gio", "2.0"), ("GLib", "2.0"), ("GObject", "2.0"), ("Pango", "1.0"), ("Graphene", "1.0")];

#[link(name = "girepository-2.0")]
unsafe extern "C" {
    fn gi_repository_new() -> *mut c_void;
    fn gi_repository_require(repo: *mut c_void, ns: *const c_char, version: *const c_char, flags: c_int, err: *mut *mut glib::ffi::GError) -> *mut c_void;
    fn gi_repository_find_by_gtype(repo: *mut c_void, gtype: glib::ffi::GType) -> Info;
    fn gi_repository_find_by_name(repo: *mut c_void, ns: *const c_char, name: *const c_char) -> Info;
    fn gi_object_info_find_method(info: Info, name: *const c_char) -> Info;
    fn gi_object_info_get_n_interfaces(info: Info) -> c_uint;
    fn gi_object_info_get_interface(info: Info, n: c_uint) -> Info;
    fn gi_interface_info_find_method(info: Info, name: *const c_char) -> Info;
    fn gi_struct_info_find_method(info: Info, name: *const c_char) -> Info;
    fn gi_struct_info_get_size(info: Info) -> usize;
    fn gi_function_info_get_flags(info: Info) -> c_int;
    fn gi_function_info_get_symbol(info: Info) -> *const c_char;
    fn gi_callable_info_get_n_args(info: Info) -> c_uint;
    fn gi_callable_info_get_arg(info: Info, n: c_uint) -> Info;
    fn gi_callable_info_get_return_type(info: Info) -> Info;
    fn gi_callable_info_get_caller_owns(info: Info) -> c_int;
    fn gi_callable_info_may_return_null(info: Info) -> c_int;
    fn gi_callable_info_can_throw_gerror(info: Info) -> c_int;
    fn gi_callable_info_is_method(info: Info) -> c_int;
    fn gi_callable_info_skip_return(info: Info) -> c_int;
    fn gi_arg_info_get_direction(info: Info) -> c_int;
    fn gi_arg_info_get_ownership_transfer(info: Info) -> c_int;
    fn gi_arg_info_may_be_null(info: Info) -> c_int;
    fn gi_arg_info_is_caller_allocates(info: Info) -> c_int;
    fn gi_arg_info_get_type_info(info: Info) -> Info;
    fn gi_type_info_get_tag(info: Info) -> c_int;
    fn gi_type_info_get_interface(info: Info) -> Info;
    fn gi_type_info_get_array_type(info: Info) -> c_int;
    fn gi_type_info_is_zero_terminated(info: Info) -> c_int;
    fn gi_type_info_is_pointer(info: Info) -> c_int;
    fn gi_type_info_get_array_length_index(info: Info, idx: *mut c_uint) -> c_int;
    fn gi_type_info_get_array_fixed_size(info: Info, size: *mut usize) -> c_int;
    fn gi_type_info_get_param_type(info: Info, n: c_uint) -> Info;
    fn gi_base_info_unref(info: Info);
    fn gi_base_info_ref(info: Info) -> Info;
    fn gi_base_info_get_name(info: Info) -> *const c_char;
    fn gi_base_info_get_namespace(info: Info) -> *const c_char;
    fn gi_base_info_get_container(info: Info) -> Info;
    fn gi_registered_type_info_get_g_type(info: Info) -> glib::ffi::GType;
    fn gi_arg_info_get_scope(info: Info) -> c_int;
    fn gi_arg_info_get_closure_index(info: Info, out: *mut c_uint) -> c_int;
    fn gi_arg_info_get_destroy_index(info: Info, out: *mut c_uint) -> c_int;
    fn gi_repository_find_by_error_domain(repo: *mut c_void, domain: u32) -> Info;
    pub(crate) fn gi_callable_info_create_closure(
        info: Info,
        cif: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void, *mut c_void),
        user_data: *mut c_void,
    ) -> *mut c_void;
    pub(crate) fn gi_callable_info_get_closure_native_address(info: Info, closure: *mut c_void) -> *mut c_void;
    pub(crate) fn gi_callable_info_destroy_closure(info: Info, closure: *mut c_void);
    fn gi_function_info_invoke(
        info: Info,
        in_args: *const GIArgument,
        n_in: usize,
        out_args: *const GIArgument,
        n_out: usize,
        ret: *mut GIArgument,
        err: *mut *mut glib::ffi::GError,
    ) -> c_int;
}

/// Owns one reference to a GIBaseInfo.
pub(crate) struct Owned(pub(crate) Info);

impl Owned {
    pub(crate) fn new(p: Info) -> Option<Owned> {
        (!p.is_null()).then(|| Owned(p))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { gi_base_info_unref(self.0) };
    }
}

/// How a C array's length is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ArrLen {
    Zero,
    /// The native argument at this index holds the length.
    Arg(usize),
    Fixed(usize),
}

/// The wire type of one argument or return value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Void,
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Unichar,
    /// A GType, carried as a native type name (`SVL_T_TYPE`).
    Gtype,
    Str,
    Enum(Type),
    Flags(Type),
    Object(Type),
    /// A boxed type with a GType (records such as GdkRGBA, opaque boxed values), by pointer.
    Boxed(Type),
    /// A NULL-terminated array of UTF-8 strings.
    Strv,
    /// GList (false) / GSList (true) of elements held by pointer.
    List(Box<Ty>, bool),
    /// A C array.
    Array(Box<Ty>, ArrLen),
    /// A JS function: index into `Plan::callbacks` (N1, GI closures by scope).
    Callback(usize),
    /// A `GDestroyNotify`, a `GAsyncReadyCallback` or an untyped pointer (callback user data): filled by
    /// the invoker, never by JS.
    Internal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dir {
    In,
    Out,
    InOut,
}

#[derive(Clone, Debug)]
pub(crate) struct Slot {
    pub(crate) ty: Ty,
    pub(crate) nullable: bool,
    /// `transfer: full`: the callee takes (arguments) or hands over (returns) the elements.
    pub(crate) full: bool,
    /// `transfer: container`: the callee takes only the container.
    pub(crate) container: bool,
    pub(crate) dir: Dir,
    /// An out struct the caller allocates (`caller-allocates`), with its size.
    pub(crate) caller_alloc: Option<usize>,
}

impl Slot {
    fn plain(ty: Ty) -> Slot {
        Slot { ty, nullable: false, full: false, container: false, dir: Dir::In, caller_alloc: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    /// Visible to JS.
    Js,
    /// The length of the array argument at this native index; JS never sees it.
    Len(usize),
    /// The user data of the callback argument at this native index: the closure itself.
    UserData(usize),
    /// The `GDestroyNotify` of the callback argument at this native index.
    Destroy(usize),
    /// Async machinery the invoker supplies (`GCancellable`, `GAsyncReadyCallback` and its user data,
    /// the `GAsyncResult` of a finish function).
    Hidden,
}

struct PlanArg {
    name: String,
    slot: Slot,
    role: Role,
    in_pos: Option<usize>,
    out_pos: Option<usize>,
}

/// A callback argument (a JS function): its type, scope and the arguments it pairs with.
pub(crate) struct CbSpec {
    /// The `GICallbackInfo`.
    pub(crate) info: Rc<Owned>,
    /// `Gtk.CustomFilterFunc`.
    pub(crate) name: String,
    pub(crate) sig: Rc<CbSig>,
    /// `GI_SCOPE_TYPE_*` as the GIR declares it.
    pub(crate) scope: c_int,
    pub(crate) user_data: Option<usize>,
    pub(crate) destroy: Option<usize>,
}

/// The signature of a callback type: what the trampoline converts.
pub(crate) struct CbSig {
    /// One per C argument; `None` for the user data pointer.
    pub(crate) args: Vec<Option<(String, Slot)>>,
    pub(crate) ret: Slot,
}

/// The `GAsyncReadyCallback` arguments of an `_async` function (folded with its `_finish`).
#[derive(Clone, Copy)]
struct AsyncArgs {
    callback: usize,
    user_data: Option<usize>,
    cancellable: Option<usize>,
}

pub(crate) const SCOPE_CALL: c_int = 1;
pub(crate) const SCOPE_ASYNC: c_int = 2;
pub(crate) const SCOPE_NOTIFIED: c_int = 3;
pub(crate) const SCOPE_FOREVER: c_int = 4;

struct Plan {
    args: Vec<PlanArg>,
    ret: Slot,
    throws: bool,
    is_method: bool,
    is_ctor: bool,
    n_in: usize,
    n_out: usize,
    callbacks: Vec<CbSpec>,
    async_args: Option<AsyncArgs>,
    /// The native index of the `GAsyncResult` argument of a finish function.
    async_result: Option<usize>,
}

impl Plan {
    fn has_outs(&self) -> bool {
        self.args.iter().any(|a| a.role == Role::Js && a.slot.dir != Dir::In)
    }
}

/// One cached typelib function.
pub struct DynMethod {
    name: String,
    info: Owned,
    declaring: Type,
    plan: Result<Plan, String>,
    /// For an `_async` function: its `_finish` function.
    finish: Option<Rc<DynMethod>>,
}

/// A converted value. Objects carry whether the call handed over a reference.
pub enum Ret {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// An enum, flags or boxed value as a `GValue`, converted like a property of that type.
    Value(glib::Value),
    Strv(Vec<String>),
    Object(glib::Object, bool),
    List(Vec<Ret>),
    Bytes(Vec<u8>),
    /// A GType.
    Gtype(Type),
}

/// The result of a call: the return value and one value per out/inout parameter.
pub struct Called {
    pub ret: Ret,
    pub outs: Vec<Ret>,
}

/// A failed invocation: a message, or the `GError` the native function raised.
pub enum GiErr {
    Plain(String),
    Gerror(glib::Error),
}

impl From<String> for GiErr {
    fn from(s: String) -> GiErr {
        GiErr::Plain(s)
    }
}

impl From<&str> for GiErr {
    fn from(s: &str) -> GiErr {
        GiErr::Plain(s.to_string())
    }
}

pub struct Gi {
    repo: *mut c_void,
    cache: RefCell<HashMap<(u32, String), u32>>,
    by_type: RefCell<HashMap<(Type, String), Option<u32>>>,
    methods: RefCell<Vec<Rc<DynMethod>>>,
    /// Callback type descriptors, by (callback type name, host scope); ids from `CB_TYPE_BASE`.
    cb_types: RefCell<Vec<(String, u16, TypeDesc)>>,
}

/// Type ids of callback signatures (`CALLBACK` slots of typelib methods).
pub const CB_TYPE_BASE: u32 = SVL_T_FIRST_BACKEND + 0x0100_0000;
/// The completion callback the host appends to a folded async method (`SVL_M_ASYNC`).
pub const T_ASYNC_DONE: u32 = SVL_T_FIRST_BACKEND + 0x00FF_0000;

/// The descriptor of [`T_ASYNC_DONE`]: one result value (the finish result, the folded out
/// parameters as an object, or an `ERROR`).
pub fn async_done_desc() -> TypeDesc {
    TypeDesc {
        name: "(result: unknown) => void".into(),
        kind: SVL_CALLBACK,
        scope: SVL_CB_ASYNC,
        members: vec![],
        fields: vec![SlotDesc::new(SVL_NULL).named("result")],
        ret: SlotDesc::new(SVL_NULL),
    }
}

/// The host scope a GIR scope maps to. `forever` is reported as `notified`: the host refuses
/// `forever`, the native closure is never freed, and the JS function lives until its owner dies.
fn host_scope(scope: c_int) -> u16 {
    match scope {
        SCOPE_CALL => SVL_CB_CALL,
        SCOPE_ASYNC => SVL_CB_ASYNC,
        _ => SVL_CB_NOTIFIED,
    }
}

fn tag_ty(tag: c_int, what: &str) -> Option<Ty> {
    Some(match tag {
        TAG_VOID => Ty::Void,
        TAG_BOOLEAN => Ty::Bool,
        TAG_INT8 => Ty::I8,
        TAG_UINT8 => Ty::U8,
        TAG_INT16 => Ty::I16,
        TAG_UINT16 => Ty::U16,
        TAG_INT32 => Ty::I32,
        TAG_UINT32 => Ty::U32,
        TAG_INT64 => Ty::I64,
        TAG_UINT64 => Ty::U64,
        TAG_FLOAT => Ty::F32,
        TAG_DOUBLE => Ty::F64,
        TAG_UTF8 | TAG_FILENAME => Ty::Str,
        TAG_GTYPE => Ty::Gtype,
        TAG_UNICHAR => Ty::Unichar,
        _ => {
            let _ = what;
            return None;
        }
    })
}

/// The element type of a list or array, which must be held by pointer or be a number.
fn elem_ty(ti: Info, what: &str, list: bool) -> Result<Ty, String> {
    // The elements of a GList/GSList are always pointers, whatever the element type info says.
    let ty = type_info_ty_ref(ti, what, list)?;
    match ty {
        Ty::Str | Ty::Object(_) | Ty::Boxed(_) | Ty::Enum(_) | Ty::Flags(_) => Ok(ty),
        Ty::Bool | Ty::I8 | Ty::U8 | Ty::I16 | Ty::U16 | Ty::I32 | Ty::U32 | Ty::I64 | Ty::U64 | Ty::F32 | Ty::F64 => Ok(ty),
        other => Err(format!("{what}: lists and arrays of {other:?} are not supported")),
    }
}

fn type_info_ty(ti: Info, what: &str) -> Result<Ty, String> {
    type_info_ty_ref(ti, what, false)
}

/// `by_ref`: the value is reached through a pointer the argument direction supplies (a
/// caller-allocated out struct), so a struct type that is not itself a pointer is fine.
fn type_info_ty_ref(ti: Info, what: &str, by_ref: bool) -> Result<Ty, String> {
    let tag = unsafe { gi_type_info_get_tag(ti) };
    if let Some(t) = tag_ty(tag, what) {
        return Ok(t);
    }
    Ok(match tag {
        TAG_ARRAY => {
            let kind = unsafe { gi_type_info_get_array_type(ti) };
            if kind != 0 {
                return Err(format!("{what}: GArray, GPtrArray and GByteArray values are not supported"));
            }
            let elem = Owned::new(unsafe { gi_type_info_get_param_type(ti, 0) }).ok_or_else(|| format!("{what}: array without an element type"))?;
            let et = elem_ty(elem.0, what, false)?;
            let mut idx: c_uint = 0;
            let mut fixed: usize = 0;
            if unsafe { gi_type_info_get_array_length_index(ti, &mut idx) } != 0 {
                Ty::Array(Box::new(et), ArrLen::Arg(idx as usize))
            } else if unsafe { gi_type_info_get_array_fixed_size(ti, &mut fixed) } != 0 {
                Ty::Array(Box::new(et), ArrLen::Fixed(fixed))
            } else if unsafe { gi_type_info_is_zero_terminated(ti) } != 0 {
                if et == Ty::Str { Ty::Strv } else { Ty::Array(Box::new(et), ArrLen::Zero) }
            } else {
                return Err(format!("{what}: arrays without a known length are not supported"));
            }
        }
        TAG_GLIST | TAG_GSLIST => {
            let elem = Owned::new(unsafe { gi_type_info_get_param_type(ti, 0) }).ok_or_else(|| format!("{what}: untyped list"))?;
            let et = elem_ty(elem.0, what, true)?;
            Ty::List(Box::new(et), tag == TAG_GSLIST)
        }
        TAG_GHASH => return Err(format!("{what}: hash table values are not supported")),
        TAG_ERROR => return Err(format!("{what}: GError values are not supported")),
        TAG_INTERFACE => {
            let iface = Owned::new(unsafe { gi_type_info_get_interface(ti) }).ok_or_else(|| format!("{what}: interface type has no info"))?;
            let tname = unsafe { CStr::from_ptr(gobject_ffi::g_type_name_from_instance(iface.0 as *mut gobject_ffi::GTypeInstance)) }.to_string_lossy().into_owned();
            match tname.as_str() {
                "GICallbackInfo" => return Err(format!("{what}: callbacks are not supported")),
                "GIUnionInfo" => return Err(format!("{what}: unions are not supported")),
                "GIStructInfo" => {
                    let t: Type = unsafe { from_glib(gi_registered_type_info_get_g_type(iface.0)) };
                    if !t.is_a(Type::BOXED) {
                        return Err(format!("{what}: structs without a boxed GType are not supported"));
                    }
                    if unsafe { gi_type_info_is_pointer(ti) } == 0 && !by_ref {
                        return Err(format!("{what}: structs passed by value are not supported"));
                    }
                    Ty::Boxed(t)
                }
                "GIObjectInfo" | "GIInterfaceInfo" | "GIEnumInfo" | "GIFlagsInfo" => {
                    let gt = unsafe { gi_registered_type_info_get_g_type(iface.0) };
                    let t: Type = unsafe { from_glib(gt) };
                    if t == Type::INVALID || t == Type::UNIT {
                        return Err(format!("{what}: type has no GType"));
                    }
                    if t.is_a(Type::ENUM) {
                        Ty::Enum(t)
                    } else if t.is_a(Type::FLAGS) {
                        Ty::Flags(t)
                    } else if t.is_a(Type::OBJECT) || t.is_a(Type::INTERFACE) {
                        Ty::Object(t)
                    } else {
                        return Err(format!("{what}: fundamental type {} is not supported", t.name()));
                    }
                }
                other => return Err(format!("{what}: unsupported interface kind {other}")),
            }
        }
        t => return Err(format!("{what}: unsupported type tag {t}")),
    })
}

fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

impl Gi {
    pub fn new() -> Result<Gi, String> {
        unsafe {
            let repo = gi_repository_new();
            for (i, (ns, ver)) in NAMESPACES.iter().enumerate() {
                let mut err = std::ptr::null_mut();
                let ns_c = CString::new(*ns).unwrap();
                let ver_c = CString::new(*ver).unwrap();
                if gi_repository_require(repo, ns_c.as_ptr(), ver_c.as_ptr(), 0, &mut err).is_null() {
                    let msg = if err.is_null() { "unknown error".to_string() } else { CStr::from_ptr((*err).message).to_string_lossy().into_owned() };
                    if !err.is_null() {
                        glib::ffi::g_error_free(err);
                    }
                    if i == 0 {
                        return Err(format!("cannot load the {ns}-{ver} typelib: {msg}"));
                    }
                }
            }
            Ok(Gi { repo, cache: RefCell::new(HashMap::new()), by_type: RefCell::new(HashMap::new()), methods: RefCell::new(Vec::new()), cb_types: RefCell::new(Vec::new()) })
        }
    }

    /// `Gio.ListStore` for a GType the typelib describes, else its GType name.
    pub fn qualified_name(&self, t: Type) -> String {
        if let Some(info) = Owned::new(unsafe { gi_repository_find_by_gtype(self.repo, t.into_glib()) }) {
            let (ns, name) = unsafe { (gi_base_info_get_namespace(info.0), gi_base_info_get_name(info.0)) };
            if !ns.is_null() && !name.is_null() {
                return format!("{}.{}", unsafe { CStr::from_ptr(ns) }.to_string_lossy(), unsafe { CStr::from_ptr(name) }.to_string_lossy());
            }
        }
        t.name().to_string()
    }

    /// A GType from a qualified native name (`Gio.ListStore`) or a GType name (`GListStore`).
    pub fn type_by_name(&self, name: &str) -> Option<Type> {
        if let Some((ns, n)) = name.split_once('.') {
            let (ns, n) = (CString::new(ns).ok()?, CString::new(n).ok()?);
            if let Some(info) = Owned::new(unsafe { gi_repository_find_by_name(self.repo, ns.as_ptr(), n.as_ptr()) }) {
                let t: Type = unsafe { from_glib(gi_registered_type_info_get_g_type(info.0)) };
                if t != Type::INVALID && t != Type::UNIT {
                    return Some(t);
                }
            }
            return None;
        }
        let t = Type::from_name(name)?;
        (t != Type::INVALID).then_some(t)
    }

    /// Finds `name` (as written, or camelCase converted to snake_case) on the kind's type, its
    /// interfaces and its ancestors. Returns an index into the method list, building the plan the
    /// first time. `None` when the typelib has no such function.
    pub fn resolve(&self, kind: u32, start: Type, name: &str) -> Option<u32> {
        if let Some(i) = self.cache.borrow().get(&(kind, name.to_string())) {
            return Some(*i);
        }
        let idx = self.find(start, name)?;
        self.cache.borrow_mut().insert((kind, name.to_string()), idx);
        Some(idx)
    }

    /// The same lookup by receiver type, for a method id that came from a sibling kind.
    pub fn resolve_on(&self, receiver: Type, name: &str) -> Option<Rc<DynMethod>> {
        if let Some(i) = self.by_type.borrow().get(&(receiver, name.to_string())) {
            return i.and_then(|i| self.method(i));
        }
        let idx = self.find(receiver, name);
        self.by_type.borrow_mut().insert((receiver, name.to_string()), idx);
        idx.and_then(|i| self.method(i))
    }

    fn find(&self, start: Type, name: &str) -> Option<u32> {
        let snake = camel_to_snake(name);
        let candidates: Vec<&str> = if snake == name { vec![name] } else { vec![name, &snake] };
        let mut t = Some(start);
        while let Some(ty) = t {
            // The type itself, then the interfaces it implements: a runtime class the typelib does
            // not know (GLocalFile) still answers the methods of its interfaces (GFile).
            let mut here = vec![ty];
            if ty.is_a(Type::OBJECT) {
                here.extend(ty.interfaces());
            }
            for cur in here {
                let Some(info) = Owned::new(unsafe { gi_repository_find_by_gtype(self.repo, cur.into_glib()) }) else { continue };
                for cand in &candidates {
                    let c = CString::new(*cand).ok()?;
                    if let Some(m) = self.find_in(&info, &c) {
                        let dm = Rc::new(self.build(m, cand));
                        let mut ms = self.methods.borrow_mut();
                        ms.push(dm);
                        return Some(ms.len() as u32 - 1);
                    }
                }
            }
            t = ty.parent();
        }
        None
    }

    fn find_in(&self, info: &Owned, name: &CStr) -> Option<Owned> {
        let tname = unsafe { CStr::from_ptr(gobject_ffi::g_type_name_from_instance(info.0 as *mut gobject_ffi::GTypeInstance)) }.to_string_lossy().into_owned();
        unsafe {
            match tname.as_str() {
                "GIObjectInfo" => {
                    if let Some(m) = Owned::new(gi_object_info_find_method(info.0, name.as_ptr())) {
                        return Some(m);
                    }
                    for i in 0..gi_object_info_get_n_interfaces(info.0) {
                        let Some(iface) = Owned::new(gi_object_info_get_interface(info.0, i)) else { continue };
                        if let Some(m) = Owned::new(gi_interface_info_find_method(iface.0, name.as_ptr())) {
                            return Some(m);
                        }
                    }
                    None
                }
                "GIInterfaceInfo" => Owned::new(gi_interface_info_find_method(info.0, name.as_ptr())),
                "GIStructInfo" => Owned::new(gi_struct_info_find_method(info.0, name.as_ptr())),
                _ => None,
            }
        }
    }

    fn build(&self, info: Owned, name: &str) -> DynMethod {
        let container = unsafe { gi_base_info_get_container(info.0) };
        let declaring = unsafe { if container.is_null() { Type::INVALID } else { from_glib(gi_registered_type_info_get_g_type(container)) } };
        let mut plan = plan_of(&info, name);
        let mut finish = None;
        // `foo_async` / `foo` with a GAsyncReadyCallback pairs with `foo_finish` of the same container
        // (the typelib only names the pair from GLib 2.84 on, so the GIR naming convention decides).
        if let Ok(p) = &plan {
            if p.async_args.is_some() {
                let native = unsafe { CStr::from_ptr(gi_base_info_get_name(info.0)) }.to_string_lossy().into_owned();
                let stem = native.strip_suffix("_async").unwrap_or(&native);
                let fname = format!("{stem}_finish");
                let found = if container.is_null() {
                    None
                } else {
                    unsafe { gi_base_info_ref(container) };
                    let c = Owned(container);
                    CString::new(fname.as_str()).ok().and_then(|n| self.find_in(&c, &n))
                };
                match found {
                    Some(f) => {
                        let fm = self.build(f, &fname);
                        match &fm.plan {
                            Ok(fp) if fp.async_result.is_some() => finish = Some(Rc::new(fm)),
                            Ok(_) => plan = Err(format!("{name}: {fname} takes no GAsyncResult")),
                            Err(e) => plan = Err(format!("{name}: its finish function is not supported: {e}")),
                        }
                    }
                    None => plan = Err(format!("{name}: async function without a {fname} function")),
                }
            }
        }
        DynMethod { name: name.to_string(), info, declaring, plan, finish }
    }

    /// The type id of a callback signature as the host sees it (registered on first use).
    fn cb_type_id(&self, table: &mut TypeTable, spec: &CbSpec) -> u32 {
        let scope = host_scope(spec.scope);
        if let Some(i) = self.cb_types.borrow().iter().position(|(n, s, _)| *n == spec.name && *s == scope) {
            return CB_TYPE_BASE + i as u32;
        }
        let fields = spec.sig.args.iter().flatten().map(|(n, s)| slot_desc(table, &s.ty, s.nullable).named(n)).collect();
        let desc = TypeDesc { name: spec.name.clone(), kind: SVL_CALLBACK, scope, members: vec![], fields, ret: slot_desc(table, &spec.sig.ret.ty, spec.sig.ret.nullable) };
        let mut v = self.cb_types.borrow_mut();
        v.push((spec.name.clone(), scope, desc));
        CB_TYPE_BASE + v.len() as u32 - 1
    }

    /// The descriptor of a callback type id from [`Gi::cb_type_id`].
    pub fn cb_type_desc(&self, id: u32) -> Option<TypeDesc> {
        let i = id.checked_sub(CB_TYPE_BASE)? as usize;
        self.cb_types.borrow().get(i).map(|(_, _, d)| d.clone())
    }

    /// The error enum registered for a `GError` domain (`G_IO_ERROR` -> `GIOErrorEnum`).
    pub fn error_enum(&self, domain: glib::Quark) -> Option<Type> {
        let info = Owned::new(unsafe { gi_repository_find_by_error_domain(self.repo, domain.into_glib()) })?;
        let t: Type = unsafe { from_glib(gi_registered_type_info_get_g_type(info.0)) };
        (t.is_a(Type::ENUM)).then_some(t)
    }

    pub fn method(&self, idx: u32) -> Option<Rc<DynMethod>> {
        self.methods.borrow().get(idx as usize).cloned()
    }
}

/// The `GICallbackInfo` of an interface-typed argument, if it is a callback type.
fn callback_iface(ti: Info) -> Option<Owned> {
    if unsafe { gi_type_info_get_tag(ti) } != TAG_INTERFACE {
        return None;
    }
    let iface = Owned::new(unsafe { gi_type_info_get_interface(ti) })?;
    let tname = unsafe { CStr::from_ptr(gobject_ffi::g_type_name_from_instance(iface.0 as *mut gobject_ffi::GTypeInstance)) };
    (tname.to_bytes() == b"GICallbackInfo").then_some(iface)
}

fn qualified(info: Info) -> String {
    unsafe {
        let ns = gi_base_info_get_namespace(info);
        let n = gi_base_info_get_name(info);
        let ns = if ns.is_null() { String::new() } else { CStr::from_ptr(ns).to_string_lossy().into_owned() };
        let n = if n.is_null() { String::new() } else { CStr::from_ptr(n).to_string_lossy().into_owned() };
        format!("{ns}.{n}")
    }
}

/// Functions whose callback compares or receives the *items of a list model* through untyped
/// pointers (`GCompareDataFunc` over `gconstpointer`): those pointers are `GObject`s, so they cross
/// as objects (N3). Keyed by C symbol; `tools/metadata/gtk/types_facts.py` has the same list.
pub(crate) const OBJECT_POINTER_CALLBACKS: &[&str] = &["g_list_store_sort", "g_list_store_insert_sorted", "gtk_custom_sorter_new", "gtk_custom_sorter_set_sort_func"];

/// The signature of a callback type. Supported: arguments the invoker reads back (numbers, strings,
/// enums, flags, objects, boxed values, GStrv, lists) plus one trailing user-data pointer, and any
/// return value the invoker can pass in. Out arguments, arrays with a length, `GError` and other
/// untyped pointers are refused, except that `objects` makes untyped pointers `GObject`s.
fn plan_callback(cb: &Owned, what: &str, objects: bool) -> Result<CbSig, String> {
    unsafe {
        if gi_callable_info_can_throw_gerror(cb.0) != 0 {
            return Err(format!("{what}: callbacks that throw are not supported"));
        }
        let n = gi_callable_info_get_n_args(cb.0);
        let mut args = Vec::new();
        for i in 0..n {
            let a = Owned::new(gi_callable_info_get_arg(cb.0, i)).ok_or("missing argument info")?;
            let aname = CStr::from_ptr(gi_base_info_get_name(a.0)).to_string_lossy().into_owned();
            let w = format!("{what} argument '{aname}'");
            if gi_arg_info_get_direction(a.0) != DIR_IN {
                return Err(format!("{w}: out arguments of callbacks are not supported"));
            }
            let ti = Owned::new(gi_arg_info_get_type_info(a.0)).ok_or("missing type info")?;
            if callback_iface(ti.0).is_some() {
                return Err(format!("{w}: callbacks taking callbacks are not supported"));
            }
            let mut ty = type_info_ty(ti.0, &w)?;
            if ty == Ty::Void {
                if i + 1 == n {
                    args.push(None);
                    continue;
                }
                if !objects {
                    return Err(format!("{w}: untyped pointer arguments are not supported"));
                }
                ty = Ty::Object(glib::Object::static_type());
            }
            if matches!(ty, Ty::Array(_, ArrLen::Arg(_))) {
                return Err(format!("{w}: arrays with a length argument are not supported in callbacks"));
            }
            let transfer = gi_arg_info_get_ownership_transfer(a.0);
            args.push(Some((
                aname,
                Slot { ty, nullable: gi_arg_info_may_be_null(a.0) != 0, full: transfer == TRANSFER_EVERYTHING, container: transfer == TRANSFER_CONTAINER, dir: Dir::In, caller_alloc: None },
            )));
        }
        let rt = Owned::new(gi_callable_info_get_return_type(cb.0)).ok_or("missing return type")?;
        let rty = type_info_ty(rt.0, &format!("{what}: return value"))?;
        if matches!(rty, Ty::List(..) | Ty::Array(..)) {
            return Err(format!("{what}: callbacks returning lists or arrays are not supported"));
        }
        let transfer = gi_callable_info_get_caller_owns(cb.0);
        let ret = Slot { ty: rty, nullable: gi_callable_info_may_return_null(cb.0) != 0, full: transfer == TRANSFER_EVERYTHING, container: false, dir: Dir::Out, caller_alloc: None };
        Ok(CbSig { args, ret })
    }
}

fn plan_of(info: &Owned, name: &str) -> Result<Plan, String> {
    unsafe {
        let flags = gi_function_info_get_flags(info.0);
        let is_method = gi_callable_info_is_method(info.0) != 0;
        let n = gi_callable_info_get_n_args(info.0);
        let mut args: Vec<PlanArg> = Vec::new();
        let mut callbacks: Vec<CbSpec> = Vec::new();
        let mut async_cb: Option<(usize, Option<usize>)> = None;
        // User-data arguments that name their callback (older typelibs put `closure` on the user data).
        let mut back_refs: Vec<(usize, usize)> = Vec::new();
        let (mut n_in, mut n_out) = (0usize, 0usize);
        for i in 0..n {
            let a = Owned::new(gi_callable_info_get_arg(info.0, i)).ok_or("missing argument info")?;
            let aname = CStr::from_ptr(gi_base_info_get_name(a.0)).to_string_lossy().into_owned();
            let what = format!("{name}: argument '{aname}'");
            let dir = match gi_arg_info_get_direction(a.0) {
                DIR_IN => Dir::In,
                DIR_OUT => Dir::Out,
                DIR_INOUT => Dir::InOut,
                d => return Err(format!("{what}: unknown direction {d}")),
            };
            let ti = Owned::new(gi_arg_info_get_type_info(a.0)).ok_or("missing type info")?;
            let mut closure: c_uint = 0;
            let has_closure = gi_arg_info_get_closure_index(a.0, &mut closure) != 0;
            let mut destroy: c_uint = 0;
            let has_destroy = gi_arg_info_get_destroy_index(a.0, &mut destroy) != 0;
            let i = i as usize;
            let transfer = gi_arg_info_get_ownership_transfer(a.0);
            let nullable = gi_arg_info_may_be_null(a.0) != 0;
            if let Some(cb) = callback_iface(ti.0) {
                if dir != Dir::In {
                    return Err(format!("{what}: out callbacks are not supported"));
                }
                let cbname = qualified(cb.0);
                let ty = match cbname.as_str() {
                    // A destroy notify belongs to the callback that names it (checked below).
                    "GLib.DestroyNotify" => Ty::Internal,
                    "Gio.AsyncReadyCallback" => {
                        async_cb = Some((i, has_closure.then_some(closure as usize)));
                        Ty::Internal
                    }
                    _ => {
                        let sym = gi_function_info_get_symbol(info.0);
                        let objects = !sym.is_null() && OBJECT_POINTER_CALLBACKS.contains(&&*CStr::from_ptr(sym).to_string_lossy());
                        let sig = plan_callback(&cb, &format!("{what} ({cbname})"), objects)?;
                        callbacks.push(CbSpec {
                            info: Rc::new(cb),
                            name: cbname,
                            sig: Rc::new(sig),
                            scope: gi_arg_info_get_scope(a.0),
                            user_data: has_closure.then_some(closure as usize),
                            destroy: has_destroy.then_some(destroy as usize),
                        });
                        Ty::Callback(callbacks.len() - 1)
                    }
                };
                let slot = Slot { ty, nullable, full: false, container: false, dir, caller_alloc: None };
                n_in += 1;
                args.push(PlanArg { name: aname, slot, role: Role::Js, in_pos: Some(n_in - 1), out_pos: None });
                continue;
            }
            let caller_alloc = dir == Dir::Out && gi_arg_info_is_caller_allocates(a.0) != 0;
            let ty = type_info_ty_ref(ti.0, &what, caller_alloc)?;
            if ty == Ty::Void && has_closure && dir == Dir::In {
                back_refs.push((i, closure as usize));
            }
            let mut slot = Slot { ty, nullable, full: transfer == TRANSFER_EVERYTHING, container: transfer == TRANSFER_CONTAINER, dir, caller_alloc: None };
            if slot.ty == Ty::Void {
                slot.ty = Ty::Internal;
            }
            if caller_alloc {
                let Ty::Boxed(_) = slot.ty else {
                    return Err(format!("{what}: caller-allocated out values other than boxed structs are not supported"));
                };
                let iface = Owned::new(gi_type_info_get_interface(ti.0)).ok_or("missing struct info")?;
                slot.caller_alloc = Some(gi_struct_info_get_size(iface.0).max(1));
            }
            if dir == Dir::InOut && matches!(slot.ty, Ty::List(..) | Ty::Array(..)) {
                return Err(format!("{what}: inout lists and arrays are not supported"));
            }
            let in_pos = (dir != Dir::Out).then(|| {
                n_in += 1;
                n_in - 1
            });
            let out_pos = (dir != Dir::In).then(|| {
                n_out += 1;
                n_out - 1
            });
            args.push(PlanArg { name: aname, slot, role: Role::Js, in_pos, out_pos });
        }
        // Callback companions: user data and destroy notifies are filled in by the invoker.
        for (k, cb) in callbacks.iter().enumerate() {
            let owner = args.iter().position(|a| a.slot.ty == Ty::Callback(k)).unwrap();
            if let Some(u) = cb.user_data.filter(|u| *u < args.len() && *u != owner && args[*u].slot.ty == Ty::Internal) {
                args[u].role = Role::UserData(owner);
            }
            if let Some(d) = cb.destroy.filter(|d| *d < args.len() && args[*d].slot.ty == Ty::Internal) {
                args[d].role = Role::Destroy(owner);
            }
        }
        for (u, c) in back_refs {
            if c < args.len() && matches!(args[c].slot.ty, Ty::Callback(_)) && args[u].role == Role::Js {
                args[u].role = Role::UserData(c);
            }
        }
        let mut async_args = None;
        if let Some((cb, ud)) = async_cb {
            args[cb].role = Role::Hidden;
            let ud = ud
                .filter(|u| *u < args.len() && *u != cb)
                .or_else(|| args.iter().position(|a| a.slot.ty == Ty::Internal && a.role == Role::Js && a.slot.dir == Dir::In));
            if let Some(u) = ud {
                args[u].role = Role::Hidden;
            }
            let cancellable = args.iter().position(|a| a.slot.dir == Dir::In && matches!(a.slot.ty, Ty::Object(t) if t == gtk::gio::Cancellable::static_type()));
            if let Some(c) = cancellable {
                args[c].role = Role::Hidden;
            }
            async_args = Some(AsyncArgs { callback: cb, user_data: ud, cancellable });
        }
        // The GAsyncResult of a finish function comes from the completion, never from JS.
        let mut async_result = None;
        if name.ends_with("_finish") {
            async_result = args.iter().position(|a| a.slot.dir == Dir::In && a.role == Role::Js && matches!(a.slot.ty, Ty::Object(t) if t == gtk::gio::AsyncResult::static_type()));
            if let Some(r) = async_result {
                args[r].role = Role::Hidden;
            }
        }
        for a in &args {
            if a.role == Role::Js && a.slot.ty == Ty::Internal {
                return Err(format!("{name}: argument '{}': untyped (void) parameters and unpaired destroy notifies are not supported", a.name));
            }
        }
        // Length arguments of arrays are filled in by the array and hidden from JS.
        let mut marks: Vec<(usize, usize)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if let Ty::Array(_, ArrLen::Arg(l)) = &a.slot.ty {
                if *l >= args.len() {
                    return Err(format!("{name}: argument '{}': length argument out of range", a.name));
                }
                marks.push((*l, i));
            }
        }
        for (l, owner) in marks {
            args[l].role = Role::Len(owner);
        }
        let rt = Owned::new(gi_callable_info_get_return_type(info.0)).ok_or("missing return type")?;
        let rty = type_info_ty(rt.0, &format!("{name}: return value"))?;
        if let Ty::Array(_, ArrLen::Arg(l)) = &rty {
            if *l >= args.len() {
                return Err(format!("{name}: return value: length argument out of range"));
            }
            args[*l].role = Role::Len(usize::MAX);
        }
        let transfer = gi_callable_info_get_caller_owns(info.0);
        let ret = Slot {
            ty: if gi_callable_info_skip_return(info.0) != 0 { Ty::Void } else { rty },
            nullable: gi_callable_info_may_return_null(info.0) != 0,
            full: transfer == TRANSFER_EVERYTHING,
            container: transfer == TRANSFER_CONTAINER,
            dir: Dir::Out,
            caller_alloc: None,
        };
        if async_args.is_some() && ret.ty != Ty::Void {
            return Err(format!("{name}: async functions with a return value are not supported"));
        }
        // GIR marks some progress callbacks of async functions `call` although they run after the
        // call returns (g_file_load_partial_contents_async): a call-scope closure would be freed first.
        if async_args.is_some() && callbacks.iter().any(|c| c.scope == SCOPE_CALL) {
            return Err(format!("{name}: call-scope callbacks of async functions are not supported"));
        }
        if async_args.is_some() && args.iter().any(|a| a.role == Role::Js && a.slot.dir != Dir::In) {
            return Err(format!("{name}: async functions with out parameters are not supported"));
        }
        Ok(Plan {
            args,
            ret,
            throws: gi_callable_info_can_throw_gerror(info.0) != 0,
            is_method,
            is_ctor: flags & FN_IS_CONSTRUCTOR != 0,
            n_in,
            n_out,
            callbacks,
            async_args,
            async_result,
        })
    }
}

fn int_arg(v: &svl_value, what: &str) -> Result<i64, String> {
    match v.tag {
        SVL_I64 => Ok(unsafe { v.u.i }),
        SVL_BOOL => Ok(unsafe { v.u.b } as i64),
        SVL_F64 => {
            let f = unsafe { v.u.f };
            if f.fract() == 0.0 && f.abs() < 9.0e15 { Ok(f as i64) } else { Err(format!("{what}: {f} is not an integer")) }
        }
        t => Err(format!("{what}: expected an integer, got tag {t}")),
    }
}

fn ranged<T: TryFrom<i64>>(x: i64, what: &str) -> Result<T, String> {
    T::try_from(x).map_err(|_| format!("{what}: {x} is out of range"))
}

/// What `call` holds while the native function runs: string copies and objects whose references
/// must be released afterwards.
#[derive(Default)]
pub(crate) struct Keep {
    strings: Vec<CString>,
    objects: Vec<glib::Object>,
    values: Vec<glib::Value>,
    /// NULL-terminated `char *` arrays handed to the callee.
    strvs: Vec<Vec<*const c_char>>,
    /// `g_malloc`ed blocks (arrays, caller-allocated structs) we free after the call.
    mallocs: Vec<*mut c_void>,
    /// GList/GSList containers we free after the call (`false`: GList).
    lists: Vec<(*mut c_void, bool)>,
    /// Storage of out and inout values; the boxes keep their addresses.
    slots: Vec<Box<GIArgument>>,
    /// The length of the array just converted.
    last_len: usize,
    /// Native closures made for callback arguments, with their GIR scope.
    closures: Vec<(*mut crate::closures::NativeClosure, c_int)>,
    /// The native function ran (it may have kept the closures its scope allows it to keep).
    committed: bool,
}

impl Drop for Keep {
    fn drop(&mut self) {
        // A `call`-scope closure ends with the call; any closure the callee never saw ends now.
        for (c, scope) in self.closures.drain(..) {
            if scope == SCOPE_CALL || !self.committed {
                unsafe { crate::closures::NativeClosure::free(c) };
            }
        }
        unsafe {
            for p in self.mallocs.drain(..) {
                glib::ffi::g_free(p);
            }
            for (p, slist) in self.lists.drain(..) {
                if slist {
                    glib::ffi::g_slist_free(p as *mut glib::ffi::GSList);
                } else {
                    glib::ffi::g_list_free(p as *mut glib::ffi::GList);
                }
            }
        }
    }
}

fn elem_size(t: &Ty) -> Option<usize> {
    Some(match t {
        Ty::Bool => std::mem::size_of::<c_int>(),
        Ty::I8 | Ty::U8 => 1,
        Ty::I16 | Ty::U16 => 2,
        Ty::I32 | Ty::U32 | Ty::F32 | Ty::Enum(_) | Ty::Flags(_) => 4,
        Ty::I64 | Ty::U64 | Ty::F64 => 8,
        Ty::Str | Ty::Object(_) | Ty::Boxed(_) => std::mem::size_of::<*mut c_void>(),
        _ => return None,
    })
}

/// What the invoker needs from the backend for callbacks and async completions (N1). The backend's
/// `Inner` implements it; closures and pending operations hold it weakly.
pub trait Host: Env {
    /// Runs the JS function behind a host callback id.
    fn invoke_callback(&self, id: u64, args: &[Value]) -> Result<Value, String>;
    /// The native side dropped a callback (idempotent).
    fn release_callback(&self, id: u64);
    /// A converted native value as an ABI value (objects get handles owned by `owner`).
    fn ret_to_value(&self, r: Ret, owner: u64) -> Value;
    /// A `GError` as an `ERROR` value (domain, error-enum member name, message).
    fn error_value(&self, e: &glib::Error) -> Value;
    /// The async operation completing `done` finished natively. False when it was already settled
    /// (its owner was destroyed), so the completion must not be delivered.
    fn async_finished(&self, done: u64) -> bool;
    fn gi(&self) -> Option<Rc<Gi>>;
    fn as_env(&self) -> &dyn Env;
}

/// What a call needs besides its arguments.
pub(crate) struct CallCx<'a> {
    /// GIArguments the invoker supplies, by native argument index (async machinery).
    pub(crate) presets: &'a [(usize, GIArgument)],
    /// The backend, for callback arguments.
    pub(crate) host: Option<&'a std::rc::Weak<dyn Host>>,
    /// The handle the call is made on (owner of minted handles), 0 for statics.
    pub(crate) owner: u64,
}

impl CallCx<'_> {
    pub(crate) fn plain() -> CallCx<'static> {
        CallCx { presets: &[], host: None, owner: 0 }
    }
}

impl DynMethod {
    pub fn is_static(&self) -> bool {
        self.plan.as_ref().is_ok_and(|p| !p.is_method)
    }

    pub fn has_outs(&self) -> bool {
        self.plan.as_ref().is_ok_and(|p| p.has_outs())
    }

    /// Why the invoker refuses this function, if it does.
    pub fn plan_error(&self) -> Option<&str> {
        self.plan.as_ref().err().map(|e| e.as_str())
    }

    /// An `_async` function folded with its `_finish`: JS gets a Promise (`SVL_M_ASYNC`).
    pub fn is_async(&self) -> bool {
        self.plan.as_ref().is_ok_and(|p| p.async_args.is_some())
    }

    pub fn declaring(&self) -> Type {
        self.declaring
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The member descriptor for the host: typed slots so that enums travel as names, flags as
    /// name sets, records and objects as their value types. `None` when the plan has an error.
    pub fn describe(&self, table: &mut TypeTable, gi: &Gi) -> Option<MemberDesc> {
        let plan = self.plan.as_ref().ok()?;
        let mut params = Vec::new();
        for a in &plan.args {
            if a.role != Role::Js {
                continue;
            }
            let mut d = match &a.slot.ty {
                Ty::Callback(k) => {
                    let mut d = SlotDesc::new(SVL_CALLBACK).typed(gi.cb_type_id(table, &plan.callbacks[*k]));
                    if a.slot.nullable {
                        d.flags |= SVL_D_NULLABLE;
                    }
                    d
                }
                ty => slot_desc(table, ty, a.slot.nullable),
            }
            .named(&a.name);
            match a.slot.dir {
                Dir::Out => d.flags |= SVL_D_OUT,
                Dir::InOut => d.flags |= SVL_D_INOUT,
                Dir::In => {}
            }
            params.push(d);
        }
        let mut flags = if plan.throws { SVL_M_THROWS } else { 0 };
        if !plan.is_method {
            flags |= SVL_M_STATIC;
            if plan.is_ctor || matches!(plan.ret.ty, Ty::Object(_) | Ty::Boxed(_)) && plan.ret.full {
                flags |= SVL_M_CTOR;
            }
        }
        if plan.async_args.is_some() {
            // The host appends its one-shot completion; the finish function's GError rejects.
            params.push(SlotDesc::new(SVL_CALLBACK).typed(T_ASYNC_DONE).named("done"));
            flags |= SVL_M_ASYNC;
            if self.finish.as_ref().and_then(|f| f.plan.as_ref().ok()).is_some_and(|f| f.throws) {
                flags |= SVL_M_THROWS;
            }
        }
        Some(MemberDesc { name: self.name.clone(), params, ret: slot_desc(table, &plan.ret.ty, plan.ret.nullable), flags })
    }

    /// Calls the function with an object receiver.
    pub fn call(&self, obj: &glib::Object, args: &[svl_value], env: &dyn Env, gi: &Gi, cx: &CallCx) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if !plan.is_method {
            return Err(format!("{} is a static function or constructor, not a method", self.name).into());
        }
        if !obj.type_().is_a(self.declaring) {
            return Err(format!("{} is not a {}", obj.type_().name(), self.declaring.name()).into());
        }
        self.invoke(plan, Some(obj.as_ptr() as *mut c_void), args, env, gi, cx)
    }

    /// Calls the function with a boxed receiver (a VALUE handle).
    pub fn call_boxed(&self, value: &glib::Value, args: &[svl_value], env: &dyn Env, gi: &Gi, cx: &CallCx) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if !plan.is_method {
            return Err(format!("{} is a static function or constructor, not a method", self.name).into());
        }
        if value.type_() != self.declaring {
            return Err(format!("{} is not a {}", value.type_().name(), self.declaring.name()).into());
        }
        let p = unsafe { gobject_ffi::g_value_get_boxed(value.to_glib_none().0) };
        if p.is_null() {
            return Err("the value is empty".into());
        }
        self.invoke(plan, Some(p as *mut c_void), args, env, gi, cx)
    }

    /// Calls a static function or constructor.
    pub fn call_static(&self, args: &[svl_value], env: &dyn Env, gi: &Gi, cx: &CallCx) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if plan.is_method {
            return Err(format!("{} is an instance method; call it on an object", self.name).into());
        }
        self.invoke(plan, None, args, env, gi, cx)
    }

    /// Starts a folded async operation: `args` are the JS-visible arguments (without the host's
    /// completion), `done` the completion callback id. The `GAsyncReadyCallback` calls the finish
    /// function on the main loop and completes `done` exactly once (see `closures.rs`).
    pub fn start_async(
        self: &Rc<Self>,
        recv: Option<&glib::Object>,
        args: &[svl_value],
        done: u64,
        cancellable: &gtk::gio::Cancellable,
        host: std::rc::Weak<dyn Host>,
        gi: &Gi,
        owner: u64,
    ) -> Result<(), GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        let aa = plan.async_args.ok_or_else(|| format!("{} is not an async function", self.name))?;
        let finish = self.finish.clone().ok_or_else(|| format!("{}: no finish function", self.name))?;
        if plan.is_method != recv.is_some() {
            return Err(format!("{}: wrong receiver for an async call", self.name).into());
        }
        if let Some(o) = recv {
            if !o.type_().is_a(self.declaring) {
                return Err(format!("{} is not a {}", o.type_().name(), self.declaring.name()).into());
            }
        }
        let op = crate::closures::AsyncOp::new(finish, recv.cloned(), done, host.clone(), owner);
        let mut presets: Vec<(usize, GIArgument)> = Vec::new();
        let mut cb = ZERO;
        cb.v_pointer = crate::closures::async_ready as *mut c_void;
        presets.push((aa.callback, cb));
        if let Some(u) = aa.user_data {
            let mut a = ZERO;
            a.v_pointer = op as *mut c_void;
            presets.push((u, a));
        }
        if let Some(c) = aa.cancellable {
            let mut a = ZERO;
            a.v_pointer = cancellable.as_ptr() as *mut c_void;
            presets.push((c, a));
        }
        let Some(h) = host.upgrade() else {
            unsafe { crate::closures::AsyncOp::discard(op) };
            return Err("the backend is gone".into());
        };
        let cx = CallCx { presets: &presets, host: Some(&host), owner };
        let r = self.invoke(plan, recv.map(|o| o.as_ptr() as *mut c_void), args, h.as_env(), gi, &cx);
        if r.is_err() {
            // An async start never throws: an error here means the native function did not run, so the
            // ready callback will never come.
            unsafe { crate::closures::AsyncOp::discard(op) };
        }
        r.map(|_| ())
    }

    /// Runs a finish function on the `GAsyncResult` of a completion.
    pub(crate) fn finish_with(&self, recv: Option<&glib::Object>, res: *mut c_void, env: &dyn Env, gi: &Gi) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        let r = plan.async_result.ok_or("not a finish function")?;
        let mut a = ZERO;
        a.v_pointer = res;
        let presets = [(r, a)];
        let cx = CallCx { presets: &presets, host: None, owner: 0 };
        let recv = if plan.is_method { Some(recv.ok_or("finish function without a receiver")?.as_ptr() as *mut c_void) } else { None };
        self.invoke(plan, recv, &[], env, gi, &cx)
    }

    /// The names of the out parameters (in order) and whether the function returns a value: the
    /// shape of a folded async result.
    pub(crate) fn result_shape(&self) -> (bool, Vec<String>) {
        match &self.plan {
            Ok(p) => (p.ret.ty != Ty::Void, p.args.iter().filter(|a| a.role == Role::Js && a.slot.dir != Dir::In).map(|a| a.name.clone()).collect()),
            Err(_) => (false, vec![]),
        }
    }

    fn invoke(&self, plan: &Plan, recv: Option<*mut c_void>, args: &[svl_value], env: &dyn Env, gi: &Gi, cx: &CallCx) -> Result<Called, GiErr> {
        let visible: Vec<usize> = plan.args.iter().enumerate().filter(|(_, a)| a.role == Role::Js && a.slot.dir != Dir::Out).map(|(i, _)| i).collect();
        if args.len() != visible.len() {
            let want: Vec<&str> = visible.iter().map(|i| plan.args[*i].name.as_str()).collect();
            return Err(format!("{} takes {} argument(s) ({}), got {}", self.name, visible.len(), want.join(", "), args.len()).into());
        }
        let mut keep = Keep::default();
        let base = recv.is_some() as usize;
        let mut ins: Vec<GIArgument> = vec![ZERO; base + plan.n_in];
        let mut outs: Vec<GIArgument> = vec![ZERO; plan.n_out];
        if let Some(p) = recv {
            ins[0].v_pointer = p;
        }
        // Length values the in-arrays produced, by native index of the length argument.
        let mut lens: HashMap<usize, usize> = HashMap::new();
        // User data and destroy notifies the callback arguments produced, by native index.
        let mut fills: Vec<(usize, GIArgument)> = Vec::new();
        let mut next_js = 0;
        // Pointers to the out storage, by out position.
        let mut out_ptr: Vec<*mut GIArgument> = vec![std::ptr::null_mut(); plan.n_out];
        for (i, a) in plan.args.iter().enumerate() {
            if let Some((_, p)) = cx.presets.iter().find(|(k, _)| *k == i) {
                ins[base + a.in_pos.ok_or("preset out argument")?] = *p;
                continue;
            }
            match (a.role, a.slot.dir) {
                (Role::Len(_), Dir::In) | (Role::UserData(_), _) | (Role::Destroy(_), _) | (Role::Hidden, _) => {}
                (_, Dir::Out) => {
                    if let Some(size) = a.slot.caller_alloc {
                        let p = unsafe { glib::ffi::g_malloc0(size) };
                        keep.mallocs.push(p);
                        outs[a.out_pos.unwrap()].v_pointer = p;
                    } else {
                        let mut b = Box::new(ZERO);
                        let p: *mut GIArgument = &mut *b;
                        keep.slots.push(b);
                        out_ptr[a.out_pos.unwrap()] = p;
                        outs[a.out_pos.unwrap()].v_pointer = p as *mut c_void;
                    }
                }
                (_, Dir::In) => {
                    let what = format!("{} argument '{}'", self.name, a.name);
                    let v = &args[next_js];
                    next_js += 1;
                    if let Ty::Callback(k) = a.slot.ty {
                        let spec = &plan.callbacks[k];
                        let (native, data) = if v.tag == SVL_NULL {
                            if !a.slot.nullable {
                                return Err(format!("{what}: null is not allowed").into());
                            }
                            (std::ptr::null_mut(), std::ptr::null_mut())
                        } else if v.tag != SVL_CALLBACK {
                            return Err(format!("{what}: expected a function").into());
                        } else {
                            let host = cx.host.ok_or_else(|| format!("{what}: callbacks need the host"))?;
                            let id = unsafe { v.u.u };
                            let c = crate::closures::NativeClosure::create(spec, id, host.clone(), cx.owner).map_err(|e| format!("{what}: {e}"))?;
                            keep.closures.push((c, spec.scope));
                            (unsafe { (*c).native }, c as *mut c_void)
                        };
                        let mut na = ZERO;
                        na.v_pointer = native;
                        ins[base + a.in_pos.unwrap()] = na;
                        for (j, b) in plan.args.iter().enumerate() {
                            let mut x = ZERO;
                            match b.role {
                                Role::UserData(o) if o == i => x.v_pointer = data,
                                Role::Destroy(o) if o == i && !data.is_null() => {
                                    x.v_pointer = crate::closures::closure_destroy as *mut c_void;
                                }
                                _ => continue,
                            }
                            fills.push((j, x));
                        }
                    } else {
                        let arg = to_arg(&a.slot, v, &what, &mut keep, env, gi)?;
                        ins[base + a.in_pos.unwrap()] = arg;
                        if let Ty::Array(_, ArrLen::Arg(l)) = &a.slot.ty {
                            lens.insert(*l, keep.last_len);
                        }
                    }
                }
                (_, Dir::InOut) => {
                    let what = format!("{} argument '{}'", self.name, a.name);
                    let arg = to_arg(&a.slot, &args[next_js], &what, &mut keep, env, gi)?;
                    next_js += 1;
                    let mut b = Box::new(arg);
                    let p: *mut GIArgument = &mut *b;
                    keep.slots.push(b);
                    ins[base + a.in_pos.unwrap()].v_pointer = p as *mut c_void;
                    out_ptr[a.out_pos.unwrap()] = p;
                    outs[a.out_pos.unwrap()].v_pointer = p as *mut c_void;
                }
            }
        }
        for (j, x) in fills {
            if let Some(pos) = plan.args[j].in_pos {
                ins[base + pos] = x;
            }
        }
        for (l, n) in &lens {
            let la = &plan.args[*l];
            if la.slot.dir == Dir::In {
                ins[base + la.in_pos.unwrap()] = int_to_arg(&la.slot.ty, *n)?;
            }
        }
        let mut ret = ZERO;
        let mut err: *mut glib::ffi::GError = std::ptr::null_mut();
        // From here on the callee may have kept the closures it was given (by their scope).
        keep.committed = true;
        let ok = unsafe { gi_function_info_invoke(self.info.0, ins.as_ptr(), ins.len(), outs.as_ptr(), outs.len(), &mut ret, &mut err) };
        if !err.is_null() {
            // Objects the callee took were ref'd for it; our temporaries go with `keep`.
            return Err(GiErr::Gerror(unsafe { from_glib_full(err) }));
        }
        if ok == 0 {
            return Err(format!("{}: invocation failed", self.name).into());
        }
        let out_len = |l: usize| -> Option<usize> {
            let la = &plan.args[l];
            match la.slot.dir {
                Dir::In => lens.get(&l).copied(),
                _ => {
                    let p = out_ptr[la.out_pos?];
                    Some(arg_to_len(&la.slot.ty, unsafe { *p }))
                }
            }
        };
        let ret_len = match &plan.ret.ty {
            Ty::Array(_, ArrLen::Arg(l)) => out_len(*l),
            _ => None,
        };
        let ret_v = read(&plan.ret, ret, ret_len, &self.name)?;
        let mut outs_v = Vec::new();
        for a in plan.args.iter().filter(|a| a.role == Role::Js && a.slot.dir != Dir::In) {
            let len = match &a.slot.ty {
                Ty::Array(_, ArrLen::Arg(l)) => out_len(*l),
                _ => None,
            };
            let op = a.out_pos.unwrap();
            let v = if a.slot.caller_alloc.is_some() {
                let Ty::Boxed(t) = a.slot.ty else { unreachable!() };
                let mut gv = glib::Value::from_type(t);
                unsafe { gobject_ffi::g_value_set_boxed(gv.to_glib_none_mut().0, outs[op].v_pointer) };
                Ret::Value(gv)
            } else {
                read(&a.slot, unsafe { *out_ptr[op] }, len, &a.name)?
            };
            outs_v.push(v);
        }
        Ok(Called { ret: ret_v, outs: outs_v })
    }
}

/// The descriptor slot of a plan type.
fn slot_desc(table: &mut TypeTable, ty: &Ty, nullable: bool) -> SlotDesc {
    let t = match ty {
        Ty::Void | Ty::Callback(_) | Ty::Internal => return SlotDesc::new(SVL_NULL),
        Ty::Bool => Type::BOOL,
        Ty::I8 => Type::I8,
        Ty::U8 => Type::U8,
        Ty::I16 | Ty::I32 => Type::I32,
        Ty::U16 | Ty::U32 => Type::U32,
        Ty::I64 => Type::I64,
        Ty::U64 => Type::U64,
        Ty::F32 => Type::F32,
        Ty::F64 => Type::F64,
        Ty::Unichar => return SlotDesc::new(SVL_I64).range(0.0, 0x10FFFF as f64),
        Ty::Gtype => {
            let mut d = SlotDesc::new(SVL_STR).typed(SVL_T_TYPE);
            if nullable {
                d.flags |= SVL_D_NULLABLE;
            }
            return d;
        }
        Ty::Str => Type::STRING,
        Ty::Strv => crate::convert::strv(),
        Ty::Enum(t) | Ty::Flags(t) | Ty::Object(t) | Ty::Boxed(t) => *t,
        Ty::List(..) | Ty::Array(..) => {
            // Untyped lists: the host encodes the elements dynamically (strings, numbers,
            // handles, boxed values) and the backend converts each by the plan's element type.
            let mut d = if matches!(ty, Ty::Array(e, _) if **e == Ty::U8) { SlotDesc::new(SVL_BYTES) } else { SlotDesc::new(SVL_LIST) };
            if nullable {
                d.flags |= SVL_D_NULLABLE;
            }
            return d;
        }
    };
    let mut d = types::slot_for(table, t, None);
    if matches!(ty, Ty::I16) {
        d = d.range(i16::MIN as f64, i16::MAX as f64);
    } else if matches!(ty, Ty::U16) {
        d = d.range(0.0, u16::MAX as f64);
    }
    if nullable {
        d.flags |= SVL_D_NULLABLE;
    } else {
        d.flags &= !SVL_D_NULLABLE;
    }
    d
}

fn int_to_arg(ty: &Ty, n: usize) -> Result<GIArgument, String> {
    let mut a = ZERO;
    match ty {
        Ty::I8 => a.v_int8 = ranged(n as i64, "length")?,
        Ty::U8 => a.v_uint8 = ranged(n as i64, "length")?,
        Ty::I16 => a.v_int16 = ranged(n as i64, "length")?,
        Ty::U16 => a.v_uint16 = ranged(n as i64, "length")?,
        Ty::I32 => a.v_int32 = ranged(n as i64, "length")?,
        Ty::U32 => a.v_uint32 = ranged(n as i64, "length")?,
        Ty::I64 => a.v_int64 = n as i64,
        Ty::U64 => a.v_uint64 = n as u64,
        t => return Err(format!("length argument of type {t:?} is not supported")),
    }
    Ok(a)
}

fn arg_to_len(ty: &Ty, a: GIArgument) -> usize {
    unsafe {
        match ty {
            Ty::I8 => a.v_int8 as usize,
            Ty::U8 => a.v_uint8 as usize,
            Ty::I16 => a.v_int16 as usize,
            Ty::U16 => a.v_uint16 as usize,
            Ty::I32 => a.v_int32 as usize,
            Ty::U32 => a.v_uint32 as usize,
            Ty::I64 => a.v_int64 as usize,
            _ => a.v_uint64 as usize,
        }
    }
}

/// A value in a list or array slot: scalars are written in place, the rest as pointers.
fn write_elem(t: &Ty, v: &svl_value, dst: *mut u8, full: bool, what: &str, keep: &mut Keep, env: &dyn Env, gi: &Gi) -> Result<(), String> {
    unsafe {
        match t {
            Ty::Bool => *(dst as *mut c_int) = match v.tag {
                SVL_BOOL => v.u.b as c_int,
                SVL_I64 => (v.u.i != 0) as c_int,
                tg => return Err(format!("{what}: expected a boolean, got tag {tg}")),
            },
            Ty::I8 => *(dst as *mut i8) = ranged(int_arg(v, what)?, what)?,
            Ty::U8 => *dst = ranged(int_arg(v, what)?, what)?,
            Ty::I16 => *(dst as *mut i16) = ranged(int_arg(v, what)?, what)?,
            Ty::U16 => *(dst as *mut u16) = ranged(int_arg(v, what)?, what)?,
            Ty::I32 => *(dst as *mut i32) = ranged(int_arg(v, what)?, what)?,
            Ty::U32 => *(dst as *mut u32) = ranged(int_arg(v, what)?, what)?,
            Ty::I64 => *(dst as *mut i64) = int_arg(v, what)?,
            Ty::U64 => *(dst as *mut u64) = ranged(int_arg(v, what)?, what)?,
            Ty::F32 | Ty::F64 => {
                let f = match v.tag {
                    SVL_F64 => v.u.f,
                    SVL_I64 => v.u.i as f64,
                    tg => return Err(format!("{what}: expected a number, got tag {tg}")),
                };
                if *t == Ty::F32 { *(dst as *mut f32) = f as f32 } else { *(dst as *mut f64) = f }
            }
            Ty::Enum(_) | Ty::Flags(_) => {
                let a = to_arg(&Slot::plain(t.clone()), v, what, keep, env, gi)?;
                *(dst as *mut u32) = a.v_uint32;
            }
            Ty::Str | Ty::Object(_) | Ty::Boxed(_) => {
                let mut slot = Slot::plain(t.clone());
                slot.full = full;
                *(dst as *mut *mut c_void) = to_arg(&slot, v, what, keep, env, gi)?.v_pointer;
            }
            other => return Err(format!("{what}: elements of type {other:?} are not supported")),
        }
    }
    Ok(())
}

fn items_of(v: &svl_value) -> &[svl_value] {
    let l = unsafe { v.u.l };
    if l.len == 0 || l.ptr.is_null() { &[] } else { unsafe { std::slice::from_raw_parts(l.ptr, l.len) } }
}

pub(crate) fn to_arg(slot: &Slot, v: &svl_value, what: &str, keep: &mut Keep, env: &dyn Env, gi: &Gi) -> Result<GIArgument, String> {
    let mut a = ZERO;
    let null_ok = |slot: &Slot| -> Result<GIArgument, String> {
        if slot.nullable { Ok(ZERO) } else { Err(format!("{what}: null is not allowed")) }
    };
    match &slot.ty {
        Ty::Void => {}
        Ty::Callback(_) | Ty::Internal => return Err(format!("{what}: not a value argument")),
        Ty::Bool => {
            a.v_boolean = match v.tag {
                SVL_BOOL => (unsafe { v.u.b }) as c_int,
                SVL_I64 => (unsafe { v.u.i } != 0) as c_int,
                t => return Err(format!("{what}: expected a boolean, got tag {t}")),
            }
        }
        Ty::I8 => a.v_int8 = ranged(int_arg(v, what)?, what)?,
        Ty::U8 => a.v_uint8 = ranged(int_arg(v, what)?, what)?,
        Ty::I16 => a.v_int16 = ranged(int_arg(v, what)?, what)?,
        Ty::U16 => a.v_uint16 = ranged(int_arg(v, what)?, what)?,
        Ty::I32 => a.v_int32 = ranged(int_arg(v, what)?, what)?,
        Ty::U32 => a.v_uint32 = ranged(int_arg(v, what)?, what)?,
        Ty::I64 => a.v_int64 = int_arg(v, what)?,
        Ty::U64 => a.v_uint64 = ranged(int_arg(v, what)?, what)?,
        Ty::Unichar => {
            let n: u32 = ranged(int_arg(v, what)?, what)?;
            if char::from_u32(n).is_none() {
                return Err(format!("{what}: {n} is not a Unicode code point"));
            }
            a.v_uint32 = n;
        }
        Ty::F32 | Ty::F64 => {
            let f = match v.tag {
                SVL_F64 => unsafe { v.u.f },
                SVL_I64 => (unsafe { v.u.i }) as f64,
                t => return Err(format!("{what}: expected a number, got tag {t}")),
            };
            if slot.ty == Ty::F32 { a.v_float = f as f32 } else { a.v_double = f }
        }
        Ty::Gtype => {
            if v.tag == SVL_NULL {
                return null_ok(slot);
            }
            let s = unsafe { as_str(v) }.ok_or_else(|| format!("{what}: expected a type name"))?;
            let _ = gi;
            let t = env.type_by_name(&s).ok_or_else(|| format!("{what}: unknown type {s:?}"))?;
            a.v_uint64 = t.into_glib() as u64;
        }
        Ty::Str => {
            if v.tag == SVL_NULL {
                return null_ok(slot);
            }
            let s = unsafe { as_str(v) }.ok_or_else(|| format!("{what}: expected a string"))?;
            let c = CString::new(s.as_ref()).map_err(|_| format!("{what}: string contains a NUL byte"))?;
            a.v_pointer = if slot.full { unsafe { glib::ffi::g_strdup(c.as_ptr()) as *mut c_void } } else { c.as_ptr() as *mut c_void };
            keep.strings.push(c);
        }
        Ty::Strv => {
            if v.tag == SVL_NULL {
                return null_ok(slot);
            }
            let gv = unsafe { to_value(crate::convert::strv(), v, env) }.map_err(|e| format!("{what}: {e}"))?;
            let strs = gv.get::<Vec<String>>().map_err(|e| format!("{what}: {e}"))?;
            let mut cs = Vec::with_capacity(strs.len());
            for s in &strs {
                cs.push(CString::new(s.as_str()).map_err(|_| format!("{what}: string contains a NUL byte"))?);
            }
            let mut ptrs: Vec<*const c_char> = cs.iter().map(|c| c.as_ptr()).collect();
            ptrs.push(std::ptr::null());
            if slot.full {
                a.v_pointer = unsafe { glib::ffi::g_strdupv(ptrs.as_ptr() as *mut *mut c_char) } as *mut c_void;
            } else {
                a.v_pointer = ptrs.as_ptr() as *mut c_void;
            }
            keep.strings.extend(cs);
            keep.strvs.push(ptrs);
        }
        Ty::Enum(t) | Ty::Flags(t) => {
            if v.tag == SVL_NULL {
                return Err(format!("{what}: null is not allowed"));
            }
            let gv = unsafe { to_value(*t, v, env) }.map_err(|e| format!("{what}: {e}"))?;
            if matches!(slot.ty, Ty::Enum(_)) {
                a.v_int32 = unsafe { glib::gobject_ffi::g_value_get_enum(gv.to_glib_none().0) };
            } else {
                a.v_uint32 = unsafe { glib::gobject_ffi::g_value_get_flags(gv.to_glib_none().0) };
            }
        }
        Ty::Object(t) | Ty::Boxed(t) => {
            if v.tag == SVL_NULL {
                return null_ok(slot);
            }
            let gv = unsafe { to_value(*t, v, env) }.map_err(|e| format!("{what}: {e}"))?;
            if matches!(slot.ty, Ty::Object(_)) {
                let o = gv.get::<Option<glib::Object>>().map_err(|e| format!("{what}: {e}"))?.ok_or_else(|| format!("{what}: null is not allowed"))?;
                if slot.full {
                    // The callee consumes one reference; give it its own.
                    unsafe { gobject_ffi::g_object_ref(o.as_ptr() as *mut _) };
                }
                a.v_pointer = o.as_ptr() as *mut c_void;
                keep.objects.push(o);
            } else {
                let p = unsafe { gobject_ffi::g_value_get_boxed(gv.to_glib_none().0) };
                if p.is_null() {
                    return Err(format!("{what}: null is not allowed"));
                }
                a.v_pointer = if slot.full { unsafe { gobject_ffi::g_boxed_copy(t.into_glib(), p) } } else { p };
                keep.values.push(gv);
            }
        }
        Ty::List(elem, slist) => {
            if v.tag == SVL_NULL {
                return null_ok(slot);
            }
            if v.tag != SVL_LIST {
                return Err(format!("{what}: expected a list"));
            }
            let owned = slot.full || slot.container;
            let mut head: *mut c_void = std::ptr::null_mut();
            for (i, item) in items_of(v).iter().enumerate() {
                let mut cell = [0u8; 8];
                write_elem(elem, item, cell.as_mut_ptr(), slot.full, &format!("{what}[{i}]"), keep, env, gi)?;
                let data = if matches!(**elem, Ty::Str | Ty::Object(_) | Ty::Boxed(_)) {
                    usize::from_ne_bytes(cell) as *mut c_void
                } else {
                    // Numbers in lists travel as GINT_TO_POINTER.
                    let n = match **elem {
                        Ty::I8 => cell[0] as i8 as isize,
                        Ty::U8 => cell[0] as isize,
                        Ty::I16 => i16::from_ne_bytes([cell[0], cell[1]]) as isize,
                        Ty::U16 => u16::from_ne_bytes([cell[0], cell[1]]) as isize,
                        Ty::I32 | Ty::Bool => i32::from_ne_bytes([cell[0], cell[1], cell[2], cell[3]]) as isize,
                        Ty::U32 | Ty::Enum(_) | Ty::Flags(_) => u32::from_ne_bytes([cell[0], cell[1], cell[2], cell[3]]) as isize,
                        Ty::I64 | Ty::U64 => isize::from_ne_bytes(cell),
                        _ => return Err(format!("{what}: lists of floating-point numbers are not supported")),
                    };
                    n as *mut c_void
                };
                head = unsafe {
                    if *slist {
                        glib::ffi::g_slist_append(head as *mut glib::ffi::GSList, data) as *mut c_void
                    } else {
                        glib::ffi::g_list_append(head as *mut glib::ffi::GList, data) as *mut c_void
                    }
                };
            }
            if !owned && !head.is_null() {
                keep.lists.push((head, *slist));
            }
            a.v_pointer = head;
        }
        Ty::Array(elem, len) => {
            if v.tag == SVL_NULL {
                keep.last_len = 0;
                return null_ok(slot);
            }
            let (data, n): (Vec<u8>, usize) = if **elem == Ty::U8 && v.tag == SVL_BYTES {
                let b = unsafe { v.u.bytes };
                (if b.len == 0 || b.ptr.is_null() { Vec::new() } else { unsafe { std::slice::from_raw_parts(b.ptr, b.len) }.to_vec() }, b.len)
            } else if v.tag == SVL_LIST {
                let es = elem_size(elem).ok_or_else(|| format!("{what}: unsupported element type"))?;
                let items = items_of(v);
                let mut buf = vec![0u8; items.len() * es];
                for (i, item) in items.iter().enumerate() {
                    write_elem(elem, item, unsafe { buf.as_mut_ptr().add(i * es) }, slot.full, &format!("{what}[{i}]"), keep, env, gi)?;
                }
                (buf, items.len())
            } else {
                return Err(format!("{what}: expected a list"));
            };
            if let ArrLen::Fixed(f) = len {
                if n != *f {
                    return Err(format!("{what}: expected {f} elements, got {n}"));
                }
            }
            let es = elem_size(elem).unwrap_or(1);
            let total = data.len() + if *len == ArrLen::Zero { es } else { 0 };
            let p = unsafe { glib::ffi::g_malloc0(total.max(1)) } as *mut u8;
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len()) };
            if !(slot.full || slot.container) {
                keep.mallocs.push(p as *mut c_void);
            }
            keep.last_len = n;
            a.v_pointer = p as *mut c_void;
        }
    }
    Ok(a)
}

/// One element read back from a list or array.
fn read_elem(t: &Ty, p: *const u8, full: bool) -> Result<Ret, String> {
    unsafe {
        Ok(match t {
            Ty::Bool => Ret::Bool(*(p as *const c_int) != 0),
            Ty::I8 => Ret::Int(*(p as *const i8) as i64),
            Ty::U8 => Ret::Int(*p as i64),
            Ty::I16 => Ret::Int(*(p as *const i16) as i64),
            Ty::U16 => Ret::Int(*(p as *const u16) as i64),
            Ty::I32 => Ret::Int(*(p as *const i32) as i64),
            Ty::U32 => Ret::Int(*(p as *const u32) as i64),
            Ty::I64 => Ret::Int(*(p as *const i64)),
            Ty::U64 => Ret::Int(*(p as *const u64) as i64),
            Ty::F32 => Ret::Float(*(p as *const f32) as f64),
            Ty::F64 => Ret::Float(*(p as *const f64)),
            Ty::Enum(_) | Ty::Flags(_) | Ty::Str | Ty::Object(_) | Ty::Boxed(_) => {
                let mut a = ZERO;
                if matches!(t, Ty::Enum(_) | Ty::Flags(_)) {
                    a.v_uint32 = *(p as *const u32);
                } else {
                    a.v_pointer = *(p as *const *mut c_void);
                }
                read(&Slot { full, ..Slot::plain(t.clone()) }, a, None, "element")?
            }
            other => return Err(format!("elements of type {other:?} are not supported")),
        })
    }
}

pub(crate) fn read(slot: &Slot, r: GIArgument, len: Option<usize>, name: &str) -> Result<Ret, String> {
    unsafe {
        Ok(match &slot.ty {
            Ty::Void | Ty::Callback(_) | Ty::Internal => Ret::Null,
            Ty::Bool => Ret::Bool(r.v_boolean != 0),
            Ty::I8 => Ret::Int(r.v_int8 as i64),
            Ty::U8 => Ret::Int(r.v_uint8 as i64),
            Ty::I16 => Ret::Int(r.v_int16 as i64),
            Ty::U16 => Ret::Int(r.v_uint16 as i64),
            Ty::I32 => Ret::Int(r.v_int32 as i64),
            Ty::U32 | Ty::Unichar => Ret::Int(r.v_uint32 as i64),
            Ty::I64 => Ret::Int(r.v_int64),
            Ty::U64 => Ret::Int(r.v_uint64 as i64),
            Ty::F32 => Ret::Float(r.v_float as f64),
            Ty::F64 => Ret::Float(r.v_double),
            Ty::Gtype => {
                let t: Type = from_glib(r.v_uint64 as glib::ffi::GType);
                if t == Type::INVALID { Ret::Null } else { Ret::Gtype(t) }
            }
            Ty::Str => {
                let p = r.v_pointer as *mut c_char;
                if p.is_null() {
                    Ret::Null
                } else {
                    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
                    if slot.full {
                        glib::ffi::g_free(p as *mut c_void);
                    }
                    Ret::Str(s)
                }
            }
            Ty::Enum(t) => {
                let mut gv = glib::Value::from_type(*t);
                gobject_ffi::g_value_set_enum(gv.to_glib_none_mut().0, r.v_int32);
                Ret::Value(gv)
            }
            Ty::Flags(t) => {
                let mut gv = glib::Value::from_type(*t);
                gobject_ffi::g_value_set_flags(gv.to_glib_none_mut().0, r.v_uint32);
                Ret::Value(gv)
            }
            Ty::Boxed(t) => {
                let p = r.v_pointer;
                if p.is_null() {
                    Ret::Null
                } else {
                    let mut gv = glib::Value::from_type(*t);
                    if slot.full {
                        gobject_ffi::g_value_take_boxed(gv.to_glib_none_mut().0, p);
                    } else {
                        gobject_ffi::g_value_set_boxed(gv.to_glib_none_mut().0, p);
                    }
                    Ret::Value(gv)
                }
            }
            Ty::Strv => {
                let p = r.v_pointer as *mut *mut c_char;
                if p.is_null() {
                    Ret::Null
                } else {
                    let mut out = Vec::new();
                    let mut i = 0;
                    while !(*p.add(i)).is_null() {
                        out.push(CStr::from_ptr(*p.add(i)).to_string_lossy().into_owned());
                        i += 1;
                    }
                    if slot.full {
                        glib::ffi::g_strfreev(p);
                    } else if slot.container {
                        glib::ffi::g_free(p as *mut c_void);
                    }
                    Ret::Strv(out)
                }
            }
            Ty::Object(_) => {
                let p = r.v_pointer as *mut gobject_ffi::GObject;
                if p.is_null() {
                    Ret::Null
                } else if slot.full {
                    if gobject_ffi::g_object_is_floating(p) != 0 {
                        gobject_ffi::g_object_ref_sink(p);
                    }
                    Ret::Object(from_glib_full(p), true)
                } else {
                    Ret::Object(from_glib_none(p), false)
                }
            }
            Ty::List(elem, slist) => {
                let head = r.v_pointer;
                if head.is_null() {
                    return Ok(Ret::List(Vec::new()));
                }
                let mut out = Vec::new();
                let mut node = head as *mut glib::ffi::GList;
                while !node.is_null() {
                    let data = (*node).data;
                    let cell: [u8; 8] = match **elem {
                        Ty::Str | Ty::Object(_) | Ty::Boxed(_) => (data as usize).to_ne_bytes(),
                        Ty::I8 | Ty::I16 | Ty::I32 | Ty::Bool => {
                            let b = (data as isize as i32).to_ne_bytes();
                            [b[0], b[1], b[2], b[3], 0, 0, 0, 0]
                        }
                        Ty::U8 | Ty::U16 | Ty::U32 | Ty::Enum(_) | Ty::Flags(_) => {
                            let b = (data as usize as u32).to_ne_bytes();
                            [b[0], b[1], b[2], b[3], 0, 0, 0, 0]
                        }
                        Ty::I64 | Ty::U64 => (data as usize).to_ne_bytes(),
                        _ => return Err(format!("{name}: lists of floating-point numbers are not supported")),
                    };
                    out.push(read_elem(elem, cell.as_ptr(), slot.full)?);
                    // GSList and GList share the layout of data/next.
                    node = (*node).next;
                }
                if slot.full || slot.container {
                    if *slist {
                        glib::ffi::g_slist_free(head as *mut glib::ffi::GSList);
                    } else {
                        glib::ffi::g_list_free(head as *mut glib::ffi::GList);
                    }
                }
                Ret::List(out)
            }
            Ty::Array(elem, alen) => {
                let p = r.v_pointer as *mut u8;
                if p.is_null() {
                    return Ok(Ret::Null);
                }
                let es = elem_size(elem).ok_or_else(|| format!("{name}: unsupported element type"))?;
                let n = match alen {
                    ArrLen::Fixed(f) => *f,
                    ArrLen::Arg(_) => len.ok_or_else(|| format!("{name}: array length is unknown"))?,
                    ArrLen::Zero => {
                        let mut n = 0;
                        while !(0..es).all(|b| *p.add(n * es + b) == 0) {
                            n += 1;
                        }
                        n
                    }
                };
                let res = if **elem == Ty::U8 {
                    Ret::Bytes(std::slice::from_raw_parts(p, n).to_vec())
                } else {
                    let mut out = Vec::with_capacity(n);
                    for i in 0..n {
                        out.push(read_elem(elem, p.add(i * es), slot.full)?);
                    }
                    Ret::List(out)
                };
                if slot.full || slot.container {
                    glib::ffi::g_free(p as *mut c_void);
                }
                res
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(ty: Ty) -> Slot {
        Slot::plain(ty)
    }

    fn int(i: i64) -> svl_value {
        svl_value::new(SVL_I64, svl_value_u { i })
    }

    struct NoEnv;
    impl Env for NoEnv {
        fn object(&self, _: u64) -> Option<glib::Object> {
            None
        }
        fn boxed(&self, _: u64) -> Option<glib::Value> {
            None
        }
        fn handle_of(&self, _: &glib::Object) -> u64 {
            0
        }
        fn mint_boxed(&self, _: glib::Value) -> u64 {
            0
        }
        fn type_id(&self, _: Type) -> svl_id {
            0
        }
    }

    fn gi() -> Gi {
        Gi { repo: std::ptr::null_mut(), cache: Default::default(), by_type: Default::default(), methods: Default::default(), cb_types: Default::default() }
    }

    fn arg(ty: Ty, v: &svl_value) -> Result<GIArgument, String> {
        let mut keep = Keep::default();
        to_arg(&slot(ty), v, "t", &mut keep, &NoEnv, &gi())
    }

    fn back(ty: Ty, r: GIArgument) -> Result<Ret, String> {
        read(&slot(ty), r, None, "t")
    }

    #[test]
    fn every_integer_width_round_trips_and_is_range_checked() {
        unsafe {
            assert_eq!(arg(Ty::I8, &int(-128)).unwrap().v_int8, -128);
            assert!(arg(Ty::I8, &int(128)).is_err());
            assert_eq!(arg(Ty::U8, &int(255)).unwrap().v_uint8, 255);
            assert!(arg(Ty::U8, &int(-1)).is_err());
            assert_eq!(arg(Ty::I16, &int(-32768)).unwrap().v_int16, -32768);
            assert!(arg(Ty::I16, &int(32768)).is_err());
            assert_eq!(arg(Ty::U16, &int(65535)).unwrap().v_uint16, 65535);
            assert!(arg(Ty::U16, &int(65536)).is_err());
            assert_eq!(arg(Ty::I32, &int(i32::MIN as i64)).unwrap().v_int32, i32::MIN);
            assert!(arg(Ty::I32, &int(1 << 31)).is_err());
            assert_eq!(arg(Ty::U32, &int(u32::MAX as i64)).unwrap().v_uint32, u32::MAX);
            assert!(arg(Ty::U32, &int(-1)).is_err());
            assert_eq!(arg(Ty::I64, &int(i64::MIN)).unwrap().v_int64, i64::MIN);
            assert_eq!(arg(Ty::U64, &int(1 << 53)).unwrap().v_uint64, 1 << 53);
            assert!(arg(Ty::U64, &int(-1)).is_err());
            assert_eq!(arg(Ty::F32, &int(2)).unwrap().v_float, 2.0);
            assert_eq!(arg(Ty::F64, &svl_value::new(SVL_F64, svl_value_u { f: 1.5 })).unwrap().v_double, 1.5);
            assert_eq!(arg(Ty::Unichar, &int(0x1F600)).unwrap().v_uint32, 0x1F600);
            assert!(arg(Ty::Unichar, &int(0xD800)).is_err());

            let mut r = ZERO;
            r.v_int8 = -5;
            assert!(matches!(back(Ty::I8, r), Ok(Ret::Int(-5))));
            r.v_uint8 = 250;
            assert!(matches!(back(Ty::U8, r), Ok(Ret::Int(250))));
            r.v_int16 = -300;
            assert!(matches!(back(Ty::I16, r), Ok(Ret::Int(-300))));
            r.v_uint16 = 60000;
            assert!(matches!(back(Ty::U16, r), Ok(Ret::Int(60000))));
            r.v_int64 = i64::MIN;
            assert!(matches!(back(Ty::I64, r), Ok(Ret::Int(i64::MIN))));
            r.v_uint64 = 1 << 40;
            assert!(matches!(back(Ty::U64, r), Ok(Ret::Int(x)) if x == 1 << 40));
            r.v_float = 0.5;
            assert!(matches!(back(Ty::F32, r), Ok(Ret::Float(x)) if x == 0.5));
            r.v_pointer = std::ptr::null_mut();
            assert!(matches!(back(Ty::Str, r), Ok(Ret::Null)));
            assert!(matches!(back(Ty::Void, r), Ok(Ret::Null)));
        }
    }

    #[test]
    fn lists_and_arrays_round_trip_through_the_c_representation() {
        // A GList of ints travels as GINT_TO_POINTER and comes back in order.
        let items = [int(3), int(-4), int(5)];
        let list = svl_value::new(SVL_LIST, svl_value_u { l: svl_list { ptr: items.as_ptr(), len: items.len() } });
        let mut keep = Keep::default();
        let mut s = slot(Ty::List(Box::new(Ty::I32), false));
        s.full = true; // the container is handed to the "callee": we read it back and free it
        let a = to_arg(&s, &list, "t", &mut keep, &NoEnv, &gi()).unwrap();
        let Ok(Ret::List(back)) = read(&s, a, None, "t") else { panic!() };
        let got: Vec<i64> = back.into_iter().map(|r| if let Ret::Int(i) = r { i } else { panic!() }).collect();
        assert_eq!(got, vec![3, -4, 5]);

        // A C array with a length: the length is reported through `last_len`.
        let mut keep = Keep::default();
        let s = slot(Ty::Array(Box::new(Ty::I32), ArrLen::Arg(1)));
        let a = to_arg(&s, &list, "t", &mut keep, &NoEnv, &gi()).unwrap();
        assert_eq!(keep.last_len, 3);
        let Ok(Ret::List(back)) = read(&s, a, Some(3), "t") else { panic!() };
        assert_eq!(back.len(), 3);
        // Byte arrays travel as BYTES.
        let data = [1u8, 2, 3];
        let b = svl_value::new(SVL_BYTES, svl_value_u { bytes: svl_bytes { ptr: data.as_ptr(), len: 3 } });
        let mut keep = Keep::default();
        let s = slot(Ty::Array(Box::new(Ty::U8), ArrLen::Arg(1)));
        let a = to_arg(&s, &b, "t", &mut keep, &NoEnv, &gi()).unwrap();
        assert!(matches!(read(&s, a, Some(3), "t"), Ok(Ret::Bytes(v)) if v == vec![1, 2, 3]));
        // A required list does not take null.
        assert!(arg(Ty::List(Box::new(Ty::I32), false), &svl_value::null()).is_err());
    }

    #[test]
    fn camel_case_names_become_snake_case() {
        assert_eq!(camel_to_snake("getFirstChild"), "get_first_child");
        assert_eq!(camel_to_snake("get_parent"), "get_parent");
        assert_eq!(camel_to_snake("setHalign"), "set_halign");
    }
}
