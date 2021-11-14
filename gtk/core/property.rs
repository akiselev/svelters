use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use glib::{
    value::{FromValue, ValueTypeChecker, ValueTypeMismatchError, ValueTypeMismatchOrNoneError},
    ToValue,
};
use rquickjs::{
    qjs, qjs::c_int, AsFunction, AsProperty, ClassDef, Ctx, FromJs, Function, InputAccessor,
    IntoJs, Value,
};

use crate::common::*;

pub struct Prop<CLS, T> {
    param: ParamSpec,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Prop<CLS, T> {
    pub fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS, T> AsProperty<'js, T> for Prop<CLS, T>
where
    CLS: 'static + AsObject + LogClass,
    T: 'static
        + IntoJs<'js>
        + FromJs<'js>
        + for<'a> FromValue<'a, Checker: ValueTypeChecker<Error: HandleParamError>>
        + ToValue,
    &'js CLS: FromJs<'js>,
{
    /// Property configuration
    ///
    /// Returns the tuple which includes the following:
    /// - flags
    /// - value or undefined when no value is here
    /// - getter or undefined if the property hasn't getter
    /// - setter or undefined if the property hasn't setter
    fn config(self, ctx: Ctx<'js>) -> Result<(c_int, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            (qjs::JS_PROP_HAS_GET | qjs::JS_PROP_HAS_SET | qjs::JS_PROP_ENUMERABLE) as c_int,
            Value::new_undefined(ctx),
            Function::new::<_, CLS, T>(ctx, Getter::<CLS, T>::new(self.param.clone()))
                .into_js(ctx)?,
            Function::new::<_, CLS, T>(ctx, Setter::<CLS, T>::new(self.param)).into_js(ctx)?,
        ))
    }
}

pub struct Getter<CLS, T> {
    param: ParamSpec,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Getter<CLS, T>
where
    CLS: AsObject,
{
    pub fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS, T> AsFunction<'js, CLS, T> for Getter<CLS, T>
where
    T: 'static
        + for<'a> FromValue<'a, Checker: ValueTypeChecker<Error: HandleParamError>>
        + IntoJs<'js>,
    CLS: 'static + AsObject + LogClass,
    &'js CLS: FromJs<'js>,
{
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let acc = input.access();
        let this = acc.this::<&CLS>()?;
        let _ = this.span().enter();
        self.param.log_getter();
        let value = match this.as_object().property(self.param.name()) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err.message)),
        };
        let value = match value.get::<T>() {
            Ok(val) => val,
            Err(err) => return err.handle_param_error(acc.ctx(), &self.param),
        };
        value.into_js(acc.ctx())
    }
}

pub struct Setter<CLS, T> {
    param: ParamSpec,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Setter<CLS, T>
where
    CLS: AsObject,
{
    pub fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS, T> AsFunction<'js, CLS, T> for Setter<CLS, T>
where
    T: ToValue + FromJs<'js>,
    CLS: 'static + AsObject + LogClass,
    &'js CLS: FromJs<'js>,
{
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let mut acc = input.access();
        let this = acc.this::<&CLS>()?;
        let _ = this.span().enter();
        self.param.log_setter();
        if acc.len() < 1 {
            return Err(exception!("Setter must have at least one argument"));
        }
        let value = acc.arg::<T>()?;
        match this.as_object().set_property(self.param.name(), value) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not set property: {}", err.message)),
        };
        Ok(Value::new_undefined(acc.ctx()))
    }
}
