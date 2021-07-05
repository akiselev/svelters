use crate::inner::*;

pub struct InitProto<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW> InitProto<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, X, T, FW, CLS, R> Implements<InitProto<'a, 'b, FW, CLS>>
    for Method<FW, CLS, (T,), R, X>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<T>,
    T: 'static + Repr<FW>,
    X: 'static + for<'x> Fn(&'x T) -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    fn build(self, input: &mut InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "---------- IMPL {}::{}",
            <CLS as Type<FW>>::typename(),
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
    for Property<FW, CLS, THIS, PROP, GET, SET>
where
    FW: Framework,
    THIS: 'static,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: CastMut<THIS>,
    PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x THIS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x mut THIS, PROP) -> Result<()>,
    for<'x> CastedAsMut<'x, FW, CLS, THIS>: DerefMut<Target = THIS>,
{
    fn build(self, input: &mut InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "---------- IMPL prop {}::{}",
            <CLS as Type<FW>>::typename(),
            self.name
        );
        let Property {
            name,
            getter,
            setter,
            ..
        } = self;
        input.obj.prop(
            name,
            Accessor::new(
                move |ctx: Ctx<'b>, mut this: JsClass<'b, Obj<FW, CLS>>| {
                    let obj: &mut Obj<FW, CLS> = this.as_mut();
                    let data = obj.inner.cast_mut();
                    let res = getter(&*data);
                    Result::<_>::Ok(res.into_js(ctx))
                },
                move |ctx: Ctx<'b>, mut this: JsClass<'b, Obj<FW, CLS>>, val: PROP| {
                    let obj: &mut Obj<FW, CLS> = this.as_mut();
                    let mut data = obj.inner.cast_mut();
                    let res = setter(&mut *data, val);
                    Result::<_>::Ok(res.into_js(ctx))
                },
            ),
        )
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitProto<'a, 'b, FW, CLS>>
    for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
{
    fn build(self, input: &mut InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "------ SUPER PROTO {} on {}",
            <SUPER as Type<FW>>::typename(),
            <CLS as Type<FW>>::typename(),
        );
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let mut cls: TypeBuilder<FW, SUPER, _> = TypeBuilder::new(phase);
        <SUPER as Type<FW>>::define(&mut cls)?;
        Ok(())
    }
}
