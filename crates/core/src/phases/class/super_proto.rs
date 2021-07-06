use crate::{build_phase, inner::*};

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
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        if build_phase!(FW::CLS with HasMember::<'a, FW, CLS>::new(self.name)).is_set {
            return Ok(());
        }

        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        println!(
                            "Calling method {}::{} for {}",
                            SUPER::name(),
                            self.name,
                            CLS::name()
                        );
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
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        if build_phase!(FW::CLS with HasMember::<'a, FW, CLS>::new(self.name)).is_set {
            return Ok(());
        }

        let Property {
            name,
            getter,
            setter,
            ..
        } = self;
        let res = phase.obj.prop(
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
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        if build_phase!(FW::CLS with HasMember::<'a, FW, CLS>::new(self.name)).is_set {
            return Ok(());
        }
        let ReadOnly { name, getter, .. } = self;
        let res = phase.obj.prop(
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
        res
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER, NESTED> Implements<InitSuperProto<'a, 'b, FW, CLS, SUPER>>
    for Superclass<FW, SUPER, NESTED>
where
    FW: Framework,
    CLS: Type<FW>,
    SUPER: Type<FW>,
    NESTED: Type<FW>,
    ReprOf<FW, CLS>: Cast<NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        build_phase!(FW::NESTED with NestedSuperProto::<'_, '_, FW, CLS, SUPER, NESTED>::new(phase.ctx, phase.obj, 1));

        Ok(())
    }
}
