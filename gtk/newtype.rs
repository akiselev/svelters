use glib::object::Object as GObject;
use glib::value::{FromValue, ToValueOptional, ValueType};
use glib::{Cast, IsA, ObjectType, StaticType, ToValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};
use rquickjs::{Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub struct G<T>(pub T);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gtk<T>(pub T);

impl<T> std::ops::Deref for Gtk<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Gtk<T> {
    pub fn new(val: T) -> Self {
        Gtk(val)
    }

    pub fn inner(self) -> T {
        self.0
    }
}

impl<T> AsRef<T> for Gtk<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for Gtk<T> {
    fn from(val: T) -> Self {
        Gtk(val)
    }
}

impl<T> ToValue for Gtk<T>
where
    T: ToValue,
{
    fn to_value(&self) -> glib::Value {
        self.0.to_value()
    }

    fn value_type(&self) -> glib::Type {
        self.0.value_type()
    }
}

impl<T> ToValueOptional for Gtk<T>
where
    T: ToValueOptional,
{
    fn to_value_optional(s: Option<&Self>) -> glib::Value {
        T::to_value_optional(s.map(|inner| &inner.0))
    }
}

impl<T> StaticType for Gtk<T>
where
    T: StaticType,
{
    fn static_type() -> glib::Type {
        T::static_type()
    }
}

impl<'js, T> IntoJs<'js> for Gtk<T>
where
    T: ObjectType + IsA<GObject>,
{
    fn into_js(self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        ctx.marshall(self.0)
    }
}

impl<'js, T> FromJs<'js> for Gtk<T>
where
    T: ObjectType,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let obj = crate::Object::from_js(ctx, value)?;
        let ty = obj.type_();
        match obj.inner().dynamic_cast::<T>() {
            Ok(target) => Ok(Gtk(target)),
            Err(_) => {
                let target_ty = T::static_type();
                Err(exception!("Failed to convert type {} to {}", ty, target_ty))
            }
        }
    }
}
