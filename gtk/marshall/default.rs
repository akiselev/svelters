use std::marker::PhantomData;

use rquickjs::{qjs::c_char, Ctx, InputAccessor, IntoJs, Value};

use super::marshall::*;
use crate::common::*;

pub struct DefaultMarshall<C, T>(PhantomData<(C, T)>);

impl<'i, 'js, C, T> InitMarshall<'i, 'js> for DefaultMarshall<C, T> {
    fn init_marshall_state(input: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(DefaultMarshall(PhantomData))
    }
}

impl<'js, C, T, M> MarshallResult<'js, M> for DefaultMarshall<C, T>
where
    T: IntoJs<'js>,
    C: for<'a> Marshall<'a, T>,
{
    type Type = C;

    type Marshall<'a> = DefaultMarshallInner<C, T>;

    fn marshall_result<'a>(self, value: Self::Type) -> Result<Self::Marshall<'a>> {
        Ok(DefaultMarshallInner(value, PhantomData))
    }
}

pub struct DefaultMarshallInner<C, T>(C, PhantomData<T>);

impl<'js, C, T> IntoJs<'js> for DefaultMarshallInner<C, T>
where
    T: IntoJs<'js>,
    C: for<'a> Marshall<'a, T>,
{
    fn into_js(self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let res = if self.0.use_ref() {
            self.0.marshall_as_ref()
        } else {
            self.0.marshall_as_owned()
        }?;
        res.into_js(ctx)
    }
}
