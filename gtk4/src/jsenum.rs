use crate::inner::*;

use glib::{
    types::Type, value::FromValue, EnumClass, EnumValue, IsA, StaticType, ToValue, Value as GValue,
};
use itertools::Itertools;

pub struct Enum<T, N = Nick>(pub T, PhantomData<N>);

impl<T, N> Enum<T, N> {
    pub fn into(self) -> T {
        self.0
    }
}

impl<T, N> From<T> for Enum<T, N> {
    fn from(val: T) -> Self {
        Enum(val, PhantomData)
    }
}

impl<T, N> std::ops::Deref for Enum<T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, T> FromJs<'js> for Enum<T, Nick>
where
    T: StaticType + for<'a> FromValue<'a>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let name = T::static_type().name();
        let cls = get_enum_class(T::static_type())?;
        let val = enum_nick_from_js(ctx, value, name, cls)?;
        val.get::<T>()
            .map(|res| Enum(res, PhantomData))
            .map_err(|err| exception!("Error parsing enum {}: {}", name, err))
    }
}

impl<'js, T> IntoJs<'js> for Enum<T, Nick>
where
    T: StaticType + ToValue,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let name = T::static_type().name();
        let cls = get_enum_class(T::static_type())?;
        let value = self.to_value();
        enum_nick_to_js(ctx, value, name, cls)
    }
}

impl<'js, T> FromJs<'js> for Enum<T, Name>
where
    T: StaticType + for<'a> FromValue<'a>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let name = T::static_type().name();
        let cls = get_enum_class(T::static_type())?;
        let val = enum_name_from_js(ctx, value, name, cls)?;
        val.get::<T>()
            .map(|res| Enum(res, PhantomData))
            .map_err(|err| exception!("Error parsing enum {}: {}", name, err))
    }
}

impl<'js, T> IntoJs<'js> for Enum<T, Name>
where
    T: StaticType + ToValue,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let name = T::static_type().name();
        let cls = get_enum_class(T::static_type())?;
        let value = self.to_value();
        enum_name_to_js(ctx, value, name, cls)
    }
}

fn get_enum_class(ty: Type) -> Result<EnumClass> {
    EnumClass::new(ty).ok_or_else(|| exception!("{} is not an enum", ty.name()))
}

fn enum_nick_from_js<'js>(
    ctx: Ctx<'js>,
    value: rquickjs::Value<'js>,
    name: &'static str,
    cls: EnumClass,
) -> Result<GValue> {
    if !value.is_string() {
        return Err(exception!("Enum {} must be a string", name));
    }
    let value = String::from_js(ctx, value)?;
    if let Some(val) = cls.to_value_by_nick(value.as_str()) {
        Ok(val)
    } else {
        Err(exception!(
            "\"{}\" is not a valid value for enum {} (\"{}\")",
            value,
            name,
            cls.values().iter().map(|val| val.nick()).join("\", \"")
        ))
    }
}

fn enum_name_from_js<'js>(
    ctx: Ctx<'js>,
    value: rquickjs::Value<'js>,
    name: &'static str,
    cls: EnumClass,
) -> Result<GValue> {
    if !value.is_string() {
        return Err(exception!("Enum {} must be a string", name));
    }
    let value = String::from_js(ctx, value)?;
    if let Some(val) = cls.to_value_by_name(value.as_str()) {
        Ok(val)
    } else {
        Err(exception!(
            "\"{}\" is not a valid for enum {} (\"{}\")",
            value,
            name,
            cls.values().iter().map(|val| val.name()).join("\", \"")
        ))
    }
}

fn enum_nick_to_js<'js>(
    ctx: Ctx<'js>,
    value: GValue,
    name: &'static str,
    cls: EnumClass,
) -> Result<Value<'js>> {
    let value = EnumValue::from_value(&value).ok_or_else(|| {
        exception!(
            "Could not convert glib Value to EnumValue for enum {}",
            name
        )
    })?;
    value.nick().into_js(ctx)
}

fn enum_name_to_js<'js>(
    ctx: Ctx<'js>,
    value: GValue,
    name: &'static str,
    cls: EnumClass,
) -> Result<Value<'js>> {
    let value = EnumValue::from_value(&value).ok_or_else(|| {
        exception!(
            "Could not convert glib Value to EnumValue for enum {}",
            name
        )
    })?;
    value.name().into_js(ctx)
}

pub struct Nick;
pub struct Name;
