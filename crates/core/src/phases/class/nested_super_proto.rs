use crate::{build_class, inner::*};

pub struct NestedSuperProto<'a, 'js, ROOT, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) depth: isize,
    pub(in crate) framework: PhantomData<(ROOT, CLS, SUPER)>,
}

impl<'a, 'js, CLS: Repr, ROOT, SUPER> NestedSuperProto<'a, 'js, ROOT, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, ROOT, CLS, R, SUPER> Implements<NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>>
    for Method<SUPER, (SUPER,), R, T>
where
    CLS: Type,
    ROOT: Type,
    SUPER: Type,
    ReprOf<ROOT>: Cast<SUPER>,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(ROOT with HasMember::<'a, ROOT>::new(self.name)).is_set {
            return Ok(());
        }
        if build_class!(ROOT with InheritsMember::<'a, ROOT>::new(self.name, phase.depth - 1))
            .is_set
        {
            return Ok(());
        }
        phase.obj.set(
            self.name,
            rquickjs::Func::new(
                self.name,
                rquickjs::Method(
                    move |this: JsClass<'b, Obj<ROOT>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<ROOT> = this.as_ref();
                        let res = (self.func)(obj.as_inner().cast().as_ref());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, ROOT, CLS, SUPER, PROP, GET, SET>
    Implements<NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>> for Property<SUPER, PROP, GET, SET>
where
    CLS: Type,
    ROOT: Type,
    SUPER: Type,
    ReprOf<ROOT>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
    SET: 'static + for<'x> Fn(&'x SUPER, PROP) -> Result<()>,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(ROOT with HasMember::<'a, ROOT>::new(self.name)).is_set {
            return Ok(());
        }
        if build_class!(ROOT with InheritsMember::<'a, ROOT>::new(self.name, phase.depth - 1))
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
                rquickjs::Method(move |this: JsClass<'b, Obj<ROOT>>| {
                    let obj: &Obj<ROOT> = this.as_ref();
                    let data = obj.inner.cast();
                    let sup: &SUPER = data.as_ref();
                    let val = getter(sup)?;
                    rquickjs::Result::Ok(val)
                }),
                rquickjs::Method(move |this: JsClass<'b, Obj<ROOT>>, val: PROP| {
                    let obj: &Obj<ROOT> = this.as_ref();
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

impl<'a, 'b: 'a, ROOT, CLS, SUPER, PROP, GET> Implements<NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>>
    for ReadOnly<SUPER, PROP, GET>
where
    CLS: Type,
    ROOT: Type,
    SUPER: Type,
    ReprOf<ROOT>: Cast<SUPER>,
    PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x>,
    GET: 'static + for<'x> Fn(&'x SUPER) -> Result<PROP>,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperProto<'a, 'b, ROOT, CLS, SUPER>) -> Result<()> {
        if build_class!(ROOT with HasMember::<'a, ROOT>::new(self.name)).is_set {
            return Ok(());
        }
        if build_class!(ROOT with InheritsMember::<'a, ROOT>::new(self.name, phase.depth)).is_set {
            return Ok(());
        }

        let ReadOnly { name, getter, .. } = self;
        let res = phase.obj.prop(
            name,
            Accessor::new_get(rquickjs::Method(move |this: JsClass<'b, Obj<ROOT>>| {
                let obj: &Obj<ROOT> = this.as_ref();
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

impl<'a, 'b: 'a, CLS, INTER, ROOT, SUPER> Implements<NestedSuperProto<'a, 'b, ROOT, CLS, INTER>>
    for Superclass<INTER, SUPER>
where
    CLS: Type,
    INTER: Type,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperProto<'a, 'b, ROOT, CLS, INTER>) -> Result<()> {
        build_class!(SUPER with NestedSuperProto::<'_, '_, ROOT, INTER, SUPER>::new(phase.ctx, phase.obj, phase.depth + 1));

        Ok(())
    }
}
