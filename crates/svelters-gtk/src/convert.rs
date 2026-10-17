//! `svl_value` <-> `glib::Value` conversion, driven by the generated property type table.

use crate::generated::tables::ValueTy;
use gtk::glib::{self, translate::ToGlibPtr, Type};
use gtk::prelude::*;
use std::ffi::c_char;
use svelters_abi::*;

/// # Safety
/// `v` must satisfy the ABI contract (valid pointers for the tag).
pub unsafe fn as_str(v: &svl_value) -> Option<&str> {
    if v.tag != SVL_STR {
        return None;
    }
    let s = unsafe { v.u.s };
    if s.len == 0 {
        return Some("");
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(s.ptr as *const u8, s.len) }).ok()
}

unsafe fn as_i64(v: &svl_value) -> Result<i64, String> {
    match v.tag {
        SVL_I64 => Ok(unsafe { v.u.i }),
        SVL_BOOL => Ok(unsafe { v.u.b } as i64),
        SVL_F64 => {
            let f = unsafe { v.u.f };
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                Ok(f as i64)
            } else {
                Err(format!("{f} is not an integer"))
            }
        }
        t => Err(format!("expected integer, got tag {t}")),
    }
}

unsafe fn as_f64(v: &svl_value) -> Result<f64, String> {
    match v.tag {
        SVL_F64 => Ok(unsafe { v.u.f }),
        SVL_I64 => Ok(unsafe { v.u.i } as f64),
        t => Err(format!("expected number, got tag {t}")),
    }
}

fn range<T: TryFrom<i64>>(x: i64) -> Result<T, String> {
    T::try_from(x).map_err(|_| format!("{x} out of range"))
}

/// Convert an ABI value to the `glib::Value` a property of table type `ty` expects.
///
/// # Safety
/// `v` must satisfy the ABI contract.
pub unsafe fn to_value(
    ty: ValueTy,
    pspec: &glib::ParamSpec,
    v: &svl_value,
    obj_of: &dyn Fn(u64) -> Option<glib::Object>,
) -> Result<glib::Value, String> {
    let vt = pspec.value_type();
    unsafe {
        Ok(match ty {
            ValueTy::Bool => (as_i64(v)? != 0).to_value(),
            ValueTy::I32 if vt == Type::I8 => range::<i8>(as_i64(v)?)?.to_value(),
            ValueTy::I32 => range::<i32>(as_i64(v)?)?.to_value(),
            ValueTy::U32 if vt == Type::U8 => range::<u8>(as_i64(v)?)?.to_value(),
            ValueTy::U32 => range::<u32>(as_i64(v)?)?.to_value(),
            ValueTy::I64 if vt == Type::I64 => as_i64(v)?.to_value(),
            ValueTy::U64 if vt == Type::U64 => range::<u64>(as_i64(v)?)?.to_value(),
            ValueTy::F32 => (as_f64(v)? as f32).to_value(),
            ValueTy::F64 => as_f64(v)?.to_value(),
            ValueTy::Str => match v.tag {
                SVL_NULL => None::<String>.to_value(),
                _ => as_str(v).ok_or("expected string")?.to_value(),
            },
            ValueTy::Enum => {
                let class = glib::EnumClass::with_type(vt).ok_or("not an enum type")?;
                match v.tag {
                    SVL_STR => class.to_value_by_nick(as_str(v).ok_or("invalid utf-8")?),
                    _ => class.to_value(range::<i32>(as_i64(v)?)?),
                }
                .ok_or_else(|| format!("invalid {} value", vt.name()))?
            }
            ValueTy::Flags => {
                let class = glib::FlagsClass::with_type(vt).ok_or("not a flags type")?;
                match v.tag {
                    SVL_STR => class.to_value_by_nick(as_str(v).ok_or("invalid utf-8")?),
                    _ => class.to_value(range::<u32>(as_i64(v)?)?),
                }
                .ok_or_else(|| format!("invalid {} value", vt.name()))?
            }
            ValueTy::Object => match v.tag {
                SVL_NULL => glib::Value::from_type(vt),
                SVL_HANDLE => {
                    let o = obj_of(v.u.h).ok_or("stale handle")?;
                    if !o.type_().is_a(vt) {
                        return Err(format!("{} is not a {}", o.type_().name(), vt.name()));
                    }
                    o.to_value()
                }
                _ => return Err("expected handle".into()),
            },
            // GStrv (e.g. GtkWidget:css-classes): a space-separated token string, null = empty.
            ValueTy::Other if vt == glib::StrV::static_type() => match v.tag {
                SVL_NULL => glib::StrV::new().to_value(),
                _ => {
                    let s = as_str(v).ok_or("expected string")?;
                    glib::StrV::from(s.split_ascii_whitespace().collect::<Vec<_>>()).to_value()
                }
            },
            other => return Err(format!("unsupported property type {other:?}")),
        })
    }
}

