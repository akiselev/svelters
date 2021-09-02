use std::{collections::HashMap, lazy::OnceCell};

use rquickjs::This;

use crate::{build_class, inner::*};

pub struct InitCaster<'a, FW: Framework, CLS: Type<FW> + Repr<FW>> {
    pub(in crate) map: &'a mut MetaFuncMap<FW, CLS>,
    pub(in crate) framework: PhantomData<(CLS)>,
}

impl<'a, FW: Framework, CLS: Type<FW> + Repr<FW>> InitCaster<'a, FW, CLS> {
    pub fn new(map: &'a mut MetaFuncMap<FW, CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Framework, CLS> Implements<InitProto<'a, 'js, FW, CLS>> for JsClassLoader<FW, CLS>
where
    CLS: Type<FW> + Repr<FW>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitProto<'a, 'js, FW, CLS>) -> Result<()> {
        println!("Calling InitCaster for {}", CLS::CLASS_NAME);
        let cell = OnceCell::new();
        phase.obj.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Obj<FW, CLS>>, ctx: Ctx<'js>, target: String| {
                        let map = cell.get_or_init(|| {
                            let mut map: MetaFuncMap<FW, CLS> = HashMap::new();
                            println!("Initializing meta map for {}", CLS::CLASS_NAME);
                            map.insert(CLS::CLASS_NAME, Box::new(|ctx, value| value.into_js(ctx)));
                            {
                                build_class!(FW::CLS with InitCaster::<FW, CLS>::new(&mut map))
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

impl<'a, FW: Framework, CLS, SUPER> Implements<InitCaster<'a, FW, CLS>>
    for Superclass<FW, CLS, SUPER>
where
    CLS: Type<FW> + Repr<FW>,
    SUPER: Type<FW>,
    ReprOf<FW, CLS>: Cast<SUPER>,
    for<'x> CastedAs<'x, FW, CLS, SUPER>: IntoRepr<FW, SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<FW, CLS>) -> Result<()> {
        println!(
            "Calling SUPERINITCASTER for {} of {}",
            CLS::CLASS_NAME,
            SUPER::CLASS_NAME
        );

        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<FW, CLS> = this.as_ref();
                let data = obj.inner.cast();
                Result::<_>::Ok(
                    Virt::<'_, FW, SUPER>::new(this.clone().into_js(ctx)?, data.into_repr())
                        .into_js(ctx)?,
                )
            }),
        );

        build_class!(FW::CLS with InitSuperCaster::<'_,  FW, CLS, SUPER>::new(phase.map));

        Ok(())
    }
}
