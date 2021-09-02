use rquickjs::This;

use crate::{build_class, inner::*};

pub struct InitProto<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, 'js, CLS: Repr> InitProto<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, FUNC, THIS, CLS, RET> Implements<InitProto<'a, 'b, CLS>>
    for Method<CLS, (THIS,), RET, FUNC>
where
    CLS: Type,
    ReprOf<CLS>: Cast<THIS>,
    THIS: 'static + Repr,
    RET: 'static + for<'js> IntoJs<'js>,
    FUNC: 'static + for<'x> Fn(&'x THIS) -> RET,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, CLS>) -> Result<()> {
        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<CLS> = this.as_ref();
                        let data = obj.inner.cast();
                        let res = (self.func)(data.as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, CLS, PROP, GET, SET> Implements<InitProto<'a, 'b, CLS>>
    for Property<CLS, PROP, GET, SET>
where
    CLS: Type,
    ReprOf<CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, CLS>) -> Result<()> {
        let Property {
            name,
            getter,
            setter,
            ..
        } = self;

        phase.obj.remove(name).ok();
        let res = phase.obj.prop(
            name,
            Accessor::new(
                rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>| {
                    let obj: &Obj<CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let val = getter(data.as_ref())?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>, val: PROP| {
                    let obj: &Obj<CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    setter(data.as_ref(), val)?;
                    Result::<_>::Ok(())
                }),
            )
            .enumerable()
            .configurable(),
        );
        res
    }
}

impl<'a, 'b: 'a, CLS, PROP, GET> Implements<InitProto<'a, 'b, CLS>> for ReadOnly<CLS, PROP, GET>
where
    CLS: Type,
    ReprOf<CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, CLS>) -> Result<()> {
        let ReadOnly { name, getter, .. } = self;

        phase.obj.remove(name).ok();
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>| {
                let obj: &Obj<CLS> = this.as_ref();
                let data = obj.inner.cast();
                let val = getter(data.as_ref())?;
                rquickjs::Result::Ok(val)
            }))
            .enumerable(),
        );
        res
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitProto<'a, 'b, CLS>> for Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'b, CLS>) -> Result<()> {
        build_class!(SUPER with InitSuperProto::<'_, '_, CLS, SUPER>::new(phase.ctx, phase.obj));
        Ok(())
    }
}
