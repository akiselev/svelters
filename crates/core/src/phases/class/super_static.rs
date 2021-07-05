use crate::inner::*;

pub struct InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS, SUPER)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS, SUPER> InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, SUPER, R> Implements<InitSuperStatic<'a, 'b, FW, CLS, SUPER>>
    for ClassStaticFunc<FW, SUPER, (), R, T>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS> + TypeDef<SUPER> + Wraps<SUPER>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
    SUPER: 'static,
{
    fn build(self, input: &InitSuperStatic<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "---------- IMPL super static {}::{} on {}",
            <FW as TypeDef<SUPER>>::typename(),
            self.0,
            <FW as TypeDef<CLS>>::typename(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                println!(
                    "CALLING SUPER STATIC {}::{} ",
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
