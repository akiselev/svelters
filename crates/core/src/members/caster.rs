use crate::inner::*;

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'js, CLS>) -> Result<InitProto<'a, 'js, CLS>> {
        let cell = OnceCell::new();
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let map = cell.get_or_try_init(|| {
                            tracing::info!("Building caster for {}", CLS::CLASS_NAME);
                            let mut map: ObjCasterMap<CLS> = HashMap::new();
                            map.insert(
                                CLS::CLASS_NAME,
                                Box::new(|ctx, value| {
                                    tracing::info!("Casting {} to self", CLS::CLASS_NAME);
                                    let obj: &Obj<CLS> = value.as_ref();
                                    Virt::<CLS>::new(
                                        obj.rt.clone(),
                                        obj.idx,
                                        value.as_object().clone(),
                                    )
                                    .into_js(ctx)
                                }),
                            );
                            {
                                build_class!(CLS with InitCaster::<CLS>::new(&mut map))
                            };
                            Result::<_>::Ok(map)
                        })?;
                        match map.get(target.as_str()) {
                            Some(func) => {
                                tracing::info!("Casting {} to {}", CLS::CLASS_NAME, target);
                                (*func)(ctx, this)
                            }
                            None => {
                                tracing::error!(
                                    "No cast function from {} to {}",
                                    CLS::CLASS_NAME,
                                    target
                                );
                                Err(Error::new(
                                    ctx,
                                    format!(
                                        "No cast function from {} to {}",
                                        CLS::CLASS_NAME,
                                        target
                                    ),
                                ))
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
    CLS: Type,
    SUPER: Type + Clone,
    CLS: Cast<SUPER>,
{
    #[inline(always)]
    fn build_phase(self, phase: InitCaster<CLS>) -> Result<InitCaster<CLS>> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let virt = Virt::<SUPER>::new(obj.rt.clone(), obj.idx, this.into_object());
                virt.into_js(ctx)
            }),
        );

        build_class!(SUPER with InitSuperCaster::<'_, CLS, SUPER>::new(phase.map));

        Ok(phase)
    }
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
        phase: InitSuperCaster<'a, CLS, SUPER>,
    ) -> Result<InitSuperCaster<'a, CLS, SUPER>> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let this = this.clone();
                let ptr = obj.borrow().unwrap();
                Virt::<NESTED>::new(obj.rt.clone(), obj.idx, this).into_js(ctx)
            }),
        );

        build_class!(NESTED with InitSuperCaster::<'_, CLS, NESTED>::new(phase.map));
        Ok(phase)
    }
}
