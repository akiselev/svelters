use crate::{build_class, inner::*};

pub struct InitStatic<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, 'js, CLS> InitStatic<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, BASE, CLS, R> Implements<InitStatic<'a, 'b, BASE>> for StaticFn<CLS, (), R, T>
where
    BASE: Caster<CLS>,
    CLS: Type,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, BASE>) -> Result<()> {
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

impl<'a, 'b: 'a, BASE, SUPER> Implements<InitStatic<'a, 'b, BASE>> for Superclass<SUPER>
where
    BASE: Caster<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut InitStatic<'a, 'b, BASE>) -> Result<()> {
        build_class!(SUPER with InitStatic::<'_, '_, BASE>::new(phase.ctx, phase.obj));
        Ok(())
    }
}
