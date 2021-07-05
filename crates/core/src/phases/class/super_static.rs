use crate::inner::*;

pub struct InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS, SUPER)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW, SUPER> InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
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
    FW: Framework,
    CLS: TypeDef<FW>,
    SUPER: TypeDef<FW>,
    CLS: Repr<FW>,
    SUPER: Repr<FW>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
    SUPER: 'static,
{
    fn build(self, input: &InitSuperStatic<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "---------- IMPL super static {}::{} on {}",
            <SUPER as TypeDef<FW>>::typename(),
            self.0,
            <CLS as TypeDef<FW>>::typename(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                println!(
                    "CALLING SUPER STATIC {}::{} ",
                    <CLS as TypeDef<FW>>::typename(),
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
