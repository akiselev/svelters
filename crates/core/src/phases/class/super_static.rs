use crate::inner::*;

pub struct InitSuperStatic<'a, 'js, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, 'js, CLS, SUPER> InitSuperStatic<'a, 'js, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, CLS, SUPER, R> Implements<InitSuperStatic<'a, 'b, CLS, SUPER>>
    for StaticFn<SUPER, (), R, T>
where
    CLS: Type,
    SUPER: Type,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperStatic<'a, 'b, CLS, SUPER>) -> Result<()> {
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
