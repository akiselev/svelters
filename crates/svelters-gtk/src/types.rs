//! Type ids and descriptors for the GTK backend.
//!
//! Ids below `SVL_T_FIRST_BACKEND` are the well-known value types; every other GType a descriptor
//! mentions gets the next id on first use. Descriptors are built from the GType system on demand
//! (enum and flags members come from the class), then cached by the vtable's [`DescStore`].

use crate::convert::{is_iface, strv, Rec};
use crate::lookup;
use gtk::glib::{self, Type};
use gtk::prelude::*;
use std::collections::HashMap;
use svelters_abi::*;

pub const FIRST_BACKEND_TYPE: u32 = SVL_T_FIRST_BACKEND;

#[derive(Default)]
pub struct TypeTable {
    ids: HashMap<Type, svl_id>,
    list: Vec<Type>,
}

impl TypeTable {
    pub fn id(&mut self, t: Type) -> svl_id {
        if let Some(w) = Rec::of(t).and_then(Rec::well_known) {
            return w;
        }
        if let Some(id) = self.ids.get(&t) {
            return *id;
        }
        let id = FIRST_BACKEND_TYPE + self.list.len() as u32;
        self.list.push(t);
        self.ids.insert(t, id);
        id
    }

    pub fn lookup(&self, id: svl_id) -> Option<Type> {
        if id >= FIRST_BACKEND_TYPE {
            return self.list.get((id - FIRST_BACKEND_TYPE) as usize).copied();
        }
        let rec = match id {
            SVL_T_RECT => Rec::RectF,
            SVL_T_SIZE => Rec::SizeF,
            SVL_T_POINT => Rec::PointF,
            SVL_T_INSETS => Rec::Insets,
            SVL_T_COLOR => Rec::Color,
            _ => return None,
        };
        Some(rec_gtype(rec))
    }
}

fn rec_gtype(r: Rec) -> Type {
    match r {
        Rec::RectF => gtk::graphene::Rect::static_type(),
        Rec::RectI => gtk::gdk::Rectangle::static_type(),
        Rec::SizeF => gtk::graphene::Size::static_type(),
        Rec::PointF => gtk::graphene::Point::static_type(),
        Rec::Insets => gtk::Border::static_type(),
        Rec::Color => gtk::gdk::RGBA::static_type(),
    }
}

fn has_string_form(t: Type) -> bool {
    matches!(t.name(), "PangoFontDescription" | "GIcon" | "GdkRGBA" | "GtkShortcutTrigger" | "GFile")
}

fn bounds(ps: Option<&glib::ParamSpec>, t: Type) -> (f64, f64) {
    macro_rules! mm {
        ($ps:expr, $ty:ty) => {
            if let Some(p) = $ps.and_then(|p| p.downcast_ref::<$ty>()) {
                return (p.minimum() as f64, p.maximum() as f64);
            }
        };
    }
    mm!(ps, glib::ParamSpecInt);
    mm!(ps, glib::ParamSpecUInt);
    mm!(ps, glib::ParamSpecChar);
    mm!(ps, glib::ParamSpecUChar);
    mm!(ps, glib::ParamSpecInt64);
    mm!(ps, glib::ParamSpecFloat);
    mm!(ps, glib::ParamSpecDouble);
    if t == Type::I32 {
        (i32::MIN as f64, i32::MAX as f64)
    } else if t == Type::U32 {
        (0.0, u32::MAX as f64)
    } else if t == Type::I8 {
        (i8::MIN as f64, i8::MAX as f64)
    } else if t == Type::U8 {
        (0.0, u8::MAX as f64)
    } else if t == Type::U64 {
        (0.0, f64::NAN)
    } else {
        (f64::NAN, f64::NAN)
    }
}

