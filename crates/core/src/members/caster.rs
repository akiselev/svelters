use crate::inner::*;

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'js, CLS>) -> Result<InitProto<'a, 'js, CLS>> {
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Obj<CLS> = this.as_ref();
                        let rt = cls.rt.inner.deref().borrow();
                        if CLS::CLASS_NAME == target {
                            Virt::<CLS>::new(cls.rt.clone(), cls.idx, this.as_object().clone())
                                .into_js(ctx)
                        } else {
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let caster = obj.casters.get(target.as_str()).ok_or_else(|| {
                                exception!("Could not cast {} to {}", CLS::CLASS_NAME, target)
                            })?;
                            caster(ctx, this.as_object().clone(), cls.rt.clone(), cls.idx)
                        }
                    },
                ),
            ),
        )?;

        Ok(phase)
    }
}

impl<'a, CLS, SUPER> Implements<InitCaster<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Cast<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: InitCaster<CLS>) -> Result<InitCaster<CLS>> {
        // let casters = phase.map.get_mut(Class::<SUPER>::typekey()).unwrap();
        // let map: &mut CasterMap<SUPER> = casters.downcast_mut().unwrap();
        phase
            .map
            .insert(SUPER::CLASS_NAME, smallbox!(caster_fn::<CLS, SUPER>));

        build_class!(SUPER with InitSuperCaster::<CLS, SUPER>::new(phase.map));
        Ok(phase)
    }
}

fn caster_fn<'js, BASE: Type + Cast<CLS>, CLS: Type>(
    ctx: Ctx<'js>,
    obj: Object<'js>,
    rt: Rt,
    idx: Idx,
) -> Result<Value<'js>> {
    println!("Trying to cast {} to {}", BASE::CLASS_NAME, CLS::CLASS_NAME);
    println!("Trying to cast {} to {}", BASE::CLASS_NAME, CLS::CLASS_NAME);
    Virt::<CLS>::new(rt, idx, obj).into_js(ctx)
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for members::Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Clone + Type,
    CLS: Cast<NESTED>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitSuperCaster<CLS, SUPER>,
    ) -> Result<InitSuperCaster<CLS, SUPER>> {
        // let casters = phase.map.get_mut(Class::<NESTED>::typekey()).unwrap();
        // let map: &mut CasterMap<NESTED> = casters.downcast_mut().unwrap();
        // map.insert(Class::<CLS>::typekey(), smallbox!(caster_fn::<CLS, NESTED>));
        phase
            .map
            .insert(NESTED::CLASS_NAME, smallbox!(caster_fn::<CLS, NESTED>));

        build_class!(NESTED with InitSuperCaster::<CLS, NESTED>::new(phase.map));
        Ok(phase)
    }
}
