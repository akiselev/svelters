//! `svl_value` <-> `glib::Value` conversion, keyed by the GType of the property or argument.
//!
//! One converter per GType class (the registry is the `match` in [`to_value`] / [`from_value`]):
//! fundamentals, enums and flags (by native value; names are resolved by the host from the type
//! descriptors, nicks are still accepted), `GStrv`, objects (known kinds travel as `HANDLE`, other
//! objects as `BOXED` wrappers), and boxed types: the value records (`GdkRGBA`, `GdkRectangle`,
//! `graphene_*`, `GtkBorder`) plus a generic opaque `BOXED` handle for everything else.

use crate::types;
use gtk::glib::{self, translate::ToGlibPtr, translate::ToGlibPtrMut, Type};
use gtk::prelude::*;
use gtk::{gdk, graphene};
use std::ffi::c_char;
use svelters_abi::*;

/// What the converters need from the backend state.
pub trait Env {
    fn object(&self, h: u64) -> Option<glib::Object>;
    /// The `glib::Value` behind a `BOXED` handle.
    fn boxed(&self, h: u64) -> Option<glib::Value>;
    /// The handle of an object we created or already minted, 0 if none.
    fn handle_of(&self, o: &glib::Object) -> u64;
    fn mint_boxed(&self, v: glib::Value) -> u64;
    fn type_id(&self, t: Type) -> svl_id;
}

fn units16<'a>(v: &'a svl_value) -> &'a [u16] {
    let s = unsafe { v.u.s };
    if s.len == 0 || s.ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(s.ptr as *const u16, s.len) }
    }
}

/// # Safety
/// `v` must satisfy the ABI contract (valid pointers for the tag).
pub unsafe fn as_str(v: &svl_value) -> Option<std::borrow::Cow<'_, str>> {
    if v.tag != SVL_STR {
        return None;
    }
    if v.flags & SVL_F_UTF16 != 0 {
        return String::from_utf16(units16(v)).ok().map(std::borrow::Cow::Owned);
    }
    let s = unsafe { v.u.s };
    if s.len == 0 {
        return Some("".into());
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(s.ptr as *const u8, s.len) })
        .ok()
        .map(std::borrow::Cow::Borrowed)
}

fn items(v: &svl_value) -> &[svl_value] {
    let l = unsafe { v.u.l };
    if l.len == 0 || l.ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(l.ptr, l.len) }
    }
}

unsafe fn as_i64(v: &svl_value) -> Result<i64, String> {
    match v.tag {
        SVL_I64 | SVL_ENUM if v.flags & SVL_F_UNSIGNED != 0 => {
            i64::try_from(unsafe { v.u.u }).map_err(|_| "value out of range".to_string())
        }
        SVL_I64 | SVL_ENUM => Ok(unsafe { v.u.i }),
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

pub fn strv() -> Type {
    <Vec<String> as StaticType>::static_type()
}

pub fn is_iface(t: Type) -> bool {
    t.is_a(Type::INTERFACE)
}

fn flags_value(t: Type, mask: u32) -> glib::Value {
    let mut out = glib::Value::from_type(t);
    unsafe { glib::gobject_ffi::g_value_set_flags(out.to_glib_none_mut().0, mask) };
    out
}

fn range<T: TryFrom<i64>>(x: i64) -> Result<T, String> {
    T::try_from(x).map_err(|_| format!("{x} out of range"))
}

// ---- records ----

/// The value records, by GType.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rec {
    RectF,
    RectI,
    SizeF,
    PointF,
    Insets,
    Color,
}

impl Rec {
    pub fn of(t: Type) -> Option<Rec> {
        Some(match t.name() {
            "graphene_rect_t" => Rec::RectF,
            "GdkRectangle" => Rec::RectI,
            "graphene_size_t" => Rec::SizeF,
            "graphene_point_t" => Rec::PointF,
            "GtkBorder" => Rec::Insets,
            "GdkRGBA" => Rec::Color,
            _ => return None,
        })
    }

    /// Portable field names, in wire order.
    pub fn fields(self) -> &'static [&'static str] {
        match self {
            Rec::RectF | Rec::RectI => &["x", "y", "width", "height"],
            Rec::SizeF => &["width", "height"],
            Rec::PointF => &["x", "y"],
            Rec::Insets => &["top", "right", "bottom", "left"],
            Rec::Color => &["red", "green", "blue", "alpha"],
        }
    }

    pub fn integral(self) -> bool {
        matches!(self, Rec::RectI | Rec::Insets)
    }

    /// Well-known type id for the float forms; the integer forms get backend ids.
    pub fn well_known(self) -> Option<svl_id> {
        match self {
            Rec::RectF => Some(SVL_T_RECT),
            Rec::SizeF => Some(SVL_T_SIZE),
            Rec::PointF => Some(SVL_T_POINT),
            Rec::Insets => Some(SVL_T_INSETS),
            Rec::Color => Some(SVL_T_COLOR),
            Rec::RectI => None,
        }
    }

    pub fn type_name(self) -> &'static str {
        match self {
            Rec::RectF | Rec::RectI => "Rect",
            Rec::SizeF => "Size",
            Rec::PointF => "Point",
            Rec::Insets => "Insets",
            Rec::Color => "Color",
        }
    }

    fn gtype(self) -> Type {
        match self {
            Rec::RectF => graphene::Rect::static_type(),
            Rec::RectI => gdk::Rectangle::static_type(),
            Rec::SizeF => graphene::Size::static_type(),
            Rec::PointF => graphene::Point::static_type(),
            Rec::Insets => gtk::Border::static_type(),
            Rec::Color => gdk::RGBA::static_type(),
        }
    }
}

