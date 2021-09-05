use std::{collections::HashMap, lazy::OnceCell};

use crate::{build_class, inner::*};

pub struct InitCaster<'a, CLS: Type> {
    pub(in crate) map: &'a mut ObjCasterMap<CLS>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, CLS: Type> InitCaster<'a, CLS> {
    pub fn new(map: &'a mut ObjCasterMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut InitProto<'a, 'js, CLS>) -> Result<()> {
        let cell = OnceCell::new();
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let map = cell.get_or_init(|| {
                            tracing::info!("Building caster for {}", CLS::CLASS_NAME);
                            let mut map: ObjCasterMap<CLS> = HashMap::new();
                            map.insert(CLS::CLASS_NAME, Box::new(|ctx, value| value.into_js(ctx)));
                            {
                                build_class!(CLS with InitCaster::<CLS>::new(&mut map))
                            };
                            Result::Ok(map)
                        });
                        let map = match map {
                            Ok(map) => Ok(map),
                            Err(err) => Err(err.clone()),
                        }?;
                        match map.get(target.as_str()) {
                            Some(func) => (*func)(ctx, this),
                            None => Err(Error::new(
                                ctx,
                                format!("No cast function from {} to {}", CLS::CLASS_NAME, target),
                            )),
                        }
                    },
                ),
            ),
        )?;

        Ok(())
    }
}

impl<'a, CLS, SUPER> Implements<InitCaster<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type + Clone,
    CLS: Caster<SUPER>,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut InitCaster<CLS>) -> Result<()> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let this = this.clone();
                obj.inner.with_cast(move |data| {
                    let virt = Virt::<SUPER>::new((*data).clone(), this);
                    let value = virt.into_js(ctx)?;
                    Result::<_>::Ok(value)
                })
            }),
        );

        build_class!(SUPER with InitSuperCaster::<'_, CLS, SUPER>::new(phase.map));

        Ok(())
    }
}
