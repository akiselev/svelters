//! Dynamic method invocation through girepository-2.0 (plans/milestones/X1-borrowed-gi.md).
//!
//! `resolve` finds a method in the Gtk-4.0 typelib and builds an argument/return plan once per
//! `(kind, name)`; `call` converts the ABI values with that plan and invokes the function with
//! `gi_function_info_invoke`, which drives libffi. A plan that uses an unsupported feature is kept
//! (with its error) so that `call` can say exactly what is unsupported.
//!
//! The release path (M6) replaces this with generated thunks that follow the same plan shape.

use crate::convert::{as_str, to_value, Env};
use crate::types::{self, TypeTable};
use gtk::glib::{self, gobject_ffi, translate::*, Type};
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::rc::Rc;
use svelters_abi::*;
use gtk::glib::translate::ToGlibPtrMut;

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
const TRANSFER_NOTHING: c_int = 0;
const TRANSFER_EVERYTHING: c_int = 2;

#[link(name = "girepository-2.0")]
unsafe extern "C" {
    fn gi_repository_new() -> *mut c_void;
    fn gi_repository_require(repo: *mut c_void, ns: *const c_char, version: *const c_char, flags: c_int, err: *mut *mut glib::ffi::GError) -> *mut c_void;
    fn gi_repository_find_by_gtype(repo: *mut c_void, gtype: glib::ffi::GType) -> Info;
    fn gi_object_info_find_method(info: Info, name: *const c_char) -> Info;
    fn gi_object_info_get_n_interfaces(info: Info) -> c_uint;
    fn gi_object_info_get_interface(info: Info, n: c_uint) -> Info;
    fn gi_interface_info_find_method(info: Info, name: *const c_char) -> Info;
    fn gi_callable_info_get_n_args(info: Info) -> c_uint;
    fn gi_callable_info_get_arg(info: Info, n: c_uint) -> Info;
    fn gi_callable_info_get_return_type(info: Info) -> Info;
    fn gi_callable_info_get_caller_owns(info: Info) -> c_int;
    fn gi_callable_info_may_return_null(info: Info) -> c_int;
    fn gi_callable_info_can_throw_gerror(info: Info) -> c_int;
    fn gi_callable_info_is_method(info: Info) -> c_int;
    fn gi_arg_info_get_direction(info: Info) -> c_int;
    fn gi_arg_info_get_ownership_transfer(info: Info) -> c_int;
    fn gi_arg_info_may_be_null(info: Info) -> c_int;
    fn gi_arg_info_get_type_info(info: Info) -> Info;
    fn gi_type_info_get_tag(info: Info) -> c_int;
    fn gi_type_info_get_interface(info: Info) -> Info;
    fn gi_type_info_get_array_type(info: Info) -> c_int;
    fn gi_type_info_is_zero_terminated(info: Info) -> c_int;
    fn gi_type_info_get_param_type(info: Info, n: c_uint) -> Info;
    fn gi_base_info_unref(info: Info);
    fn gi_base_info_get_name(info: Info) -> *const c_char;
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

/// The wire type of one argument or return value.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    Str,
    Enum(Type),
    Flags(Type),
    Object(Type),
    /// A boxed type with a GType (records such as GdkRGBA, opaque boxed values).
    Boxed(Type),
    /// A NULL-terminated array of UTF-8 strings.
    Strv,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    ty: Ty,
    nullable: bool,
    /// `transfer: full`: the callee takes (arguments) or hands over (returns) a reference.
    full: bool,
}

struct Plan {
    args: Vec<(String, Slot)>,
    ret: Slot,
    throws: bool,
}

/// One cached typelib method.
pub struct DynMethod {
    name: String,
    info: Owned,
    declaring: Type,
    plan: Result<Plan, String>,
}

/// A converted return value. Objects carry whether the call handed over a reference.
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
    methods: RefCell<Vec<Rc<DynMethod>>>,
}

