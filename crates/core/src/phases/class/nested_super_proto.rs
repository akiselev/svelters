use crate::{build_class, inner::*};

pub struct NestedSuperProto<'a, 'js, FW, ROOT, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) depth: isize,
    pub(in crate) framework: PhantomData<(FW, ROOT, CLS, SUPER)>,
}

impl<'a, 'js, CLS: Repr<FW>, ROOT, FW, SUPER> NestedSuperProto<'a, 'js, FW, ROOT, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, ROOT, CLS, R, SUPER>
    Implements<NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>> for Method<FW, SUPER, (SUPER,), R, T>
where
    FW: Framework,
    CLS: Type<FW>,
    ROOT: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, ROOT>: Cast<SUPER>,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)] fn build(self, phase: &mut NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(FW::ROOT with HasMember::<'a, FW, ROOT>::new(self.name)).is_set {
            
            return Ok(());
        }
        if build_class!(FW::ROOT with InheritsMember::<'a, FW, ROOT>::new(self.name, phase.depth - 1))
            .is_set
        {
            
            return Ok(());
        }
        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<FW, ROOT>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, ROOT> = this.as_ref();
                        let res = (self.func)(obj.as_inner().cast().as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, ROOT, CLS, SUPER, PROP, GET, SET>
    Implements<NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>>
    for Property<FW, SUPER, PROP, GET, SET>
where
    FW: Framework,
    CLS: Type<FW>,
    ROOT: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, ROOT>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x SUPER, PROP) -> Result<()>,
{
    #[inline(always)] fn build(self, phase: &mut NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(FW::ROOT with HasMember::<'a, FW, ROOT>::new(self.name)).is_set {
            
            return Ok(());
        }
        if build_class!(FW::ROOT with InheritsMember::<'a, FW, ROOT>::new(self.name, phase.depth - 1))
            .is_set
        {
            
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
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, ROOT>>| {
                    let obj: &Obj<FW, ROOT> = this.as_ref();
                    let data = obj.inner.cast();
                    let sup: &SUPER = data.as_ref();
                    let val = getter(sup)?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<FW, ROOT>>, val: PROP| {
                    let obj: &Obj<FW, ROOT> = this.as_ref();
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

impl<'a, 'b: 'a, FW, ROOT, CLS, SUPER, PROP, GET>
    Implements<NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>> for ReadOnly<FW, SUPER, PROP, GET>
where
    FW: Framework,
    CLS: Type<FW>,
    ROOT: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, ROOT>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
{
    #[inline(always)] fn build(self, phase: &mut NestedSuperProto<'a, 'b, FW, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(FW::ROOT with HasMember::<'a, FW, ROOT>::new(self.name)).is_set {
            
            return Ok(());
        }
        if build_class!(FW::ROOT with InheritsMember::<'a, FW, ROOT>::new(self.name, phase.depth))
            .is_set
        {
            
            return Ok(());
        }

        let ReadOnly { name, getter, .. } = self;
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<FW, ROOT>>| {
                let obj: &Obj<FW, ROOT> = this.as_ref();
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

impl<'a, 'b: 'a, FW, CLS, INTER, ROOT, SUPER>
    Implements<NestedSuperProto<'a, 'b, FW, ROOT, CLS, INTER>> for Superclass<FW, INTER, SUPER>
where
    FW: Framework,
    CLS: Type<FW>,
    INTER: Type<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
{
    #[inline(always)] fn build(self, phase: &mut NestedSuperProto<'a, 'b, FW, ROOT, CLS, INTER>) -> Result<()> {
        build_class!(FW::SUPER with NestedSuperProto::<'_, '_, FW, ROOT, INTER, SUPER>::new(phase.ctx, phase.obj, phase.depth + 1));

        Ok(())
    }
}
