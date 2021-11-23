use std::marker::PhantomData;

use glib::{
    value::{FromValue, ValueTypeChecker},
    ToValue,
};
use rquickjs::{qjs, qjs::c_int, AsFunction, AsProperty, Ctx, FromJs, Function, IntoJs, Value};

use crate::{common::*, Object};

pub struct Method<T> {
    param: ParamSpec,
    marker: PhantomData<T>,
}

impl<T> Method<T> {
    pub fn new(param: ParamSpec) -> Self {
        Self {
            param,
            marker: PhantomData,
        }
    }
}

impl<'js, T> AsProperty<'js, T> for Method<T>
where
    T: 'static
        + IntoJs<'js>
        + FromJs<'js>
        + for<'a> FromValue<'a, Checker: ValueTypeChecker<Error: HandleParamError>>
        + ToValue,
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
            Function::new::<_, (), T>(ctx, Getter::<T>::new(self.param.clone())).into_js(ctx)?,
            Function::new::<_, (), T>(ctx, Setter::<T>::new(self.param)).into_js(ctx)?,
        ))
    }
}