/// The descriptor slot for a value of GType `t` (with its `GParamSpec` when it is a property).
pub fn slot_for(table: &mut TypeTable, t: Type, ps: Option<&glib::ParamSpec>) -> SlotDesc {
    let (min, max) = bounds(ps, t);
    let finite = |x: f64| if x.is_finite() && x.abs() < f64::MAX / 2.0 { x } else { f64::NAN };
    let mut s = if t == Type::BOOL {
        SlotDesc::new(SVL_BOOL)
    } else if t == Type::I32 || t == Type::U32 || t == Type::I8 || t == Type::U8 || t == Type::I64 {
        SlotDesc::new(SVL_I64).range(min, max)
    } else if t == Type::U64 {
        SlotDesc::new(SVL_I64).flags(SVL_D_UNSIGNED).range(0.0, f64::NAN)
    } else if t == Type::F32 || t == Type::F64 {
        SlotDesc::new(SVL_F64).range(finite(min), finite(max))
    } else if t == Type::STRING {
        SlotDesc::new(SVL_STR).flags(SVL_D_NULLABLE)
    } else if t.is_a(Type::ENUM) {
        SlotDesc::new(SVL_ENUM).typed(table.id(t))
    } else if t.is_a(Type::FLAGS) {
        SlotDesc::new(SVL_FLAGS).typed(table.id(t))
    } else if t == crate::convert::gtype_gtype() {
        SlotDesc::new(SVL_STR).typed(SVL_T_TYPE).flags(SVL_D_NULLABLE)
    } else if t == Type::VARIANT || t == crate::convert::gvalue_type() {
        SlotDesc::new(SVL_NULL).typed(SVL_T_VARIANT).flags(SVL_D_NULLABLE)
    } else if t == glib::DateTime::static_type() {
        SlotDesc::new(SVL_F64).typed(SVL_T_DATETIME).flags(SVL_D_NULLABLE)
    } else if t == strv() {
        SlotDesc::new(SVL_LIST).typed(table.id(t)).flags(SVL_D_NULLABLE)
    } else if t.is_a(Type::OBJECT) || is_iface(t) {
        // Widgets and objects with a kind travel as handles (ours owned, the rest borrowed);
        // other objects (icons, layouts) are opaque BOXED values.
        let mut s = SlotDesc::new(SVL_HANDLE).typed(lookup::nearest_kind(t)).flags(SVL_D_NULLABLE);
        if has_string_form(t) {
            s = s.flags(SVL_D_STRING_FORM);
        }
        s
    } else if let Some(rec) = Rec::of(t) {
        let mut s = SlotDesc::new(SVL_RECORD).typed(table.id(t)).flags(SVL_D_NULLABLE);
        if rec == Rec::Color {
            s = s.flags(SVL_D_STRING_FORM);
        }
        s
    } else if t.is_a(Type::BOXED) {
        let mut s = SlotDesc::new(SVL_BOXED).typed(table.id(t)).flags(SVL_D_NULLABLE);
        if has_string_form(t) {
            s = s.flags(SVL_D_STRING_FORM);
        }
        s
    } else {
        SlotDesc::new(SVL_NULL)
    };
    if s.tag == SVL_NULL {
        s.flags |= SVL_D_NULLABLE;
    }
    s
}

/// The type descriptor of GType `t` (enum, flags, record, list or opaque boxed/object value).
pub fn type_desc(table: &mut TypeTable, t: Type) -> TypeDesc {
    let none = SlotDesc::new(SVL_NULL);
    if t.is_a(Type::ENUM) {
        let members = glib::EnumClass::with_type(t)
            .map(|c| c.values().iter().map(|v| EnumMember { name: v.nick().to_string(), value: v.value() as i64 }).collect())
            .unwrap_or_default();
        return TypeDesc { name: t.name().to_string(), kind: SVL_ENUM, scope: 0, members, fields: vec![], ret: none };
    }
    if t.is_a(Type::FLAGS) {
        let members = glib::FlagsClass::with_type(t)
            .map(|c| c.values().iter().map(|v| EnumMember { name: v.nick().to_string(), value: v.value() as i64 }).collect())
            .unwrap_or_default();
        return TypeDesc { name: t.name().to_string(), kind: SVL_FLAGS, scope: 0, members, fields: vec![], ret: none };
    }
    if t == strv() {
        let elem = SlotDesc::new(SVL_STR);
        return TypeDesc { name: "string[]".into(), kind: SVL_LIST, scope: 0, members: vec![], fields: vec![elem], ret: none };
    }
    if let Some(rec) = Rec::of(t) {
        let fields = rec
            .fields()
            .iter()
            .map(|n| {
                let s = if rec.integral() {
                    let (lo, hi) = if rec == Rec::Insets { (i16::MIN as f64, i16::MAX as f64) } else { (i32::MIN as f64, i32::MAX as f64) };
                    SlotDesc::new(SVL_I64).range(lo, hi)
                } else if rec == Rec::Color {
                    SlotDesc::new(SVL_F64).range(0.0, 1.0)
                } else {
                    SlotDesc::new(SVL_F64)
                };
                s.named(n)
            })
            .collect();
        let _ = table;
        return TypeDesc { name: rec.type_name().to_string(), kind: SVL_RECORD, scope: 0, members: vec![], fields, ret: none };
    }
    TypeDesc { name: t.name().to_string(), kind: SVL_BOXED, scope: 0, members: vec![], fields: vec![], ret: none }
}
