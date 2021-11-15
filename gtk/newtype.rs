use std::marker::PhantomData;

use glib::object::Object as GObject;
use glib::value::{
    FromValue, FromValueOptional, ToValueOptional, ValueType, ValueTypeChecker,
    ValueTypeMismatchOrNoneError,
};
use glib::{Cast, IsA, ObjectType, StaticType, ToValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};
use rquickjs::{Ctx, FromJs, IntoJs, Opt, Value};

use crate::common::*;
use crate::registry::Registry;
use crate::traits::*;

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

impl<T, U> CastRef<U> for Gtk<T>
where
    U: ObjectType,
    T: Cast,
{
    fn cast_ref(&self) -> Option<&U> {
        self.0.dynamic_cast_ref::<U>()
    }
}

impl<T> From<T> for Gtk<T> {
    fn from(val: T) -> Self {
        Gtk(val)
    }
}

unsafe impl<'a, T> FromValue<'a> for Gtk<T>
where
    T: FromValue<'a>,
{
    type Checker = <T as FromValue<'a>>::Checker;

    unsafe fn from_value(value: &'a glib::Value) -> Self {
        Gtk(T::from_value(value))
    }
}

impl<'a, T> ValueType for Gtk<T>
where
    T: ValueType,
{
    type Type = <T as ValueType>::Type;
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

impl<T: IsA<GObject>> AsObject for Gtk<T> {
    fn as_object(&self) -> &GObject {
        &self.0.upcast_ref::<GObject>()
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
        if let Some(target) = obj.inner().dynamic_cast::<T>().ok() {
            Ok(Gtk(target))
        } else {
            let target_ty = T::static_type();
            Err(exception!("Failed to convert type {} to {}", ty, target_ty))
        }
    }
}
