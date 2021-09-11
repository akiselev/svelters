use crate::inner::*;

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'js, CLS>) -> Result<InitProto<'a, 'js, CLS>> {
        let obj = Object::new(phase.ctx)?;
        build_class!(CLS with InitVirt::<CLS>::new(phase.ctx, &obj));
        phase.obj.set("__VIRT__", obj)?;
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Obj<CLS> = this.as_ref();
                        let rt = cls.rt.inner.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&cls.rt, cls.idx)?;
                        let objref = this.as_object().clone();
                        let proto: Object<'js> = objref.get("__VIRT__")?;
                        if CLS::CLASS_NAME == target {
                            WithProto(
                                Virt::<CLS>::new(
                                    cls.rt.clone(),
                                    cls.idx,
                                    obj.deref().clone(),
                                    objref,
                                ),
                                proto,
                            )
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
    CLS: Type + Upcast<SUPER>,
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

fn caster_fn<'js, BASE: Type + Upcast<CLS>, CLS: Type>(
    ctx: Ctx<'js>,
    obj: Object<'js>,
    rt: Rt,
    idx: Idx,
) -> Result<Value<'js>> {
    let this = Obj::<BASE>::from_rt(&rt, idx)?;
    Virt::<CLS>::new(rt.clone(), idx, this.cast(), obj).into_js(ctx)
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for members::Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Clone + Type,
    CLS: Upcast<NESTED>,
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