fn rec_in(rec: Rec, v: &svl_value) -> Result<glib::Value, String> {
    if v.tag == SVL_STR && rec == Rec::Color {
        let s = unsafe { as_str(v) }.ok_or("invalid utf-8")?;
        let c = gdk::RGBA::parse(&*s).map_err(|_| format!("cannot parse color {s:?}"))?;
        return Ok(c.to_value());
    }
    if v.tag != SVL_RECORD {
        return Err(format!("expected a {} record", rec.type_name()));
    }
    let f = items(v);
    let n = rec.fields().len();
    if f.len() != n {
        return Err(format!("{} needs {n} fields, got {}", rec.type_name(), f.len()));
    }
    let num = |i: usize| unsafe { as_f64(&f[i]) };
    let int = |i: usize| -> Result<i32, String> {
        let x = unsafe { as_i64(&f[i]) }?;
        range::<i32>(x)
    };
    Ok(match rec {
        Rec::RectF => graphene::Rect::new(num(0)? as f32, num(1)? as f32, num(2)? as f32, num(3)? as f32).to_value(),
        Rec::RectI => gdk::Rectangle::new(int(0)?, int(1)?, int(2)?, int(3)?).to_value(),
        Rec::SizeF => graphene::Size::new(num(0)? as f32, num(1)? as f32).to_value(),
        Rec::PointF => graphene::Point::new(num(0)? as f32, num(1)? as f32).to_value(),
        Rec::Insets => {
            let mut b = gtk::Border::new();
            b.set_top(range::<i16>(int(0)? as i64)?);
            b.set_right(range::<i16>(int(1)? as i64)?);
            b.set_bottom(range::<i16>(int(2)? as i64)?);
            b.set_left(range::<i16>(int(3)? as i64)?);
            b.to_value()
        }
        Rec::Color => gdk::RGBA::new(num(0)? as f32, num(1)? as f32, num(2)? as f32, num(3)? as f32).to_value(),
    })
}

fn rec_out(rec: Rec, v: &glib::Value, env: &dyn Env, buf: &mut StrBuf) -> Option<svl_value> {
    let f = |x: f64| svl_value::new(SVL_F64, svl_value_u { f: x });
    let i = |x: i64| svl_value::new(SVL_I64, svl_value_u { i: x });
    let fields: Vec<svl_value> = match rec {
        Rec::RectF => {
            let r = v.get::<graphene::Rect>().ok()?;
            vec![f(r.x() as f64), f(r.y() as f64), f(r.width() as f64), f(r.height() as f64)]
        }
        Rec::RectI => {
            let r = v.get::<gdk::Rectangle>().ok()?;
            vec![i(r.x() as i64), i(r.y() as i64), i(r.width() as i64), i(r.height() as i64)]
        }
        Rec::SizeF => {
            let r = v.get::<graphene::Size>().ok()?;
            vec![f(r.width() as f64), f(r.height() as f64)]
        }
        Rec::PointF => {
            let r = v.get::<graphene::Point>().ok()?;
            vec![f(r.x() as f64), f(r.y() as f64)]
        }
        Rec::Insets => {
            let b = v.get::<gtk::Border>().ok()?;
            vec![i(b.top() as i64), i(b.right() as i64), i(b.bottom() as i64), i(b.left() as i64)]
        }
        Rec::Color => {
            let c = v.get::<gdk::RGBA>().ok()?;
            vec![f(c.red() as f64), f(c.green() as f64), f(c.blue() as f64), f(c.alpha() as f64)]
        }
    };
    let ty = env.type_id(rec.gtype());
    Some(buf.list_value(SVL_RECORD, ty, fields))
}

// ---- inbound ----

