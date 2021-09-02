use crate::{build_class, inner::*};

pub struct InitSuperCaster<'a, CLS: Type, SUPER> {
    pub(in crate) map: &'a mut MetaFuncMap<CLS>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, CLS: Type, SUPER> InitSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut MetaFuncMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Clone + Type,
    CLS: Caster<NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperCaster<'a, CLS, SUPER>) -> Result<()> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                println!(
                    "Casting nested {} to {} via {}\n\n",
                    CLS::CLASS_NAME,
                    NESTED::CLASS_NAME,
                    SUPER::CLASS_NAME
                );
                obj.inner.with_cast(|data| {
                    Result::<_>::Ok(
                        Virt::<'_, NESTED>::new(this.clone().into_js(ctx)?, (*data).clone())
                            .into_js(ctx)?,
                    )
                })
            }),
        );

        build_class!(NESTED with InitSuperCaster::<'_, CLS, NESTED>::new(phase.map));
        Ok(())
    }
}
