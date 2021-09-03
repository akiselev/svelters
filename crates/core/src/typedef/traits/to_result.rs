use crate::inner::*;

pub trait ToResult<'js>: Sized {
    fn to_result(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

impl<'js, T> ToResult<'js> for T
where
    T: NotResultType + NotCoreType + NotQJsType + IntoJs<'js>,
{
    fn to_result(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js(ctx)
    }
}

impl<'js> ToResult<'js> for Value<'js> {
    fn to_result(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self)
    }
}

impl<'js> !NotQJsType for Value<'js> {}

macro_rules! rquickjs_ty {
    ($($type:ty)*) => {
        $(
            impl<'js> !NotQJsType for $type {}

            impl<'js> ToResult<'js> for $type {
                fn to_result(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_js(ctx)
                }
            }
        )*
    };
}

rquickjs_ty!(
    rquickjs::String<'js>
    rquickjs::Object<'js>
    rquickjs::Array<'js>
    rquickjs::Function<'js>
);

pub auto trait NotResultType {}
impl<T, E> !NotResultType for std::result::Result<T, E> {}
