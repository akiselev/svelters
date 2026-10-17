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

/// Whether the value behind a `BOXED` handle is usable where GType `vt` is expected. Objects are
/// held as `glib::Value`s of the generic object type, so the runtime class is what counts.
fn boxed_is_a(b: &glib::Value, vt: Type) -> bool {
    if b.type_().is_a(vt) {
        return true;
    }
    b.get::<Option<glib::Object>>().ok().flatten().is_some_and(|o| o.type_().is_a(vt))
}

fn boxed_type_name(b: &glib::Value) -> String {
    match b.get::<Option<glib::Object>>().ok().flatten() {
        Some(o) => o.type_().name().to_string(),
        None => b.type_().name().to_string(),
    }
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
        // Compare GTypes, not names: graphene registers `GrapheneRect` where the C library says
        // `graphene_rect_t`.
        [Rec::RectF, Rec::RectI, Rec::SizeF, Rec::PointF, Rec::Insets, Rec::Color]
            .into_iter()
            .find(|r| r.gtype() == t)
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

/// A `GVariant` from a JSON-like value: bool `b`, integer `x` (`t` when unsigned), float `d`, string
/// `s`, bytes `ay`, list `av`, map `a{sv}`. The variant type of a value is inferred, not declared:
/// a property that needs an exact type (`GtkActionable:action-target`) takes what these map to.
fn variant_in(v: &svl_value) -> Result<glib::Variant, String> {
    use glib::prelude::ToVariant;
    Ok(match v.tag {
        SVL_BOOL => unsafe { v.u.b }.to_variant(),
        SVL_I64 if v.flags & SVL_F_UNSIGNED != 0 => unsafe { v.u.u }.to_variant(),
        SVL_I64 => unsafe { v.u.i }.to_variant(),
        SVL_F64 => unsafe { v.u.f }.to_variant(),
        SVL_STR => unsafe { as_str(v) }.ok_or("invalid utf-8")?.as_ref().to_variant(),
        SVL_BYTES => {
            let b = unsafe { v.u.bytes };
            let data: &[u8] = if b.ptr.is_null() || b.len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(b.ptr, b.len) } };
            glib::Variant::array_from_fixed_array(data)
        }
        SVL_LIST => {
            let items: Vec<glib::Variant> =
                items(v).iter().map(|e| variant_in(e).map(|x| glib::Variant::from_variant(&x))).collect::<Result<_, _>>()?;
            glib::Variant::array_from_iter_with_type(glib::VariantTy::VARIANT, items)
        }
        SVL_MAP => {
            let dict = glib::VariantDict::new(None);
            for kv in items(v).chunks(2) {
                let [k, val] = kv else { return Err("malformed map".into()) };
                let key = unsafe { as_str(k) }.ok_or("map keys must be strings")?;
                dict.insert_value(&key, &variant_in(val)?);
            }
            dict.end()
        }
        SVL_RECORD if v.type_ == SVL_T_TYPED_VARIANT => {
            let [sig, val] = items(v) else { return Err("malformed typed variant".into()) };
            let sig = unsafe { as_str(sig) }.ok_or("the variant signature must be a string")?;
            let ty = glib::VariantType::new(&sig).map_err(|e| format!("invalid GVariant type {sig:?}: {e}"))?;
            typed_variant(&ty, val).map_err(|e| format!("variant({sig:?}): {e}"))?
        }
        SVL_NULL => return Err("null cannot be nested in a GVariant".into()),
        t => return Err(format!("cannot convert tag {t} to a GVariant")),
    })
}

