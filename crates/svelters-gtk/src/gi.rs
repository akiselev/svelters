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

type Info = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
union GIArgument {
    v_boolean: c_int,
    v_int8: i8,
    v_uint8: u8,
    v_int16: i16,
    v_uint16: u16,
    v_int32: i32,
    v_uint32: u32,
    v_int64: i64,
    v_uint64: u64,
    v_float: f32,
    v_double: f64,
    v_pointer: *mut c_void,
}

const ZERO: GIArgument = GIArgument { v_uint64: 0 };

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
const TRANSFER_NOTHING: c_int = 0;
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
    fn gi_base_info_get_name(info: Info) -> *const c_char;
    fn gi_base_info_get_namespace(info: Info) -> *const c_char;
    fn gi_base_info_get_container(info: Info) -> Info;
    fn gi_registered_type_info_get_g_type(info: Info) -> glib::ffi::GType;
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
struct Owned(Info);

impl Owned {
    fn new(p: Info) -> Option<Owned> {
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
enum ArrLen {
    Zero,
    /// The native argument at this index holds the length.
    Arg(usize),
    Fixed(usize),
}

/// The wire type of one argument or return value.
#[derive(Clone, Debug, PartialEq)]
enum Ty {
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    In,
    Out,
    InOut,
}

#[derive(Clone, Debug)]
struct Slot {
    ty: Ty,
    nullable: bool,
    /// `transfer: full`: the callee takes (arguments) or hands over (returns) the elements.
    full: bool,
    /// `transfer: container`: the callee takes only the container.
    container: bool,
    dir: Dir,
    /// An out struct the caller allocates (`caller-allocates`), with its size.
    caller_alloc: Option<usize>,
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
}

struct PlanArg {
    name: String,
    slot: Slot,
    role: Role,
    in_pos: Option<usize>,
    out_pos: Option<usize>,
}

struct Plan {
    args: Vec<PlanArg>,
    ret: Slot,
    throws: bool,
    is_method: bool,
    is_ctor: bool,
    n_in: usize,
    n_out: usize,
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
            Ok(Gi { repo, cache: RefCell::new(HashMap::new()), by_type: RefCell::new(HashMap::new()), methods: RefCell::new(Vec::new()) })
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
        let declaring = unsafe {
            let c = gi_base_info_get_container(info.0);
            if c.is_null() { Type::INVALID } else { from_glib(gi_registered_type_info_get_g_type(c)) }
        };
        let plan = plan_of(&info, name);
        DynMethod { name: name.to_string(), info, declaring, plan }
    }

    pub fn method(&self, idx: u32) -> Option<Rc<DynMethod>> {
        self.methods.borrow().get(idx as usize).cloned()
    }
}

fn plan_of(info: &Owned, name: &str) -> Result<Plan, String> {
    unsafe {
        let flags = gi_function_info_get_flags(info.0);
        let is_method = gi_callable_info_is_method(info.0) != 0;
        let n = gi_callable_info_get_n_args(info.0);
        let mut args: Vec<PlanArg> = Vec::new();
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
            let caller_alloc = dir == Dir::Out && gi_arg_info_is_caller_allocates(a.0) != 0;
            let ty = type_info_ty_ref(ti.0, &what, caller_alloc).map_err(|e| {
                if e.contains("callbacks") { format!("{e} (this method takes a callback)") } else { e }
            })?;
            if ty == Ty::Void {
                return Err(format!("{what}: untyped (void) parameters, such as callback user data, are not supported"));
            }
            let transfer = gi_arg_info_get_ownership_transfer(a.0);
            let mut slot = Slot {
                ty,
                nullable: gi_arg_info_may_be_null(a.0) != 0,
                full: transfer == TRANSFER_EVERYTHING,
                container: transfer == TRANSFER_CONTAINER,
                dir,
                caller_alloc: None,
            };
            if dir == Dir::Out && gi_arg_info_is_caller_allocates(a.0) != 0 {
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
        Ok(Plan { args, ret, throws: gi_callable_info_can_throw_gerror(info.0) != 0, is_method, is_ctor: flags & FN_IS_CONSTRUCTOR != 0, n_in, n_out })
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
struct Keep {
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
}

impl Drop for Keep {
    fn drop(&mut self) {
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

impl DynMethod {
    pub fn is_static(&self) -> bool {
        self.plan.as_ref().is_ok_and(|p| !p.is_method)
    }

    pub fn has_outs(&self) -> bool {
        self.plan.as_ref().is_ok_and(|p| p.has_outs())
    }

    pub fn declaring(&self) -> Type {
        self.declaring
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The member descriptor for the host: typed slots so that enums travel as names, flags as
    /// name sets, records and objects as their value types. `None` when the plan has an error.
    pub fn describe(&self, table: &mut TypeTable) -> Option<MemberDesc> {
        let plan = self.plan.as_ref().ok()?;
        let mut params = Vec::new();
        for a in &plan.args {
            if a.role != Role::Js {
                continue;
            }
            let mut d = slot_desc(table, &a.slot.ty, a.slot.nullable).named(&a.name);
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
        Some(MemberDesc { name: self.name.clone(), params, ret: slot_desc(table, &plan.ret.ty, plan.ret.nullable), flags })
    }

    /// Calls the function with an object receiver.
    pub fn call(&self, obj: &glib::Object, args: &[svl_value], env: &dyn Env, gi: &Gi) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if !plan.is_method {
            return Err(format!("{} is a static function or constructor, not a method", self.name).into());
        }
        if !obj.type_().is_a(self.declaring) {
            return Err(format!("{} is not a {}", obj.type_().name(), self.declaring.name()).into());
        }
        self.invoke(plan, Some(obj.as_ptr() as *mut c_void), args, env, gi)
    }

    /// Calls the function with a boxed receiver (a VALUE handle).
    pub fn call_boxed(&self, value: &glib::Value, args: &[svl_value], env: &dyn Env, gi: &Gi) -> Result<Called, GiErr> {
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
        self.invoke(plan, Some(p as *mut c_void), args, env, gi)
    }

    /// Calls a static function or constructor.
    pub fn call_static(&self, args: &[svl_value], env: &dyn Env, gi: &Gi) -> Result<Called, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if plan.is_method {
            return Err(format!("{} is an instance method; call it on an object", self.name).into());
        }
        self.invoke(plan, None, args, env, gi)
    }

    fn invoke(&self, plan: &Plan, recv: Option<*mut c_void>, args: &[svl_value], env: &dyn Env, gi: &Gi) -> Result<Called, GiErr> {
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
        let mut next_js = 0;
        // Pointers to the out storage, by out position.
        let mut out_ptr: Vec<*mut GIArgument> = vec![std::ptr::null_mut(); plan.n_out];
        for (i, a) in plan.args.iter().enumerate() {
            match (a.role, a.slot.dir) {
                (Role::Len(_), Dir::In) => {}
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
                    let arg = to_arg(&a.slot, &args[next_js], &what, &mut keep, env, gi)?;
                    next_js += 1;
                    ins[base + a.in_pos.unwrap()] = arg;
                    if let Ty::Array(_, ArrLen::Arg(l)) = &a.slot.ty {
                        lens.insert(*l, keep.last_len);
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
            let _ = i;
        }
        for (l, n) in &lens {
            let la = &plan.args[*l];
            if la.slot.dir == Dir::In {
                ins[base + la.in_pos.unwrap()] = int_to_arg(&la.slot.ty, *n)?;
            }
        }
        let mut ret = ZERO;
        let mut err: *mut glib::ffi::GError = std::ptr::null_mut();
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
        Ty::Void => return SlotDesc::new(SVL_NULL),
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

fn to_arg(slot: &Slot, v: &svl_value, what: &str, keep: &mut Keep, env: &dyn Env, gi: &Gi) -> Result<GIArgument, String> {
    let mut a = ZERO;
    let null_ok = |slot: &Slot| -> Result<GIArgument, String> {
        if slot.nullable { Ok(ZERO) } else { Err(format!("{what}: null is not allowed")) }
    };
    match &slot.ty {
        Ty::Void => {}
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

fn read(slot: &Slot, r: GIArgument, len: Option<usize>, name: &str) -> Result<Ret, String> {
    unsafe {
        Ok(match &slot.ty {
            Ty::Void => Ret::Null,
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
        Gi { repo: std::ptr::null_mut(), cache: Default::default(), by_type: Default::default(), methods: Default::default() }
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
