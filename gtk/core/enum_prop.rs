use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use glib::{value::FromValue, EnumClass, EnumValue, ToValue, Value as GValue};
use itertools::Itertools;
use rquickjs::{
    qjs, qjs::c_int, AsFunction, AsProperty, ClassDef, Ctx, FromJs, Function, IntoJs, Value,
};

use crate::{common::*, Object};

pub struct EnumProp {
    spec: ParamSpec,
}

impl EnumProp {
    pub fn new(spec: ParamSpec) -> Self {
        Self { spec }
    }
}

impl<'js> AsProperty<'js, String> for EnumProp {
    fn config(self, ctx: Ctx<'js>) -> Result<(c_int, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            (qjs::JS_PROP_HAS_GET | qjs::JS_PROP_HAS_SET | qjs::JS_PROP_HAS_ENUMERABLE) as c_int,
            Value::new_undefined(ctx),
            Function::new::<_, (), ()>(ctx, Getter::new(self.spec.clone())).into_js(ctx)?,
            Function::new::<_, (), ()>(ctx, Setter::new(self.spec)).into_js(ctx)?,
        ))
    }
}
struct Getter {
    param: ParamSpec,
}

impl Getter {
    fn new(param: ParamSpec) -> Self {
        Self { param }
    }
}

impl<'js> AsFunction<'js, (), ()> for Getter {
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let acc = input.access();
        let this = acc.this::<&Object>()?;
        let _ = this.span().enter();
        self.param.log_getter();
        let value = match this.as_object().property(self.param.name()) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err.message)),
        };
        enum_nick_to_js(acc.ctx(), value, self.param.name())
    }
}

struct Setter {
    param: ParamSpec,
}

impl Setter {
    fn new(param: ParamSpec) -> Self {
        Self { param }
    }
}

impl<'js> AsFunction<'js, (), ()> for Setter {
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let mut acc = input.access();
        let this = acc.this::<&Object>()?;
        let _ = this.span().enter();
        self.param.log_setter();
        if acc.len() < 1 {
            return Err(exception!("Setter must have at least one argument"));
        }
        let value = enum_nick_from_js(
            acc.ctx(),
            acc.arg::<Value>()?,
            self.param.name(),
            get_enum_class(self.param.value_type())?,
        )?;
        match this.as_object().set_property(self.param.name(), value) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not set property: {}", err.message)),
        };
        Ok(Value::new_undefined(acc.ctx()))
    }
}

fn get_enum_class(ty: glib::types::Type) -> Result<EnumClass> {
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

fn enum_nick_to_js<'js>(ctx: Ctx<'js>, value: GValue, name: &'static str) -> Result<Value<'js>> {
    let value = EnumValue::from_value(&value).ok_or_else(|| {
        exception!(
            "Could not convert glib Value to EnumValue for enum {}",
            name
        )
    })?;
    value.nick().into_js(ctx)
}

fn enum_name_to_js<'js>(ctx: Ctx<'js>, value: GValue, name: &'static str) -> Result<Value<'js>> {
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
