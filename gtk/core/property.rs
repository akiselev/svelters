use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use glib::{value::FromValue, ToValue};
use rquickjs::{
    qjs, qjs::c_int, AsFunction, AsProperty, ClassDef, Ctx, FromJs, Function, IntoJs, Value,
};

use crate::common::*;

pub struct Prop<CLS, T> {
    param_name: &'static str,
    spec: UnsafeCell<Option<ParamSpec>>,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Prop<CLS, T> {
    pub fn new(param_name: &'static str) -> Self {
        Self {
            param_name,
            spec: UnsafeCell::new(None),
            marker: PhantomData,
        }
    }
}

impl<'js, CLS, T> AsProperty<'js, T> for Prop<CLS, T>
where
    CLS: 'static + AsObject,
    T: 'static + IntoJs<'js> + FromJs<'js> + for<'a> FromValue<'a> + ToValue,
    for<'a> &'a CLS: FromJs<'js>,
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
            (qjs::JS_PROP_HAS_GET | qjs::JS_PROP_HAS_SET | qjs::JS_PROP_HAS_ENUMERABLE) as c_int,
            Value::new_undefined(ctx),
            Function::new::<_, CLS, T>(ctx, Getter::<CLS, T>::new(self.param_name)).into_js(ctx)?,
            Function::new::<_, CLS, T>(ctx, Setter::<CLS, T>::new(self.param_name)).into_js(ctx)?,
        ))
    }
}

pub struct Getter<CLS, T> {
    param_name: &'static str,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Getter<CLS, T>
where
    CLS: AsObject,
{
    pub fn new(param_name: &'static str) -> Self {
        Self {
            param_name,
            marker: PhantomData,
        }
    }
}

impl<'js, CLS, T> AsFunction<'js, CLS, T> for Getter<CLS, T>
where
    T: for<'a> FromValue<'a> + IntoJs<'js>,
    CLS: AsObject,
    for<'a> &'a CLS: FromJs<'js>,
{
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let mut acc = input.access();
        let this = acc.this::<&CLS>()?;
        let value = match this.as_object().property(self.param_name) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err.message)),
        };
        let value = match value.get::<T>() {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err)),
        };
        value.into_js(acc.ctx())
    }
}

pub struct Setter<CLS, T> {
    param_name: &'static str,
    spec: UnsafeCell<Option<ParamSpec>>,
    marker: PhantomData<(CLS, T)>,
}

impl<CLS, T> Setter<CLS, T>
where
    CLS: AsObject,
{
    pub fn new(param_name: &'static str) -> Self {
        Self {
            param_name,
            spec: UnsafeCell::new(None),
            marker: PhantomData,
        }
    }

    fn spec(&self, this: &CLS) -> &ParamSpec {
        unsafe {
            let spec: &mut Option<ParamSpec> = &mut *self.spec.get();
            if let Some(spec) = spec {
                &*spec
            } else {
                let obj = this.as_object();
                *spec = obj.find_property(self.param_name.as_ref());
                &*spec.as_ref().expect("Param not found")
            }
        }
    }
}

impl<'js, CLS, T> AsFunction<'js, CLS, T> for Setter<CLS, T>
where
    T: ToValue + FromJs<'js>,
    CLS: AsObject,
    for<'a> &'a CLS: FromJs<'js>,
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
        let value = acc.arg::<T>()?;
        match this.as_object().set_property(self.param_name, value) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not set property: {}", err.message)),
        };
        Ok(Value::new_undefined(acc.ctx()))
    }
}
