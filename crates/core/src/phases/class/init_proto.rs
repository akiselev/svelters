use crate::inner::*;

pub struct InitProto<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS> InitProto<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, X, T, FW, CLS, R> Implements<InitProto<'a, 'b, FW, CLS>>
    for ClassMethod<FW, CLS, CLS, (T,), R, X>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<T>,
    X: 'static + for<'x> Fn(&'x T) -> R,
    Wrapped<FW, CLS>: Cast<T>,
    R: 'static + for<'js> IntoJs<'js>,
    <FW as Wraps<CLS>>::Type: for<'x> GetRef<'x, CLS>,
    CLS: 'static,
    T: 'static,
{
    fn build(self, input: &InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Implementing {}::{}",
            <FW as TypeDef<CLS>>::typename(),
            self.0
        );
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
                        let data = obj.inner.cast();
                        let res = self.1(&*data);
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitProto<'a, 'b, FW, CLS>>
    for ClassSuper<FW, CLS, SUPER>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS> + Wraps<SUPER> + TypeDef<SUPER>,
    CLS: 'static,
{
    fn build(self, input: &InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Building SUPER PROTO on {}",
            <FW as TypeDef<CLS>>::typename()
        );
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: Type<FW, SUPER, _> = Type::new(phase);
        <FW as TypeDef<SUPER>>::define(cls)?;
        Ok(())
    }
}
