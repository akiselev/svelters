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
    for Method<FW, SUPER, (SUPER,), R, T>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    fn build(self, input: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        input.obj.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let res = self.1(obj.as_inner().cast().as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS, INTER, SUPER> Implements<InitSuperProto<'a, 'b, FW, CLS, INTER>>
    for Superclass<FW, INTER, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    INTER: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
{
    fn build(self, input: &mut InitSuperProto<'a, 'b, FW, CLS, INTER>) -> Result<()> {
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let mut cls: TypeBuilder<FW, SUPER, _> = TypeBuilder::new(phase);
        <SUPER as Type<FW>>::define(&mut cls)?;
        Ok(())
    }
}