/// A `GVariant` of exactly the type `ty` from a JSON-like value (`variant(signature, value)` in JS):
/// basic types from numbers / strings / booleans, `aX` from lists, `a{kV}` from objects, `(..)` from
/// lists, `mX` from null or a value, `v` by inference, `ay` from bytes.
fn typed_variant(ty: &glib::VariantTy, v: &svl_value) -> Result<glib::Variant, String> {
    use glib::prelude::ToVariant;
    use std::convert::TryFrom;
    if v.tag == SVL_RECORD && v.type_ == SVL_T_TYPED_VARIANT && ty.as_str() != "v" {
        let inner = variant_in(v)?;
        return if inner.type_() == ty { Ok(inner) } else { Err(format!("a variant of type {} is not {}", inner.type_(), ty)) };
    }
    let integer = || -> Result<i128, String> {
        match v.tag {
            SVL_I64 if v.flags & SVL_F_UNSIGNED != 0 => Ok(unsafe { v.u.u } as i128),
            SVL_I64 => Ok(unsafe { v.u.i } as i128),
            SVL_F64 if unsafe { v.u.f }.fract() == 0.0 => Ok(unsafe { v.u.f } as i128),
            t => Err(format!("expected an integer for type {ty}, got tag {t}")),
        }
    };
    fn fit<T: TryFrom<i128>>(n: i128, ty: &glib::VariantTy) -> Result<T, String> {
        T::try_from(n).map_err(|_| format!("{n} does not fit type {ty}"))
    }
    let s = ty.as_str();
    Ok(match s {
        "b" if v.tag == SVL_BOOL => unsafe { v.u.b }.to_variant(),
        "y" => fit::<u8>(integer()?, ty)?.to_variant(),
        "n" => fit::<i16>(integer()?, ty)?.to_variant(),
        "q" => fit::<u16>(integer()?, ty)?.to_variant(),
        "i" => fit::<i32>(integer()?, ty)?.to_variant(),
        "u" => fit::<u32>(integer()?, ty)?.to_variant(),
        "x" => fit::<i64>(integer()?, ty)?.to_variant(),
        "t" => fit::<u64>(integer()?, ty)?.to_variant(),
        "d" => match v.tag {
            SVL_F64 => unsafe { v.u.f }.to_variant(),
            SVL_I64 => (integer()? as f64).to_variant(),
            t => return Err(format!("expected a number for type d, got tag {t}")),
        },
        "s" if v.tag == SVL_STR => unsafe { as_str(v) }.ok_or("invalid utf-8")?.as_ref().to_variant(),
        "o" if v.tag == SVL_STR => {
            let text = unsafe { as_str(v) }.ok_or("invalid utf-8")?.into_owned();
            glib::variant::ObjectPath::try_from(text).map_err(|e| e.to_string())?.to_variant()
        }
        "g" if v.tag == SVL_STR => {
            let text = unsafe { as_str(v) }.ok_or("invalid utf-8")?.into_owned();
            glib::variant::Signature::try_from(text).map_err(|e| e.to_string())?.to_variant()
        }
        "v" => glib::Variant::from_variant(&variant_in(v)?),
        _ if ty.is_maybe() => match v.tag {
            SVL_NULL => glib::Variant::from_none(ty.element()),
            _ => glib::Variant::from_some(&typed_variant(ty.element(), v)?),
        },
        _ if ty.is_array() => {
            let elem = ty.element();
            match v.tag {
                SVL_BYTES if elem.as_str() == "y" => {
                    let b = unsafe { v.u.bytes };
                    let data: &[u8] = if b.ptr.is_null() || b.len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(b.ptr, b.len) } };
                    glib::Variant::array_from_fixed_array(data)
                }
                SVL_MAP if elem.is_dict_entry() => {
                    let (kt, vt) = (elem.key(), elem.value());
                    if !matches!(kt.as_str(), "s" | "o" | "g") {
                        return Err(format!("an object becomes a dictionary only with string keys, not {kt}"));
                    }
                    let mut entries = Vec::new();
                    for kv in items(v).chunks(2) {
                        let [k, val] = kv else { return Err("malformed map".into()) };
                        let key = typed_variant(kt, k)?;
                        entries.push(glib::Variant::from_dict_entry(&key, &typed_variant(vt, val).map_err(|e| format!(".{}: {e}", unsafe { as_str(k) }.unwrap_or_default()))?));
                    }
                    glib::Variant::array_from_iter_with_type(elem, entries)
                }
                SVL_LIST => {
                    let mut out = Vec::new();
                    for (i, e) in items(v).iter().enumerate() {
                        out.push(typed_variant(elem, e).map_err(|m| format!("[{i}]: {m}"))?);
                    }
                    glib::Variant::array_from_iter_with_type(elem, out)
                }
                t => return Err(format!("expected a list for type {ty}, got tag {t}")),
            }
        }
        _ if ty.is_tuple() => {
            let types: Vec<&glib::VariantTy> = ty.tuple_types().collect();
            if v.tag != SVL_LIST || items(v).len() != types.len() {
                return Err(format!("expected a list of {} item(s) for type {ty}", types.len()));
            }
            let mut out = Vec::new();
            for (i, (t, e)) in types.iter().zip(items(v)).enumerate() {
                out.push(typed_variant(t, e).map_err(|m| format!("[{i}]: {m}"))?);
            }
            glib::Variant::tuple_from_iter(out)
        }
        _ if ty.is_dict_entry() => {
            let [k, val] = items(v) else { return Err(format!("expected a [key, value] list for type {ty}")) };
            glib::Variant::from_dict_entry(&typed_variant(ty.key(), k)?, &typed_variant(ty.value(), val)?)
        }
        _ => return Err(format!("cannot build type {ty} from tag {}", v.tag)),
    })
}