/// String forms: the canonical text of boxed or interface types that have one.
fn from_string_form(t: Type, s: &str) -> Option<Result<glib::Value, String>> {
    match t.name() {
        "PangoFontDescription" => Some(Ok(gtk::pango::FontDescription::from_string(s).to_value())),
        "GIcon" => Some(Ok(gtk::gio::ThemedIcon::new(s).upcast::<glib::Object>().to_value())),
        _ => None,
    }
}

/// Converts an ABI value to a `glib::Value` of type `vt`.
///
/// # Safety
/// `v` must satisfy the ABI contract.
pub unsafe fn to_value(vt: Type, v: &svl_value, env: &dyn Env) -> Result<glib::Value, String> {
    unsafe {
        Ok(if vt == Type::BOOL {
            (as_i64(v)? != 0).to_value()
        } else if vt == Type::I32 {
            range::<i32>(as_i64(v)?)?.to_value()
        } else if vt == Type::U32 {
            range::<u32>(as_i64(v)?)?.to_value()
        } else if vt == Type::I8 {
            range::<i8>(as_i64(v)?)?.to_value()
        } else if vt == Type::U8 {
            range::<u8>(as_i64(v)?)?.to_value()
        } else if vt == Type::I64 {
            as_i64(v)?.to_value()
        } else if vt == Type::U64 {
            if v.tag == SVL_I64 && v.flags & SVL_F_UNSIGNED != 0 {
                v.u.u.to_value()
            } else {
                range::<u64>(as_i64(v)?)?.to_value()
            }
        } else if vt == Type::F32 {
            (as_f64(v)? as f32).to_value()
        } else if vt == Type::F64 {
            as_f64(v)?.to_value()
        } else if vt == Type::STRING {
            match v.tag {
                SVL_NULL => None::<String>.to_value(),
                _ => as_str(v).ok_or("expected string")?.into_owned().to_value(),
            }
        } else if vt.is_a(Type::ENUM) {
            let class = glib::EnumClass::with_type(vt).ok_or("not an enum type")?;
            match v.tag {
                SVL_STR => class.to_value_by_nick(&as_str(v).ok_or("invalid utf-8")?),
                SVL_I64 | SVL_ENUM => class.to_value(range::<i32>(as_i64(v)?)?),
                t => return Err(format!("expected enum, got tag {t}")),
            }
            .ok_or_else(|| format!("invalid {} value", vt.name()))?
        } else if vt.is_a(Type::FLAGS) {
            let class = glib::FlagsClass::with_type(vt).ok_or("not a flags type")?;
            match v.tag {
                SVL_STR => {
                    let text = as_str(v).ok_or("invalid utf-8")?;
                    let mut mask = 0u32;
                    for nick in text.split_whitespace() {
                        let f = class
                            .values()
                            .iter()
                            .find(|f| f.nick() == nick)
                            .ok_or_else(|| format!("invalid {} value {nick:?}", vt.name()))?;
                        mask |= f.value();
                    }
                    Some(flags_value(vt, mask))
                }
                SVL_FLAGS => Some(flags_value(vt, v.u.u as u32)),
                SVL_I64 => class.to_value(range::<u32>(as_i64(v)?)?),
                t => return Err(format!("expected flags, got tag {t}")),
            }
            .ok_or_else(|| format!("invalid {} value", vt.name()))?
        } else if vt == strv() {
            match v.tag {
                SVL_NULL => Vec::<String>::new().to_value(),
                SVL_LIST => {
                    let mut out = Vec::new();
                    for e in items(v) {
                        out.push(as_str(e).ok_or("expected a list of strings")?.into_owned());
                    }
                    out.to_value()
                }
                _ => return Err("expected a list of strings".into()),
            }
        } else if vt.is_a(Type::OBJECT) || is_iface(vt) {
            match v.tag {
                SVL_NULL => glib::Value::from_type(vt),
                SVL_HANDLE => {
                    let o = env.object(v.u.h).ok_or("stale handle")?;
                    if !o.type_().is_a(vt) {
                        return Err(format!("{} is not a {}", o.type_().name(), vt.name()));
                    }
                    o.to_value()
                }
                SVL_BOXED => {
                    let b = env.boxed(v.u.h).ok_or("stale boxed handle")?;
                    if !b.type_().is_a(vt) {
                        return Err(format!("{} is not a {}", b.type_().name(), vt.name()));
                    }
                    b
                }
                SVL_STR => match from_string_form(vt, &as_str(v).ok_or("invalid utf-8")?) {
                    Some(r) => r?,
                    None => return Err(format!("{} has no string form", vt.name())),
                },
                _ => return Err("expected handle".into()),
            }
        } else if vt.is_a(Type::BOXED) {
            if let Some(rec) = Rec::of(vt) {
                match v.tag {
                    SVL_NULL => glib::Value::from_type(vt),
                    _ => rec_in(rec, v)?,
                }
            } else {
                match v.tag {
                    SVL_NULL => glib::Value::from_type(vt),
                    SVL_BOXED => {
                        let b = env.boxed(v.u.h).ok_or("stale boxed handle")?;
                        if !b.type_().is_a(vt) {
                            return Err(format!("{} is not a {}", b.type_().name(), vt.name()));
                        }
                        b
                    }
                    SVL_STR => match from_string_form(vt, &as_str(v).ok_or("invalid utf-8")?) {
                        Some(r) => r?,
                        None => return Err(format!("{} has no string form", vt.name())),
                    },
                    _ => return Err(format!("expected a {} value", vt.name())),
                }
            }
        } else {
            return Err(format!("unsupported property type {}", vt.name()));
        })
    }
}

