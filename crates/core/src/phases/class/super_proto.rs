use crate::inner::*;

pub struct InitSuperProto<'a, 'js, FW, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS, SUPER)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS, SUPER> InitSuperProto<'a, 'js, FW, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R, SUPER> Implements<InitSuperProto<'a, 'b, FW, CLS, SUPER>>
    for ClassMethod<FW, SUPER, (SUPER,), R, T>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<SUPER> + TypeDef<SUPER>,
    Wrapped<FW, CLS>: for<'c> GetRef<'c, CLS>,
    Wrapped<FW, CLS>: Cast<SUPER>,
    SUPER: 'static,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
{
    fn build(self, input: &InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "---------- >> SUPER {}::{} on {}",
            <FW as TypeDef<SUPER>>::typename(),
            self.0,
            <FW as TypeDef<CLS>>::typename(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        println!(
                            "CALLING SUPER METHOD {}::{} ON {}",
                            <FW as TypeDef<SUPER>>::typename(),
                            self.0,
                            <FW as TypeDef<CLS>>::typename(),
                        );
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let res = self.1(&*obj.as_box().cast());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS, INTER, SUPER> Implements<InitSuperProto<'a, 'b, FW, CLS, INTER>>
    for ClassSuper<FW, INTER, SUPER>
where
    FW: TypeDef<CLS> + TypeDef<INTER> + TypeDef<SUPER>,
    Wrapped<FW, CLS>: for<'c> GetRef<'c, CLS>,
    Wrapped<FW, CLS>: Cast<SUPER>,
    SUPER: 'static,
    CLS: 'static,
{
    fn build(self, input: &InitSuperProto<'a, 'b, FW, CLS, INTER>) -> Result<()> {
        println!(
            "------ >> SUPER PROTO {} on {} via {}",
            <FW as TypeDef<SUPER>>::typename(),
            <FW as TypeDef<CLS>>::typename(),
            <FW as TypeDef<INTER>>::typename(),
        );
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: Type<FW, SUPER, _> = Type::new(phase);
        <FW as TypeDef<SUPER>>::define(cls)?;
        Ok(())
    }
}
