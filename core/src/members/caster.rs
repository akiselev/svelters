use crate::inner::*;

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'js, CLS>) -> Result<InitProto<'a, 'js, CLS>> {
        let obj = Object::new(phase.ctx)?;
        build!(CLS::InitVirt::<CLS>[phase.ctx, &obj]);
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
                            (caster.obj_to_virt)(
                                ctx,
                                this.as_object().clone(),
                                cls.rt.clone(),
                                cls.idx,
                            )
                        }
                    },
                ),
            ),
        )?;
        phase.obj.set(
            "asImpl",
            rquickjs::Func::new(
                "asImpl",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Obj<CLS> = this.as_ref();
                        let rt = cls.rt.inner.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&cls.rt, cls.idx)?;
                        let objref = this.as_object().clone();
                        if CLS::CLASS_NAME == target {
                            Result::<_>::Ok(Some(
                                Impl::<CLS>::new(obj.deref().clone()).into_js(ctx)?,
                            ))
                        } else {
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let res = obj.casters.get(target.as_str());
                            match res {
                                Some(caster) => Ok(Some((caster.obj_to_impl)(
                                    ctx,
                                    this.as_object().clone(),
                                    cls.rt.clone(),
                                    cls.idx,
                                )?)),
                                None => Ok(None),
                            }
                        }
                    },
                ),
            ),
        )?;

        Ok(phase)
    }
}

impl<'a, 'js, CLS> Implements<InitVirt<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitVirt<'a, 'js, CLS>) -> Result<InitVirt<'a, 'js, CLS>> {
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Virt<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Virt<CLS> = this.as_ref();
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
                            (caster.obj_to_virt)(
                                ctx,
                                this.as_object().clone(),
                                cls.rt.clone(),
                                cls.idx,
                            )
                        }
                    },
                ),
            ),
        )?;
        phase.obj.set(
            "asImpl",
            rquickjs::Func::new(
                "asImpl",
                rquickjs::Method(
                    move |this: JsClass<'js, Virt<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Virt<CLS> = this.as_ref();
                        let rt = cls.rt.inner.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&cls.rt, cls.idx)?;
                        let objref = this.as_object().clone();
                        if CLS::CLASS_NAME == target {
                            Result::<_>::Ok(Some(
                                Impl::<CLS>::new(obj.deref().clone()).into_js(ctx)?,
                            ))
                        } else {
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let res = obj.casters.get(target.as_str());
                            match res {
                                Some(caster) => Ok(Some((caster.obj_to_impl)(
                                    ctx,
                                    this.as_object().clone(),
                                    cls.rt.clone(),
                                    cls.idx,
                                )?)),
                                None => Ok(None),
                            }
                        }
                    },
                ),
            ),
        )?;

        Ok(phase)
    }
}

impl<'a, 'js, CLS> Implements<InitImpl<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitImpl<'a, 'js, CLS>) -> Result<InitImpl<'a, 'js, CLS>> {
        phase.obj.set(
            "asImpl",
            rquickjs::Func::new(
                "asImpl",
                rquickjs::Method(
                    move |this: JsClass<'js, Impl<CLS>>, ctx: Ctx<'js>, target: String| {
                        let cls: &Impl<CLS> = this.as_ref();
                        if CLS::CLASS_NAME == target {
                            Result::<_>::Ok(Some(cls.clone().into_js(ctx)?))
                        } else {
                            let doc: &Document = ctx.globals().get("document")?;
                            let rt = doc.rt.inner.borrow();
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let res = obj.casters.get(target.as_str());
                            match res {
                                Some(caster) => {
                                    Ok(Some((caster.impl_to_impl)(ctx, this.as_object().clone())?))
                                }
                                None => Ok(None),
                            }
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
        phase.map.insert(
            SUPER::CLASS_NAME,
            Casters::new(
                obj_to_virt::<CLS, SUPER>,
                obj_to_impl::<CLS, SUPER>,
                impl_to_impl::<CLS, SUPER>,
            ),
        );

        build!(SUPER::InitSuperCaster::<CLS, SUPER>[phase.map]);
        Ok(phase)
    }
}

fn obj_to_virt<'js, BASE: Type + Upcast<CLS>, CLS: Type>(
    ctx: Ctx<'js>,
    obj: Object<'js>,
    rt: Rt,
    idx: Idx,
) -> Result<Value<'js>> {
    let this = Obj::<BASE>::from_rt(&rt, idx)?;
    Virt::<CLS>::new(rt.clone(), idx, this.upcast(), obj).into_js(ctx)
}

fn obj_to_impl<'js, BASE: Type + Upcast<CLS>, CLS: Type>(
    ctx: Ctx<'js>,
    obj: Object<'js>,
    rt: Rt,
    idx: Idx,
) -> Result<Value<'js>> {
    let this = Obj::<BASE>::from_rt(&rt, idx)?;
    Impl::<CLS>::new(this.upcast()).into_js(ctx)
}

fn impl_to_impl<'js, BASE: Type + Upcast<CLS>, CLS: Type>(
    ctx: Ctx<'js>,
    obj: Object<'js>,
) -> Result<Value<'js>> {
    let this = Impl::<BASE>::from_js_ref(ctx, obj.into_value())?;
    Impl::<CLS>::new(this.upcast()).into_js(ctx)
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for members::Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Type,
    CLS: Upcast<NESTED>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitSuperCaster<CLS, SUPER>,
    ) -> Result<InitSuperCaster<CLS, SUPER>> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Casters::new(
                obj_to_virt::<CLS, NESTED>,
                obj_to_impl::<CLS, NESTED>,
                impl_to_impl::<CLS, NESTED>,
            ),
        );

        build!(NESTED::InitSuperCaster::<CLS, NESTED>[phase.map]);
        Ok(phase)
    }
}
