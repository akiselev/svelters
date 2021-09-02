use crate::{build_class, inner::*};

pub struct InitSuperProto<'a, 'js, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, 'js, CLS, SUPER> InitSuperProto<'a, 'js, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, CLS, R, SUPER> Implements<InitSuperProto<'a, 'b, CLS, SUPER>>
    for Method<SUPER, (SUPER,), R, T>
where
    CLS: Type,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, CLS, SUPER>) -> Result<()> {
        if build_class!(CLS with HasMember::<'a, CLS>::new(self.name)).is_set {
            return Ok(());
        }

        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        println!(
                            "Calling method {}::{} for {}",
                            SUPER::name(),
                            self.name,
                            CLS::name()
                        );
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<CLS> = this.as_ref();
                        let res = (self.func)(obj.as_inner().cast().as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, CLS, SUPER, PROP, GET, SET> Implements<InitSuperProto<'a, 'b, CLS, SUPER>>
    for Property<SUPER, PROP, GET, SET>
where
    CLS: Type,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x SUPER, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, CLS, SUPER>) -> Result<()> {
        if build_class!(CLS with HasMember::<'a, CLS>::new(self.name)).is_set {
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
                rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>| {
                    let obj: &Obj<CLS> = this.as_ref();
                    let data = obj.inner.cast();
                    let sup: &SUPER = data.as_ref();
                    let val = getter(sup)?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>, val: PROP| {
                    let obj: &Obj<CLS> = this.as_ref();
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

impl<'a, 'b: 'a, CLS, SUPER, PROP, GET> Implements<InitSuperProto<'a, 'b, CLS, SUPER>>
    for ReadOnly<SUPER, PROP, GET>
where
    CLS: Type,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, CLS, SUPER>) -> Result<()> {
        if build_class!(CLS with HasMember::<'a, CLS>::new(self.name)).is_set {
            return Ok(());
        }
        let ReadOnly { name, getter, .. } = self;
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<CLS>>| {
                let obj: &Obj<CLS> = this.as_ref();
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

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperProto<'a, 'b, CLS, SUPER>>
    for Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Type,
    ReprOf<CLS>: Cast<NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperProto<'a, 'b, CLS, SUPER>) -> Result<()> {
        build_class!(NESTED with NestedSuperProto::<'_, '_, CLS, SUPER, NESTED>::new(phase.ctx, phase.obj, 1));

        Ok(())
    }
}
