use rquickjs::Rest;

use crate::inner::*;

pub struct ClassStaticFunc<FW, const CLS: &'static str, A, R, T>(
    &'static str,
    T,
    PhantomData<(A, R, FW)>,
)
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>;

unsafe impl<FW, T, A, R, const CLS: &'static str> Sync for ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>,
{
}

impl<FW, T, A, R, const CLS: &'static str> ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>,
{
    pub(in crate) const fn new(name: &'static str, func: T) -> Self {
        ClassStaticFunc(name, func, PhantomData)
    }
}

impl<FW, T, A, R, const CLS: &'static str> TypeDef for ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: 'static + StaticFunc<FW, CLS, A, R>,
    R: 'static + for<'js> IntoJs<'js>,
    A: 'static,
    <T as StaticFunc<FW, CLS, A, R>>::Args: for<'a> FromJs<'a>,
{
    fn init_static<'js>(&'static self, _ctx: Ctx<'js>, exports: &Object<'js>) -> Result<()> {
        exports.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                let args = args.into_inner();
                let res = self.1.try_call(ctx, &args)?;
                Result::<_>::Ok(res.into_js(ctx))
            }),
        )?;
        Ok(())
    }
}
