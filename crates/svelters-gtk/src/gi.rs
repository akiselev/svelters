//! Dynamic method invocation through girepository-2.0 (plans/milestones/X1-borrowed-gi.md).
//!
//! `resolve` finds a method in the Gtk-4.0 typelib and builds an argument/return plan once per
//! `(kind, name)`; `call` converts the ABI values with that plan and invokes the function with
//! `gi_function_info_invoke`, which drives libffi. A plan that uses an unsupported feature is kept
//! (with its error) so that `call` can say exactly what is unsupported.
//!
//! The release path (M6) replaces this with generated thunks that follow the same plan shape.

use crate::convert::as_str;
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
    /// An enum value as its nick.
    Nick(String),
    Object(glib::Object, bool),
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
        TAG_ARRAY => return Err(format!("{what}: array values are not supported")),
        TAG_GLIST | TAG_GSLIST => return Err(format!("{what}: list values are not supported")),
        TAG_GHASH => return Err(format!("{what}: hash table values are not supported")),
        TAG_ERROR => return Err(format!("{what}: GError values are not supported")),
        TAG_INTERFACE => {
            let iface = Owned::new(unsafe { gi_type_info_get_interface(ti) }).ok_or_else(|| format!("{what}: interface type has no info"))?;
            let tname = unsafe { CStr::from_ptr(gobject_ffi::g_type_name_from_instance(iface.0 as *mut gobject_ffi::GTypeInstance)) }.to_string_lossy().into_owned();
            match tname.as_str() {
                "GICallbackInfo" => return Err(format!("{what}: callbacks are not supported")),
                "GIStructInfo" | "GIUnionInfo" => return Err(format!("{what}: structs, unions and boxed values are not supported (ABI v3)")),
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
}

impl DynMethod {
    pub fn call(&self, obj: &glib::Object, args: &[svl_value], obj_of: &dyn Fn(u64) -> Option<glib::Object>) -> Result<Ret, String> {
        let plan = self.plan.as_ref().map_err(|e| e.clone())?;
        if !obj.type_().is_a(self.declaring) {
            return Err(format!("{} is not a {}", obj.type_().name(), self.declaring.name()));
        }
        if args.len() != plan.args.len() {
            let want: Vec<&str> = plan.args.iter().map(|(n, _)| n.as_str()).collect();
            return Err(format!("{} takes {} argument(s) ({}), got {}", self.name, plan.args.len(), want.join(", "), args.len()));
        }
        let mut keep = Keep { strings: Vec::new(), objects: Vec::new() };
        let mut ins: Vec<GIArgument> = Vec::with_capacity(args.len() + 1);
        ins.push(GIArgument { v_pointer: obj.as_ptr() as *mut c_void });
        for ((aname, slot), v) in plan.args.iter().zip(args) {
            let what = format!("{} argument '{}'", self.name, aname);
            ins.push(to_arg(*slot, v, &what, &mut keep, obj_of)?);
        }
        let mut ret = GIArgument { v_uint64: 0 };
        let mut err: *mut glib::ffi::GError = std::ptr::null_mut();
        let ok = unsafe { gi_function_info_invoke(self.info.0, ins.as_ptr(), ins.len(), std::ptr::null(), 0, &mut ret, &mut err) };
        // Objects taken by the callee (transfer full) were ref'd for it; our temporaries go now.
        drop(keep);
        if ok == 0 || !err.is_null() {
            let msg = if err.is_null() {
                format!("{}: invocation failed", self.name)
            } else {
                let m = unsafe { CStr::from_ptr((*err).message) }.to_string_lossy().into_owned();
                unsafe { glib::ffi::g_error_free(err) };
                format!("{}: {m}", self.name)
            };
            return Err(msg);
        }
        let _ = plan.throws;
        from_ret(plan.ret, ret, &self.name)
    }
}

fn to_arg(slot: Slot, v: &svl_value, what: &str, keep: &mut Keep, obj_of: &dyn Fn(u64) -> Option<glib::Object>) -> Result<GIArgument, String> {
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
                let c = CString::new(s).map_err(|_| format!("{what}: string contains a NUL byte"))?;
                a.v_pointer = if slot.full { unsafe { glib::ffi::g_strdup(c.as_ptr()) as *mut c_void } } else { c.as_ptr() as *mut c_void };
                keep.strings.push(c);
            }
        }
        Ty::Enum(t) => {
            let class = glib::EnumClass::with_type(t).ok_or_else(|| format!("{what}: {} is not an enum", t.name()))?;
            a.v_int32 = match v.tag {
                SVL_STR => {
                    let nick = unsafe { as_str(v) }.ok_or_else(|| format!("{what}: expected a string"))?;
                    class.value_by_nick(nick).ok_or_else(|| format!("{what}: '{nick}' is not a {} value", t.name()))?.value()
                }
                _ => {
                    let n: i32 = ranged(int_arg(v, what)?, what)?;
                    class.value(n).ok_or_else(|| format!("{what}: {n} is not a {} value", t.name()))?;
                    n
                }
            };
        }
        Ty::Flags(t) => {
            let class = glib::FlagsClass::with_type(t).ok_or_else(|| format!("{what}: {} is not a flags type", t.name()))?;
            a.v_uint32 = match v.tag {
                SVL_STR => {
                    let text = unsafe { as_str(v) }.ok_or_else(|| format!("{what}: expected a string"))?;
                    let mut bits = 0u32;
                    for nick in text.split(['|', ',']).map(str::trim).filter(|s| !s.is_empty()) {
                        bits |= class.value_by_nick(nick).ok_or_else(|| format!("{what}: '{nick}' is not a {} flag", t.name()))?.value();
                    }
                    bits
                }
                _ => ranged(int_arg(v, what)?, what)?,
            };
        }
        Ty::Object(t) => match v.tag {
            SVL_NULL => {
                if !slot.nullable {
                    return Err(format!("{what}: null is not allowed"));
                }
                a.v_pointer = std::ptr::null_mut();
            }
            SVL_HANDLE => {
                let o = obj_of(unsafe { v.u.h }).ok_or_else(|| format!("{what}: stale or unknown handle"))?;
                if !o.type_().is_a(t) {
                    return Err(format!("{what}: {} is not a {}", o.type_().name(), t.name()));
                }
                if slot.full {
                    // The callee consumes one reference; give it its own.
                    unsafe { gobject_ffi::g_object_ref(o.as_ptr() as *mut _) };
                }
                a.v_pointer = o.as_ptr() as *mut c_void;
                keep.objects.push(o);
            }
            tag => return Err(format!("{what}: expected a handle, got tag {tag}")),
        },
    }
    Ok(a)
}

fn from_ret(slot: Slot, r: GIArgument, name: &str) -> Result<Ret, String> {
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
                let class = glib::EnumClass::with_type(t).ok_or_else(|| format!("{name}: {} is not an enum", t.name()))?;
                match class.value(r.v_int32) {
                    Some(ev) => Ret::Nick(ev.nick().to_string()),
                    None => Ret::Int(r.v_int32 as i64),
                }
            }
            Ty::Flags(_) => Ret::Int(r.v_uint32 as i64),
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
        svl_value { tag: SVL_I64, u: svl_value_u { i } }
    }

    fn arg(ty: Ty, v: &svl_value) -> Result<GIArgument, String> {
        let mut keep = Keep { strings: Vec::new(), objects: Vec::new() };
        to_arg(slot(ty), v, "t", &mut keep, &|_| None)
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
            assert_eq!(arg(Ty::F64, &svl_value { tag: SVL_F64, u: svl_value_u { f: 1.5 } }).unwrap().v_double, 1.5);

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