fn type_info_ty(ti: Info, what: &str) -> Result<Ty, String> {
    let tag = unsafe { gi_type_info_get_tag(ti) };
    Ok(match tag {
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
        TAG_GTYPE => return Err(format!("{what}: GType values are not supported")),
        TAG_UNICHAR => return Err(format!("{what}: gunichar values are not supported")),
        TAG_ARRAY => {
            // Only NULL-terminated arrays of strings (GStrv) are supported.
            let plain_c = unsafe { gi_type_info_get_array_type(ti) } == 0 && unsafe { gi_type_info_is_zero_terminated(ti) } != 0;
            let elem = Owned::new(unsafe { gi_type_info_get_param_type(ti, 0) });
            match elem {
                Some(e) if plain_c && matches!(unsafe { gi_type_info_get_tag(e.0) }, TAG_UTF8 | TAG_FILENAME) => Ty::Strv,
                _ => return Err(format!("{what}: arrays other than NULL-terminated string arrays are not supported")),
            }
        }
        TAG_GLIST | TAG_GSLIST => return Err(format!("{what}: list values are not supported")),
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
                    if t.is_a(Type::BOXED) {
                        Ty::Boxed(t)
                    } else {
                        return Err(format!("{what}: structs without a boxed GType are not supported"));
                    }
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
                        return Err(format!("{what}: unsupported type {}", t.name()));
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
            let mut err = std::ptr::null_mut();
            let ns = CString::new("Gtk").unwrap();
            let ver = CString::new("4.0").unwrap();
            if gi_repository_require(repo, ns.as_ptr(), ver.as_ptr(), 0, &mut err).is_null() {
                let msg = if err.is_null() { "unknown error".to_string() } else { CStr::from_ptr((*err).message).to_string_lossy().into_owned() };
                if !err.is_null() {
                    glib::ffi::g_error_free(err);
                }
                return Err(format!("cannot load the Gtk-4.0 typelib: {msg}"));
            }
            Ok(Gi { repo, cache: RefCell::new(HashMap::new()), methods: RefCell::new(Vec::new()) })
        }
    }

    /// Finds `name` (as written, or camelCase converted to snake_case) on the kind's type, its
    /// interfaces and its ancestors. Returns an index into the method list, building the plan the
    /// first time. `None` when the typelib has no such method.
    pub fn resolve(&self, kind: u32, gtype: &str, name: &str) -> Option<u32> {
        if let Some(i) = self.cache.borrow().get(&(kind, name.to_string())) {
            return Some(*i);
        }
        let start = Type::from_name(gtype)?;
        let snake = camel_to_snake(name);
        let candidates: Vec<&str> = if snake == name { vec![name] } else { vec![name, &snake] };
        let mut t = Some(start);
        while let Some(ty) = t {
            if let Some(info) = Owned::new(unsafe { gi_repository_find_by_gtype(self.repo, ty.into_glib()) }) {
                for cand in &candidates {
                    let c = CString::new(*cand).ok()?;
                    if let Some(m) = self.find_in(&info, &c) {
                        let dm = Rc::new(self.build(m, cand));
                        let mut ms = self.methods.borrow_mut();
                        ms.push(dm);
                        let idx = ms.len() as u32 - 1;
                        self.cache.borrow_mut().insert((kind, name.to_string()), idx);
                        return Some(idx);
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
        if gi_callable_info_is_method(info.0) == 0 {
            return Err(format!("{name} is a static function or constructor, not a method"));
        }
        let n = gi_callable_info_get_n_args(info.0);
        let mut args = Vec::new();
        for i in 0..n {
            let a = Owned::new(gi_callable_info_get_arg(info.0, i)).ok_or("missing argument info")?;
            let aname = CStr::from_ptr(gi_base_info_get_name(a.0)).to_string_lossy().into_owned();
            let what = format!("{name}: argument '{aname}'");
            let dir = gi_arg_info_get_direction(a.0);
            if dir != DIR_IN {
                return Err(format!("{what}: out and inout parameters are not supported"));
            }
            let ti = Owned::new(gi_arg_info_get_type_info(a.0)).ok_or("missing type info")?;
            let ty = type_info_ty(ti.0, &what).map_err(|e| {
                if e.contains("callbacks") { format!("{e} (this method takes a callback)") } else { e }
            })?;
            if ty == Ty::Void {
                return Err(format!("{what}: untyped (void) parameters, such as callback user data, are not supported"));
            }
            args.push((aname, Slot { ty, nullable: gi_arg_info_may_be_null(a.0) != 0, full: gi_arg_info_get_ownership_transfer(a.0) == TRANSFER_EVERYTHING }));
        }
        let rt = Owned::new(gi_callable_info_get_return_type(info.0)).ok_or("missing return type")?;
        let rty = type_info_ty(rt.0, &format!("{name}: return value"))?;
        let transfer = gi_callable_info_get_caller_owns(info.0);
        let ret = Slot { ty: rty, nullable: gi_callable_info_may_return_null(info.0) != 0, full: transfer != TRANSFER_NOTHING };
        Ok(Plan { args, ret, throws: gi_callable_info_can_throw_gerror(info.0) != 0 })
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
struct Keep {
    strings: Vec<CString>,
    objects: Vec<glib::Object>,
    values: Vec<glib::Value>,
    /// NULL-terminated `char *` arrays handed to the callee.
    strvs: Vec<Vec<*const c_char>>,
}

impl DynMethod {
    /// The member descriptor for the host: typed slots so that enums travel as names, flags as
    /// name sets, records and objects as their value types. `None` when the plan has an error.
    pub fn describe(&self, table: &mut TypeTable) -> Option<MemberDesc> {
        let plan = self.plan.as_ref().ok()?;
        let mut slot = |s: &Slot, name: Option<&str>| {
            let t = match s.ty {
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
                Ty::Str => Type::STRING,
                Ty::Strv => crate::convert::strv(),
                Ty::Enum(t) | Ty::Flags(t) | Ty::Object(t) | Ty::Boxed(t) => t,
            };
            let mut d = types::slot_for(table, t, None);
            if matches!(s.ty, Ty::I16) {
                d = d.range(i16::MIN as f64, i16::MAX as f64);
            } else if matches!(s.ty, Ty::U16) {
                d = d.range(0.0, u16::MAX as f64);
            }
            if s.nullable {
                d.flags |= SVL_D_NULLABLE;
            } else {
                d.flags &= !SVL_D_NULLABLE;
            }
            match name {
                Some(n) => d.named(n),
                None => d,
            }
        };
        Some(MemberDesc {
            name: self.name.clone(),
            params: plan.args.iter().map(|(n, s)| slot(s, Some(n))).collect(),
            ret: slot(&plan.ret, None),
            flags: if plan.throws { SVL_M_THROWS } else { 0 },
        })
    }

    pub fn call(&self, obj: &glib::Object, args: &[svl_value], env: &dyn Env) -> Result<Ret, GiErr> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if !obj.type_().is_a(self.declaring) {
            return Err(format!("{} is not a {}", obj.type_().name(), self.declaring.name()).into());
        }
        if args.len() != plan.args.len() {
            let want: Vec<&str> = plan.args.iter().map(|(n, _)| n.as_str()).collect();
            return Err(format!("{} takes {} argument(s) ({}), got {}", self.name, plan.args.len(), want.join(", "), args.len()).into());
        }
        let mut keep = Keep { strings: Vec::new(), objects: Vec::new(), values: Vec::new(), strvs: Vec::new() };
        let mut ins: Vec<GIArgument> = Vec::with_capacity(args.len() + 1);
        ins.push(GIArgument { v_pointer: obj.as_ptr() as *mut c_void });
        for ((aname, slot), v) in plan.args.iter().zip(args) {
            let what = format!("{} argument '{}'", self.name, aname);
            ins.push(to_arg(*slot, v, &what, &mut keep, env)?);
        }
        let mut ret = GIArgument { v_uint64: 0 };
        let mut err: *mut glib::ffi::GError = std::ptr::null_mut();
        let ok = unsafe { gi_function_info_invoke(self.info.0, ins.as_ptr(), ins.len(), std::ptr::null(), 0, &mut ret, &mut err) };
        // Objects taken by the callee (transfer full) were ref'd for it; our temporaries go now.
        drop(keep);
        if !err.is_null() {
            return Err(GiErr::Gerror(unsafe { from_glib_full(err) }));
        }
        if ok == 0 {
            return Err(format!("{}: invocation failed", self.name).into());
        }
        Ok(from_ret(plan.ret, ret, &self.name)?)
    }
}

fn to_arg(slot: Slot, v: &svl_value, what: &str, keep: &mut Keep, env: &dyn Env) -> Result<GIArgument, String> {
    let mut a = GIArgument { v_uint64: 0 };
    match slot.ty {
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
        Ty::F32 | Ty::F64 => {
            let f = match v.tag {
                SVL_F64 => unsafe { v.u.f },
                SVL_I64 => (unsafe { v.u.i }) as f64,
                t => return Err(format!("{what}: expected a number, got tag {t}")),
            };
            if slot.ty == Ty::F32 { a.v_float = f as f32 } else { a.v_double = f }
        }
        Ty::Str => {
            if v.tag == SVL_NULL {
                if !slot.nullable {
                    return Err(format!("{what}: null is not allowed"));
                }
                a.v_pointer = std::ptr::null_mut();
            } else {
                let s = unsafe { as_str(v) }.ok_or_else(|| format!("{what}: expected a string"))?;
                let c = CString::new(s.as_ref()).map_err(|_| format!("{what}: string contains a NUL byte"))?;
                a.v_pointer = if slot.full { unsafe { glib::ffi::g_strdup(c.as_ptr()) as *mut c_void } } else { c.as_ptr() as *mut c_void };
                keep.strings.push(c);
            }
        }
        Ty::Strv => {
            if v.tag == SVL_NULL {
                if !slot.nullable {
                    return Err(format!("{what}: null is not allowed"));
                }
                a.v_pointer = std::ptr::null_mut();
            } else {
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
        }
        Ty::Enum(t) | Ty::Flags(t) => {
            if v.tag == SVL_NULL {
                return Err(format!("{what}: null is not allowed"));
            }
            let gv = unsafe { to_value(t, v, env) }.map_err(|e| format!("{what}: {e}"))?;
            if matches!(slot.ty, Ty::Enum(_)) {
                a.v_int32 = unsafe { glib::gobject_ffi::g_value_get_enum(gv.to_glib_none().0) };
            } else {
                a.v_uint32 = unsafe { glib::gobject_ffi::g_value_get_flags(gv.to_glib_none().0) };
            }
        }
        Ty::Object(t) | Ty::Boxed(t) => {
            if v.tag == SVL_NULL {
                if !slot.nullable {
                    return Err(format!("{what}: null is not allowed"));
                }
                a.v_pointer = std::ptr::null_mut();
                return Ok(a);
            }
            let gv = unsafe { to_value(t, v, env) }.map_err(|e| format!("{what}: {e}"))?;
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
    }
    Ok(a)
}

fn from_ret(slot: Slot, r: GIArgument, _name: &str) -> Result<Ret, String> {
    unsafe {
        Ok(match slot.ty {
            Ty::Void => Ret::Null,
            Ty::Bool => Ret::Bool(r.v_boolean != 0),
            Ty::I8 => Ret::Int(r.v_int8 as i64),
            Ty::U8 => Ret::Int(r.v_uint8 as i64),
            Ty::I16 => Ret::Int(r.v_int16 as i64),
            Ty::U16 => Ret::Int(r.v_uint16 as i64),
            Ty::I32 => Ret::Int(r.v_int32 as i64),
            Ty::U32 => Ret::Int(r.v_uint32 as i64),
            Ty::I64 => Ret::Int(r.v_int64),
            Ty::U64 => Ret::Int(r.v_uint64 as i64),
            Ty::F32 => Ret::Float(r.v_float as f64),
            Ty::F64 => Ret::Float(r.v_double),
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
                let mut gv = glib::Value::from_type(t);
                gobject_ffi::g_value_set_enum(gv.to_glib_none_mut().0, r.v_int32);
                Ret::Value(gv)
            }
            Ty::Flags(t) => {
                let mut gv = glib::Value::from_type(t);
                gobject_ffi::g_value_set_flags(gv.to_glib_none_mut().0, r.v_uint32);
                Ret::Value(gv)
            }
            Ty::Boxed(t) => {
                let p = r.v_pointer;
                if p.is_null() {
                    Ret::Null
                } else {
                    let mut gv = glib::Value::from_type(t);
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(ty: Ty) -> Slot {
        Slot { ty, nullable: false, full: false }
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

    fn arg(ty: Ty, v: &svl_value) -> Result<GIArgument, String> {
        let mut keep = Keep { strings: Vec::new(), objects: Vec::new(), values: Vec::new(), strvs: Vec::new() };
        to_arg(slot(ty), v, "t", &mut keep, &NoEnv)
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

            let mut r = GIArgument { v_uint64: 0 };
            r.v_int8 = -5;
            assert!(matches!(from_ret(slot(Ty::I8), r, "t"), Ok(Ret::Int(-5))));
            r.v_uint8 = 250;
            assert!(matches!(from_ret(slot(Ty::U8), r, "t"), Ok(Ret::Int(250))));
            r.v_int16 = -300;
            assert!(matches!(from_ret(slot(Ty::I16), r, "t"), Ok(Ret::Int(-300))));
            r.v_uint16 = 60000;
            assert!(matches!(from_ret(slot(Ty::U16), r, "t"), Ok(Ret::Int(60000))));
            r.v_int64 = i64::MIN;
            assert!(matches!(from_ret(slot(Ty::I64), r, "t"), Ok(Ret::Int(i64::MIN))));
            r.v_uint64 = 1 << 40;
            assert!(matches!(from_ret(slot(Ty::U64), r, "t"), Ok(Ret::Int(x)) if x == 1 << 40));
            r.v_float = 0.5;
            assert!(matches!(from_ret(slot(Ty::F32), r, "t"), Ok(Ret::Float(x)) if x == 0.5));
            r.v_pointer = std::ptr::null_mut();
            assert!(matches!(from_ret(slot(Ty::Str), r, "t"), Ok(Ret::Null)));
            assert!(matches!(from_ret(slot(Ty::Void), r, "t"), Ok(Ret::Null)));
        }
    }

    #[test]
    fn camel_case_names_become_snake_case() {
        assert_eq!(camel_to_snake("getFirstChild"), "get_first_child");
        assert_eq!(camel_to_snake("get_parent"), "get_parent");
        assert_eq!(camel_to_snake("setHalign"), "set_halign");
    }
}
