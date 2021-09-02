use crate::{build_class, inner::*};

pub struct InitStatic<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, 'js, CLS: Repr> InitStatic<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, CLS, R> Implements<InitStatic<'a, 'b, CLS>> for StaticFn<CLS, (), R, T>
where
    CLS: Type,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, CLS>) -> Result<()> {
        phase.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                let args = args.into_inner();
                let args_len = args.len();
                tuple_match_size(args_len, 0)?;

                let res = self.1();
                Result::<_>::Ok(res.into_js(ctx))
            }),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitStatic<'a, 'b, CLS>> for Superclass<CLS, SUPER>
where
    CLS: Type,
    SUPER: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, CLS>) -> Result<()> {
        build_class!(SUPER with InitSuperStatic::<'_, '_, CLS, SUPER>::new(phase.ctx, phase.obj));
        Ok(())
    }
}
