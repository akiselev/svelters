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
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let res = (self.func)(obj.as_inner().cast().as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER, PROP, GET, SET> Implements<InitSuperProto<'a, 'b, FW, CLS, SUPER>>
    for Property<FW, SUPER, PROP, GET, SET>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x SUPER, PROP) -> Result<()>,
{
    fn build(self, input: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        let Property {
            name,
            getter,
            setter,
            ..
        } = self;
        let res = input.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>| {
                    let obj: &Obj<FW, CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let sup: &SUPER = data.as_ref();
                    let val = getter(sup)?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>, val: PROP| {
                    let obj: &Obj<FW, CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let sup: &SUPER = data.as_ref();
                    setter(sup, val)?;
                    Result::<_>::Ok(())
                }),
            )
            .enumerable(),
        );
        if let Err(ref err) = &res {
            println!("Err3: {:?}", err);
            println!("---");
            println!("---");
            println!("---");
        }
        res
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER, PROP, GET> Implements<InitSuperProto<'a, 'b, FW, CLS, SUPER>>
    for ReadOnly<FW, SUPER, PROP, GET>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
{
    fn build(self, input: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        let ReadOnly { name, getter, .. } = self;
        let res = input.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>| {
                let obj: &Obj<FW, CLS> = this.as_ref();
                let data = obj.inner.cast();
                let sup: &SUPER = data.as_ref();
                let val = getter(sup)?;
                rquickjs::Result::Ok(val)
            }))
            .enumerable(),
        );
        if let Err(ref err) = &res {
            println!("Err4: {:?}", err);
            println!("---");
            println!("---");
            println!("---");
        }
        res
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
