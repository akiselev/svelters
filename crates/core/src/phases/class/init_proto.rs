use rquickjs::This;

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

impl<'a, 'b: 'a, FUNC, THIS, FW, CLS, RET> Implements<InitProto<'a, 'b, FW, CLS>>
    for Method<FW, CLS, (THIS,), RET, FUNC>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<THIS>,
    THIS: 'static + Repr<FW>,
    RET: 'static + for<'js> IntoJs<'js>,
    FUNC: 'static + for<'x> Fn(&'x THIS) -> RET,
{
    fn build(self, input: &mut InitProto<'a, 'b, FW, CLS>) -> Result<()> {
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
                        let res = self.1(data.as_ref());
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
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<THIS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x THIS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x THIS, PROP) -> Result<()>,
{
    fn build(self, input: &mut InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        let Property {
            name,
            getter,
            setter,
            ..
        } = self;
        input.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>| {
                    let obj: &Obj<FW, CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let val = getter(data.as_ref())?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>, val: PROP| {
                    let obj: &Obj<FW, CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    setter(data.as_ref(), val)?;
                    Result::<_>::Ok(())
                }),
            )
            .enumerable(),
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
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let mut cls: TypeBuilder<FW, SUPER, _> = TypeBuilder::new(phase);
        <SUPER as Type<FW>>::define(&mut cls)?;
        Ok(())
    }
}