/// Owns the bytes of strings handed out through `svl_value`s.
#[derive(Default)]
pub struct StrBuf(Vec<Box<[u8]>>);

impl StrBuf {
    pub fn str_value(&mut self, s: &str) -> svl_value {
        let b: Box<[u8]> = s.as_bytes().into();
        let ptr = b.as_ptr() as *const c_char;
        let len = b.len();
        self.0.push(b);
        svl_value { tag: SVL_STR, u: svl_value_u { s: svl_str { ptr, len } } }
    }
}

pub fn null_value() -> svl_value {
    svl_value { tag: SVL_NULL, u: svl_value_u { i: 0 } }
}

/// Convert a `glib::Value` to an ABI value; strings are kept alive by `buf`.
pub fn from_value(v: &glib::Value, handle_of: &dyn Fn(&glib::Object) -> u64, buf: &mut StrBuf) -> svl_value {
    let t = v.type_();
    let mk = |tag, u| svl_value { tag, u };
    if t == Type::BOOL {
        return mk(SVL_BOOL, svl_value_u { b: v.get::<bool>().unwrap_or(false) });
    }
    if t == Type::STRING {
        return match v.get::<Option<String>>() {
            Ok(Some(s)) => buf.str_value(&s),
            _ => null_value(),
        };
    }
    if t == glib::StrV::static_type() {
        let parts = v.get::<glib::StrV>().unwrap_or_default();
        let joined: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        return buf.str_value(&joined.join(" "));
    }
    if t.is_a(Type::ENUM) {
        return match glib::EnumValue::from_value(v) {
            Some((_, ev)) => buf.str_value(ev.nick()),
            None => null_value(),
        };
    }
    if t.is_a(Type::FLAGS) {
        let n = unsafe { glib::gobject_ffi::g_value_get_flags(v.to_glib_none().0) };
        return mk(SVL_I64, svl_value_u { i: n as i64 });
    }
    if t.is_a(Type::OBJECT) {
        return match v.get::<Option<glib::Object>>() {
            Ok(Some(o)) => match handle_of(&o) {
                0 => null_value(),
                h => mk(SVL_HANDLE, svl_value_u { h }),
            },
            _ => null_value(),
        };
    }
    let int = |x: i64| mk(SVL_I64, svl_value_u { i: x });
    let float = |x: f64| mk(SVL_F64, svl_value_u { f: x });
    if t == Type::I32 {
        return int(v.get::<i32>().unwrap_or(0) as i64);
    }
    if t == Type::U32 {
        return int(v.get::<u32>().unwrap_or(0) as i64);
    }
    if t == Type::I64 {
        return int(v.get::<i64>().unwrap_or(0));
    }
    if t == Type::U64 {
        return int(v.get::<u64>().unwrap_or(0) as i64);
    }
    if t == Type::I8 {
        return int(v.get::<i8>().unwrap_or(0) as i64);
    }
    if t == Type::U8 {
        return int(v.get::<u8>().unwrap_or(0) as i64);
    }
    if t == Type::F32 {
        return float(v.get::<f32>().unwrap_or(0.0) as f64);
    }
    if t == Type::F64 {
        return float(v.get::<f64>().unwrap_or(0.0));
    }
    null_value()
}
