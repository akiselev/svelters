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
    for ClassMethod<FW, CLS, (T,), R, X>
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

impl<'a, 'b: 'a, FW, CLS, THIS, PROP, GET, SET> Implements<InitProto<'a, 'b, FW, CLS>>
    for ClassProp<FW, CLS, THIS, PROP, GET, SET>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS>,
    Wrapped<FW, CLS>: Cast<THIS>,
    PROP: for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: for<'x> Fn(&'x THIS) -> Result<PROP>,
    SET: for<'x> Fn(&'x mut THIS, PROP) -> Result<()>,
    CLS: 'static,
    THIS: 'static,
    PROP: 'static,
    GET: 'static,
    SET: 'static,
    for<'x> <<FW as Wraps<CLS>>::Type as Cast<THIS>>::Output<'x>: DerefMut<Target = THIS>,
{
    fn build(self, input: &InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Implementing prop {}::{}",
            <FW as TypeDef<CLS>>::typename(),
            self.name
        );
        let ClassProp {
            name,
            getter,
            setter,
            ..
        } = self;
        input.obj.prop(
            name,
            Accessor::new(
                move |ctx: Ctx<'b>, this: JsClass<'b, Obj<FW, CLS>>| {
                    let obj: &Obj<FW, CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let res = getter(&*data);
                    Result::<_>::Ok(res.into_js(ctx))
                },
                move |ctx: Ctx<'b>, mut this: JsClass<'b, Obj<FW, CLS>>, val: PROP| {
                    let obj: &mut Obj<FW, CLS> = this.as_mut();
                    let mut data = obj.inner.cast();
                    let res = setter(&mut *data, val);
                    Result::<_>::Ok(res.into_js(ctx))
                },
            ),
        )
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