/// The boxed type `G_TYPE_VALUE`: a property that holds an arbitrary `GValue`.
pub fn gvalue_type() -> Type {
    // SAFETY: g_value_get_type has no preconditions.
    unsafe { glib::translate::from_glib(glib::gobject_ffi::g_value_get_type()) }
}

/// A `GValue` (the boxed `G_TYPE_VALUE`, e.g. `GtkDropTarget:value`) from a dynamic JS value: the type
/// is inferred (bool, int64, double, string, bytes -> `GBytes`, a boxed handle or object handle -> itself).
fn gvalue_in(v: &svl_value, env: &dyn Env) -> Result<glib::Value, String> {
    Ok(match v.tag {
        SVL_BOOL => unsafe { v.u.b }.to_value(),
        SVL_I64 if v.flags & SVL_F_UNSIGNED != 0 => unsafe { v.u.u }.to_value(),
        SVL_I64 => unsafe { v.u.i }.to_value(),
        SVL_F64 => unsafe { v.u.f }.to_value(),
        SVL_STR => unsafe { as_str(v) }.ok_or("invalid utf-8")?.into_owned().to_value(),
        SVL_BYTES => {
            let b = unsafe { v.u.bytes };
            let data: &[u8] = if b.ptr.is_null() || b.len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(b.ptr, b.len) } };
            glib::Bytes::from(data).to_value()
        }
        SVL_BOXED => env.boxed(unsafe { v.u.h }).ok_or("stale boxed handle")?,
        SVL_HANDLE => env.object(unsafe { v.u.h }).ok_or("stale handle")?.to_value(),
        SVL_RECORD if v.type_ == SVL_T_TYPED_VARIANT => variant_in(v)?.to_value(),
        t => return Err(format!("cannot convert tag {t} to a GValue")),
    })
}

fn variant_out(var: &glib::Variant, buf: &mut StrBuf) -> svl_value {
    use glib::VariantClass as K;
    let int = |x: i64| svl_value::new(SVL_I64, svl_value_u { i: x });
    match var.classify() {
        K::Boolean => svl_value::new(SVL_BOOL, svl_value_u { b: var.get::<bool>().unwrap_or(false) }),
        K::Byte => int(var.get::<u8>().unwrap_or(0) as i64),
        K::Int16 => int(var.get::<i16>().unwrap_or(0) as i64),
        K::Uint16 => int(var.get::<u16>().unwrap_or(0) as i64),
        K::Int32 => int(var.get::<i32>().unwrap_or(0) as i64),
        K::Handle => int(var.get::<glib::variant::Handle>().map(|h| h.0 as i64).unwrap_or(0)),
        K::Uint32 => int(var.get::<u32>().unwrap_or(0) as i64),
        K::Int64 => int(var.get::<i64>().unwrap_or(0)),
        K::Uint64 => {
            let mut r = svl_value::new(SVL_I64, svl_value_u { u: var.get::<u64>().unwrap_or(0) });
            r.flags = SVL_F_UNSIGNED;
            r
        }
        K::Double => svl_value::new(SVL_F64, svl_value_u { f: var.get::<f64>().unwrap_or(0.0) }),
        K::String | K::ObjectPath | K::Signature => buf.str_value(var.str().unwrap_or("")),
        K::Variant => variant_out(&var.as_variant().unwrap_or_else(|| ().to_variant()), buf),
        K::Maybe => match var.as_maybe() {
            Some(inner) => variant_out(&inner, buf),
            None => null_value(),
        },
        K::Array | K::Tuple | K::DictEntry => {
            let ty = var.type_();
            if var.classify() == K::Array {
                let elem = ty.element();
                if elem.as_str() == "y" {
                    return buf.bytes_value(var.fixed_array::<u8>().map(|s| s.to_vec()).unwrap_or_default());
                }
                // An array of string-keyed dict entries is a map.
                if elem.as_str().starts_with("{s") {
                    let mut flat = Vec::new();
                    for i in 0..var.n_children() {
                        let e = var.child_value(i);
                        flat.push(variant_out(&e.child_value(0), buf));
                        flat.push(variant_out(&e.child_value(1), buf));
                    }
                    return buf.list_value(SVL_MAP, 0, flat);
                }
            }
            let kids: Vec<svl_value> = (0..var.n_children()).map(|i| variant_out(&var.child_value(i), buf)).collect();
            buf.list_value(SVL_LIST, 0, kids)
        }
        _ => null_value(),
    }
}

