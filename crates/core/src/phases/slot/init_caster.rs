use std::{collections::HashMap, lazy::OnceCell};

use crate::{build_class, inner::*};

pub struct InitCaster<'a, CLS: Type> {
    pub(in crate) map: &'a mut MetaFuncMap<CLS>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, CLS: Type> InitCaster<'a, CLS> {
    pub fn new(map: &'a mut MetaFuncMap<CLS>) -> Self {
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
                            map.insert(
                                CLS::CLASS_NAME,
                                Box::new(|ctx, value| {
                                    println!("Casting self {} \n\n", CLS::CLASS_NAME);
                                    println!("Casting self {} \n\n", CLS::CLASS_NAME);

                                    value.into_js(ctx)
                                }),
                            );
                            {
                                build_class!(CLS with InitCaster::<CLS>::new(&mut map))
                            };
                            println!("{:?}", map.keys().collect::<Vec<_>>());
                            println!("{:?}", map.keys().collect::<Vec<_>>());
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

impl<'a, CLS, SUPER> Implements<InitCaster<'a, CLS>> for Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type + Clone,
    CLS: Caster<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitCaster<CLS>) -> Result<()> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                obj.inner.with_cast(|data| {
                    let virt = Virt::<SUPER>::new((*data).clone());
                    let value = virt.into_js(ctx)?;
                    Result::<_>::Ok(value)
                })
            }),
        );

        build_class!(SUPER with InitSuperCaster::<'_, CLS, SUPER>::new(phase.map));

        Ok(())
    }
}
