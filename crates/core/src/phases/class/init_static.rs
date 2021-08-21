use crate::{build_class, inner::*};

pub struct InitStatic<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW> InitStatic<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<InitStatic<'a, 'b, FW, CLS>>
    for StaticFn<FW, CLS, (), R, T>
where
    FW: Framework,
    CLS: Type<FW>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
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

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitStatic<'a, 'b, FW, CLS>>
    for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        build_class!(FW::SUPER with InitSuperStatic::<'_, '_, FW, CLS, SUPER>::new(phase.ctx, phase.obj));
        Ok(())
    }
}