// ---- outbound ----

/// Owns the bytes and lists handed out through `svl_value`s until the next ABI call.
#[derive(Default)]
pub struct StrBuf {
    strs: Vec<Box<[u8]>>,
    lists: Vec<Box<[svl_value]>>,
}

impl StrBuf {
    pub fn str_value(&mut self, s: &str) -> svl_value {
        let b: Box<[u8]> = s.as_bytes().into();
        let ptr = b.as_ptr() as *const std::ffi::c_void;
        let len = b.len();
        self.strs.push(b);
        svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr, len } })
    }

    pub fn list_value(&mut self, tag: u16, ty: svl_id, items: Vec<svl_value>) -> svl_value {
        let b = items.into_boxed_slice();
        let l = svl_list { ptr: b.as_ptr(), len: b.len() };
        self.lists.push(b);
        svl_value::typed(tag, ty, svl_value_u { l })
    }
}

pub fn null_value() -> svl_value {
    svl_value::null()
}

/// Converts a `glib::Value` to an ABI value; strings and lists are kept alive by `buf`.
pub fn from_value(v: &glib::Value, env: &dyn Env, buf: &mut StrBuf) -> svl_value {
    let t = v.type_();
    let int = |x: i64| svl_value::new(SVL_I64, svl_value_u { i: x });
    let float = |x: f64| svl_value::new(SVL_F64, svl_value_u { f: x });
    if t == Type::BOOL {
        return svl_value::new(SVL_BOOL, svl_value_u { b: v.get::<bool>().unwrap_or(false) });
    }
    if t == Type::STRING {
        return match v.get::<Option<String>>() {
            Ok(Some(s)) => buf.str_value(&s),
            _ => null_value(),
        };
    }
    if t.is_a(Type::ENUM) {
        let n = unsafe { glib::gobject_ffi::g_value_get_enum(v.to_glib_none().0) };
        return svl_value::typed(SVL_ENUM, env.type_id(t), svl_value_u { i: n as i64 });
    }
    if t.is_a(Type::FLAGS) {
        let n = unsafe { glib::gobject_ffi::g_value_get_flags(v.to_glib_none().0) };
        return svl_value::typed(SVL_FLAGS, env.type_id(t), svl_value_u { u: n as u64 });
    }
    if t == strv() {
        let strs = v.get::<Vec<String>>().unwrap_or_default();
        let items = strs.iter().map(|s| buf.str_value(s)).collect();
        return buf.list_value(SVL_LIST, 0, items);
    }
    if t.is_a(Type::OBJECT) || is_iface(t) {
        return match v.get::<Option<glib::Object>>() {
            Ok(Some(o)) => match env.handle_of(&o) {
                0 => {
                    let ty = env.type_id(o.type_());
                    let h = env.mint_boxed(o.to_value());
                    svl_value::typed(SVL_BOXED, ty, svl_value_u { h })
                }
                h => svl_value::new(SVL_HANDLE, svl_value_u { h }),
            },
            _ => null_value(),
        };
    }
    if t.is_a(Type::BOXED) {
        if let Some(rec) = Rec::of(t) {
            return rec_out(rec, v, env, buf).unwrap_or_else(null_value);
        }
        // `Value::get` of a null boxed pointer fails the Option check below.
        let is_null = unsafe { glib::gobject_ffi::g_value_get_boxed(v.to_glib_none().0).is_null() };
        if is_null {
            return null_value();
        }
        let ty = env.type_id(t);
        let h = env.mint_boxed(v.clone());
        return svl_value::typed(SVL_BOXED, ty, svl_value_u { h });
    }
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
        let u = v.get::<u64>().unwrap_or(0);
        let mut r = svl_value::new(SVL_I64, svl_value_u { u });
        r.flags = SVL_F_UNSIGNED;
        return r;
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

#[allow(dead_code)]
fn _keep(_: *const c_char) {
    let _ = types::FIRST_BACKEND_TYPE;
}