fn datetime_in(ms: f64) -> Result<glib::DateTime, String> {
    let secs = (ms / 1000.0).floor();
    let rest = ms - secs * 1000.0;
    glib::DateTime::from_unix_utc(secs as i64)
        .and_then(|d| d.add_seconds(rest / 1000.0))
        .and_then(|d| d.to_local())
        .map_err(|e| e.to_string())
}

/// String forms: the canonical text of boxed or interface types that have one.
fn from_string_form(t: Type, s: &str) -> Option<Result<glib::Value, String>> {
    match t.name() {
        "GtkShortcutTrigger" => Some(
            gtk::ShortcutTrigger::parse_string(s)
                .map(|x| x.upcast::<glib::Object>().to_value())
                .ok_or_else(|| format!("cannot parse shortcut {s:?}")),
        ),
        // A URI when it has a scheme, else a path.
        "GFile" => Some(Ok(if s.contains("://") { gtk::gio::File::for_uri(s) } else { gtk::gio::File::for_path(s) }
            .upcast::<glib::Object>()
            .to_value())),
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
        } else if vt == Type::VARIANT {
            match v.tag {
                SVL_NULL => None::<glib::Variant>.to_value(),
                _ => Some(variant_in(v)?).to_value(),
            }
        } else if vt == gvalue_type() {
            let mut out = glib::Value::from_type(vt);
            if v.tag != SVL_NULL {
                let inner = gvalue_in(v, env)?;
                // SAFETY: `out` holds a G_TYPE_VALUE; set_boxed copies `inner`.
                unsafe { glib::gobject_ffi::g_value_set_boxed(out.to_glib_none_mut().0, inner.to_glib_none().0 as *const _) };
            }
            out
        } else if vt == glib::DateTime::static_type() {
            match v.tag {
                SVL_NULL => glib::Value::from_type(vt),
                SVL_F64 | SVL_I64 => datetime_in(as_f64(v)?)?.to_value(),
                _ => return Err("expected a Date".into()),
            }
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
                    if !boxed_is_a(&b, vt) {
                        return Err(format!("{} is not a {}", boxed_type_name(&b), vt.name()));
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
                        if !boxed_is_a(&b, vt) {
                            return Err(format!("{} is not a {}", boxed_type_name(&b), vt.name()));
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
    /// A `BYTES` value over a copy owned by this buffer.
    pub fn bytes_value(&mut self, data: Vec<u8>) -> svl_value {
        let b = data.into_boxed_slice();
        let (ptr, len) = (b.as_ptr(), b.len());
        self.strs.push(b);
        svl_value::new(SVL_BYTES, svl_value_u { bytes: svl_bytes { ptr, len } })
    }
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
    if t == Type::VARIANT {
        return match v.get::<Option<glib::Variant>>() {
            Ok(Some(var)) => variant_out(&var, buf),
            _ => null_value(),
        };
    }
    if t == gvalue_type() {
        // SAFETY: a G_TYPE_VALUE holds a borrowed `GValue *` (NULL when unset); from_glib_none copies it.
        let p = unsafe { glib::gobject_ffi::g_value_get_boxed(v.to_glib_none().0) } as *const glib::gobject_ffi::GValue;
        if p.is_null() {
            return null_value();
        }
        let inner: glib::Value = unsafe { glib::translate::from_glib_none(p) };
        return from_value(&inner, env, buf);
    }
    if t == glib::DateTime::static_type() {
        return match v.get::<Option<glib::DateTime>>() {
            Ok(Some(d)) => {
                let ms = d.to_unix() as f64 * 1000.0 + (d.microsecond() / 1000) as f64;
                svl_value::typed(SVL_F64, SVL_T_DATETIME, svl_value_u { f: ms })
            }
            _ => null_value(),
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::on_gtk;

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
        fn type_id(&self, t: Type) -> svl_id {
            Rec::of(t).and_then(Rec::well_known).unwrap_or(SVL_T_FIRST_BACKEND)
        }
    }

    fn record(ty: svl_id, fields: Vec<Value>) -> Value {
        Value::Record { ty, fields }
    }

    /// Converts `v` to a GValue of `t` and back; returns the GValue and the value that comes out.
    fn round_trip(t: Type, v: &Value) -> (glib::Value, Value) {
        let gv = v.with_raw(|raw| unsafe { to_value(t, raw, &NoEnv) }).expect("to_value");
        let mut buf = StrBuf::default();
        let out = from_value(&gv, &NoEnv, &mut buf);
        (gv, unsafe { Value::from_raw(&out) })
    }

    #[test]
    fn graphene_and_border_records_round_trip() {
        on_gtk(|| {
            let (gv, back) = round_trip(
                gtk::graphene::Rect::static_type(),
                &record(SVL_T_RECT, vec![Value::F64(1.5), Value::F64(2.5), Value::F64(30.0), Value::F64(40.0)]),
            );
            let r = gv.get::<gtk::graphene::Rect>().unwrap();
            assert_eq!((r.x(), r.y(), r.width(), r.height()), (1.5, 2.5, 30.0, 40.0));
            assert_eq!(back, record(SVL_T_RECT, vec![Value::F64(1.5), Value::F64(2.5), Value::F64(30.0), Value::F64(40.0)]));

            let (gv, back) = round_trip(
                gtk::graphene::Size::static_type(),
                &record(SVL_T_SIZE, vec![Value::F64(8.0), Value::I64(9)]),
            );
            let s = gv.get::<gtk::graphene::Size>().unwrap();
            assert_eq!((s.width(), s.height()), (8.0, 9.0));
            assert_eq!(back, record(SVL_T_SIZE, vec![Value::F64(8.0), Value::F64(9.0)]));

            let (gv, back) = round_trip(
                gtk::graphene::Point::static_type(),
                &record(SVL_T_POINT, vec![Value::F64(-3.0), Value::F64(4.5)]),
            );
            let p = gv.get::<gtk::graphene::Point>().unwrap();
            assert_eq!((p.x(), p.y()), (-3.0, 4.5));
            assert_eq!(back, record(SVL_T_POINT, vec![Value::F64(-3.0), Value::F64(4.5)]));

            // Insets: top, right, bottom, left, integral and i16-ranged.
            let (gv, back) = round_trip(
                gtk::Border::static_type(),
                &record(SVL_T_INSETS, vec![Value::I64(1), Value::I64(2), Value::I64(3), Value::I64(4)]),
            );
            let b = gv.get::<gtk::Border>().unwrap();
            assert_eq!((b.top(), b.right(), b.bottom(), b.left()), (1, 2, 3, 4));
            assert_eq!(back, record(SVL_T_INSETS, vec![Value::I64(1), Value::I64(2), Value::I64(3), Value::I64(4)]));
            let too_big = record(SVL_T_INSETS, vec![Value::I64(100_000), Value::I64(0), Value::I64(0), Value::I64(0)]);
            assert!(too_big.with_raw(|raw| unsafe { to_value(gtk::Border::static_type(), raw, &NoEnv) }).is_err());
        });
    }

    #[test]
    fn gvalue_properties_hold_an_inferred_value() {
        on_gtk(|| {
            for (v, check) in [
                (Value::Bool(true), "gboolean"),
                (Value::I64(-5), "gint64"),
                (Value::F64(0.5), "gdouble"),
                (Value::Str("hi".into()), "gchararray"),
            ] {
                let (gv, back) = round_trip(gvalue_type(), &v);
                assert_eq!(gv.type_(), gvalue_type());
                assert_eq!(back, v);
                let p = unsafe { glib::gobject_ffi::g_value_get_boxed(gv.to_glib_none().0) } as *const glib::gobject_ffi::GValue;
                let inner: glib::Value = unsafe { glib::translate::from_glib_none(p) };
                assert_eq!(inner.type_().name(), check);
            }
            // Unset reads as null; null writes an unset value.
            let (gv, back) = round_trip(gvalue_type(), &Value::Null);
            assert_eq!(back, Value::Null);
            assert!(unsafe { glib::gobject_ffi::g_value_get_boxed(gv.to_glib_none().0) }.is_null());
            // A typed variant is a GVariant inside the GValue.
            let typed = Value::Record { ty: SVL_T_TYPED_VARIANT, fields: vec![Value::Str("u".into()), Value::I64(7)] };
            let (_, back) = round_trip(gvalue_type(), &typed);
            assert_eq!(back, Value::I64(7));
        });
    }

    #[test]
    fn null_boxed_record_property_is_null() {
        on_gtk(|| {
            let gv = glib::Value::from_type(gtk::gdk::RGBA::static_type());
            let mut buf = StrBuf::default();
            let out = from_value(&gv, &NoEnv, &mut buf);
            assert_eq!(out.tag, SVL_NULL);
        });
    }
}
