use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use glib::{value::FromValue, EnumClass, ToValue, Value as GValue};
use itertools::Itertools;
use rquickjs::{
    qjs, qjs::c_int, AsFunction, AsProperty, ClassDef, Ctx, FromJs, Function, IntoJs, Value,
};

use crate::{common::*, Object};

pub struct ObjectProp<CLS> {
    spec: ParamSpec,
    marker: PhantomData<CLS>,
}

impl<CLS> ObjectProp<CLS> {
    pub fn new(spec: ParamSpec) -> Self {
        Self {
            spec,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS> AsProperty<'js, ()> for ObjectProp<CLS>
where
    CLS: 'static + AsObject,
    &'js CLS: FromJs<'js>,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(c_int, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            (qjs::JS_PROP_HAS_GET | qjs::JS_PROP_HAS_SET | qjs::JS_PROP_HAS_ENUMERABLE) as c_int,
            Value::new_undefined(ctx),
            Function::new::<_, CLS, ()>(ctx, Getter::<CLS>::new(self.spec.clone())).into_js(ctx)?,
            Function::new::<_, CLS, ()>(ctx, Setter::<CLS>::new(self.spec)).into_js(ctx)?,
        ))
    }
}

struct Getter<CLS> {
    param: ParamSpec,
    marker: PhantomData<CLS>,
}

impl<CLS> Getter<CLS>
where
    CLS: AsObject,
{
    fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS> AsFunction<'js, CLS, ()> for Getter<CLS>
where
    CLS: 'static + AsObject,
    &'js CLS: FromJs<'js>,
{
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let acc = input.access();
        let ctx = acc.ctx();
        let this = acc.this::<&CLS>()?;
        let value = match this.as_object().property(self.param.name()) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err.message)),
        };
        let obj = match value.get::<GObject>() {
            Ok(value) => value,
            Err(err) => return err.handle_param_error(acc.ctx(), &self.param),
        };
        let ty = obj.type_();
        let converter = match ctx.get_typed(&TypeKey(ty)) {
            Some(converter) => converter,
            None => return Err(exception!("Type {} is not registered", ty.name())),
        };
        (converter.construct)(ctx, obj)
    }
}

struct Setter<CLS> {
    param: ParamSpec,
    marker: PhantomData<CLS>,
}

impl<CLS> Setter<CLS>
where
    CLS: AsObject,
{
    fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS> AsFunction<'js, CLS, ()> for Setter<CLS>
where
    CLS: 'static + AsObject,
    &'js CLS: FromJs<'js>,
{
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let mut acc = input.access();
        if acc.len() < 1 {
            return Err(exception!("Setter must have at least one argument"));
        }
        let this = acc.this::<&CLS>()?;
        let value = acc.arg::<Value<'js>>()?;
        let value = if value.is_null() || value.is_undefined() {
            self.param.default_value().clone()
        } else {
            let value = value.as_object().ok_or_else(|| {
                exception!(
                    "Excepted object type for {}::{}, found {}",
                    self.param.owner_type().name(),
                    self.param.name(),
                    value.type_name()
                )
            })?;
            let obj = match value.get::<_, Object>("asGObject") {
                Ok(val) => val,
                Err(err) => return Err(exception!("Not a GObject: {}", err)),
            };
            obj.to_value()
        };
        match this.as_object().set_property(self.param.name(), value) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not set property: {}", err.message)),
        };
        Ok(Value::new_undefined(acc.ctx()))
    }
}

pub struct Nick;
pub struct Name;
