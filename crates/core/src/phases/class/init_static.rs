use crate::inner::*;

pub struct InitStatic<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS> InitStatic<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<InitStatic<'a, 'b, FW, CLS>>
    for ClassStaticFunc<FW, CLS, (), R, T>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
{
    fn build(self, input: &InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "---------- IMPL static {}::{} ",
            <FW as TypeDef<CLS>>::typename(),
            self.0,
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                println!(
                    "CALLING STATIC {}::{} ",
                    <FW as TypeDef<CLS>>::typename(),
                    self.0,
                );
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
    for ClassSuper<FW, CLS, SUPER>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<SUPER> + TypeDef<SUPER>,
    CLS: 'static,
{
    fn build(self, input: &InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "------ SUPER STATIC {} on {}",
            <FW as TypeDef<SUPER>>::typename(),
            <FW as TypeDef<CLS>>::typename(),
        );
        let phase = InitSuperStatic::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: Type<FW, SUPER, _> = Type::new(phase);
        <FW as TypeDef<SUPER>>::define(cls)?;
        Ok(())
    }
}
