use crate::inner::*;

pub struct InitSuperProto<'a, 'js, FW, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS, SUPER)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW, SUPER> InitSuperProto<'a, 'js, FW, CLS, SUPER> {
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
    FW: Framework,
    CLS: TypeDef<FW>,
    SUPER: TypeDef<FW>,
    CLS: Repr<FW>,
    SUPER: Repr<FW>,
    Type<FW, CLS>: Cast<SUPER>,
    SUPER: 'static,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
{
    fn build(self, input: &InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "---------- >> SUPER {}::{} on {}",
            <SUPER as TypeDef<FW>>::typename(),
            self.0,
            <CLS as TypeDef<FW>>::typename(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        println!(
                            "CALLING SUPER METHOD {}::{} ON {}",
                            <SUPER as TypeDef<FW>>::typename(),
                            self.0,
                            <CLS as TypeDef<FW>>::typename(),
                        );
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let res = self.1(&*obj.as_inner().cast());
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
    FW: Framework,
    CLS: 'static + TypeDef<FW>,
    INTER: 'static + TypeDef<FW>,
    SUPER: 'static + TypeDef<FW>,
    Type<FW, CLS>: Cast<SUPER>,
{
    fn build(self, input: &InitSuperProto<'a, 'b, FW, CLS, INTER>) -> Result<()> {
        println!(
            "------ >> SUPER PROTO {} on {} via {}",
            <SUPER as TypeDef<FW>>::typename(),
            <CLS as TypeDef<FW>>::typename(),
            <INTER as TypeDef<FW>>::typename(),
        );
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: TypeBuilder<FW, SUPER, _> = TypeBuilder::new(phase);
        <SUPER as TypeDef<FW>>::define(cls)?;
        Ok(())
    }
}
