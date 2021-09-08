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
                        // TODO: FIX CASTER
                        // let map = cell.get_or_try_init(|| {
                        //     tracing::info!("Building caster for {}", CLS::CLASS_NAME);
                        //     map.insert(
                        //         CLS::CLASS_NAME,
                        //         Box::new(|ctx, value| {
                        //             tracing::info!("Casting {} to self", CLS::CLASS_NAME);
                        //             let obj: &Obj<CLS> = value.as_ref();
                        //             Virt::<CLS>::new(
                        //                 obj.rt.clone(),
                        //                 obj.idx,
                        //                 value.as_object().clone(),
                        //             )
                        //             .into_js(ctx)
                        //         }),
                        //     );
                        //     {
                        //         build_class!(CLS with InitCaster::<CLS>::new(&mut map))
                        //     };
                        //     Result::<_>::Ok(map)
                        // })?;
                        // match map.get(target.as_str()) {
                        //     Some(func) => {
                        //         tracing::info!("Casting {} to {}", CLS::CLASS_NAME, target);
                        //         (*func)(ctx, this)
                        //     }
                        //     None => {
                        //         tracing::error!(
                        //             "No cast function from {} to {}",
                        //             CLS::CLASS_NAME,
                        //             target
                        //         );
                        //         Err(Error::new(
                        //             ctx,
                        //             format!(
                        //                 "No cast function from {} to {}",
                        //                 CLS::CLASS_NAME,
                        //                 target
                        //             ),
                        //         ))
                        //     }
                        // }
                        Result::<_>::Ok(Value::new_undefined(ctx))
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
        let casters = phase.map.get_mut(Class::<SUPER>::typekey()).unwrap();
        let map: &mut CasterMap<SUPER> = casters.downcast_mut().unwrap();
        map.insert(Class::<CLS>::typekey(), smallbox!(caster_fn::<CLS, SUPER>));

        build_class!(SUPER with InitSuperCaster::<CLS, SUPER>::new(phase.map));
        Ok(phase)
    }
}

fn caster_fn<'js, BASE: Type + Cast<CLS>, CLS>(
    ctx: Ctx<'js>,
    rt: Rt,
    idx: Idx,
    func: CasterCallback<'js, CLS>,
) -> Result<Value<'js>> {
    let obj = rt.get_instance::<BASE>(idx).unwrap();
    let ptr = obj.cast();
    (func)(ctx, ptr.as_ref())
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
        let casters = phase.map.get_mut(Class::<NESTED>::typekey()).unwrap();
        let map: &mut CasterMap<NESTED> = casters.downcast_mut().unwrap();
        map.insert(Class::<CLS>::typekey(), smallbox!(caster_fn::<CLS, NESTED>));

        build_class!(NESTED with InitSuperCaster::<CLS, NESTED>::new(phase.map));
        Ok(phase)
    }
}
