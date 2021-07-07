use std::collections::HashMap;

use rquickjs::This;

use crate::{build_phase, inner::*};

pub struct InitCasterProto<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) funcs: &'static MetaFuncMap,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

// impl<'a, 'js, CLS: Type<FW>, FW> InitCasterProto<'a, 'js, FW, CLS> {
//     pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
//         Self {
//             ctx,
//             obj,
//             funcs: unsafe { &CLS::metadata().cast_methods },
//             framework: PhantomData,
//         }
//     }
// }

impl<'a, 'b: 'a, FUNC, THIS, FW, CLS, RET> Implements<InitCasterProto<'a, 'b, FW, CLS>>
    for Method<FW, CLS, (THIS,), RET, FUNC>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<THIS>,
    THIS: 'static + Repr<FW>,
    RET: 'static + for<'js> IntoJs<'js>,
    FUNC: 'static + for<'x> Fn(&'x THIS) -> RET,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCasterProto<'a, 'b, FW, CLS>) -> Result<()> {
        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
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

impl<'a, 'b: 'a, FW, CLS, PROP, GET, SET> Implements<InitCasterProto<'a, 'b, FW, CLS>>
    for Property<FW, CLS, PROP, GET, SET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCasterProto<'a, 'b, FW, CLS>) -> Result<()> {
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
            .enumerable()
            .configurable(),
        );
        res
    }
}

impl<'a, 'b: 'a, FW, CLS, PROP, GET> Implements<InitCasterProto<'a, 'b, FW, CLS>>
    for ReadOnly<FW, CLS, PROP, GET>
where
    FW: Framework,
    CLS: Type<FW>,
    ReprOf<FW, CLS>: Cast<CLS>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCasterProto<'a, 'b, FW, CLS>) -> Result<()> {
        let ReadOnly { name, getter, .. } = self;

        phase.obj.remove(name).ok();
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<FW, CLS>>| {
                let obj: &Obj<FW, CLS> = this.as_ref();
                let data = obj.inner.cast();
                let val = getter(data.as_ref())?;
                rquickjs::Result::Ok(val)
            }))
            .enumerable(),
        );
        res
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitCasterProto<'a, 'b, FW, CLS>>
    for Superclass<FW, CLS, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCasterProto<'a, 'b, FW, CLS>) -> Result<()> {
        build_phase!(FW::SUPER with InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(phase.ctx, phase.obj));
        Ok(())
    }
}
