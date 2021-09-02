use std::{collections::HashMap, lazy::OnceCell};

use rquickjs::This;

use crate::{build_class, inner::*};

pub struct InitCaster<'a, CLS: Type + Repr> {
    pub(in crate) map: &'a mut MetaFuncMap<CLS>,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, CLS: Type + Repr> InitCaster<'a, CLS> {
    pub fn new(map: &'a mut MetaFuncMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, CLS> Implements<InitProto<'a, 'js, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Repr,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'js, CLS>) -> Result<()> {
        println!("Calling InitCaster for {}", CLS::CLASS_NAME);
        let cell = OnceCell::new();
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<CLS>>, ctx: Ctx<'js>, target: String| {
                        let map = cell.get_or_init(|| {
                            let mut map: MetaFuncMap<CLS> = HashMap::new();
                            println!("Initializing meta map for {}", CLS::CLASS_NAME);
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

impl<'a, CLS, SUPER> Implements<InitCaster<'a, CLS>> for Superclass<CLS, SUPER>
where
    CLS: Type + Repr,
    SUPER: Type,
    ReprOf<CLS>: Cast<SUPER>,
    for<'x> CastedAs<'x, CLS, SUPER>: IntoRepr<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<CLS>) -> Result<()> {
        println!(
            "Calling SUPERINITCASTER for {} of {}",
            CLS::CLASS_NAME,
            SUPER::CLASS_NAME
        );

        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let data = obj.inner.cast();
                Result::<_>::Ok(
                    Virt::<'_, SUPER>::new(this.clone().into_js(ctx)?, data.into_repr())
                        .into_js(ctx)?,
                )
            }),
        );

        build_class!(CLS with InitSuperCaster::<'_, CLS, SUPER>::new(phase.map));

        Ok(())
    }
}
