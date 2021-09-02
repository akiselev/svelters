use crate::{build_class, inner::*};

pub struct InitSuperCaster<'a, FW: Framework, CLS: Type<FW> + Repr<FW>, SUPER> {
    pub(in crate) map: &'a mut MetaFuncMap<FW, CLS>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, FW: Framework, CLS: Type<FW> + Repr<FW>, SUPER> InitSuperCaster<'a, FW, CLS, SUPER> {
    pub fn new(map: &'a mut MetaFuncMap<FW, CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, FW: Framework, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, FW, CLS, SUPER>>
    for Superclass<FW, SUPER, NESTED>
where
    CLS: Type<FW> + Repr<FW>,
    SUPER: Type<FW>,
    NESTED: Type<FW>,
    ReprOf<FW, CLS>: Cast<NESTED>,
    for<'x> CastedAs<'x, FW, CLS, NESTED>: IntoRepr<FW, NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperCaster<'a, FW, CLS, SUPER>) -> Result<()> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<FW, CLS> = this.as_ref();
                let data = obj.inner.cast();
                Result::<_>::Ok(
                    Virt::<'_, FW, NESTED>::new(this.clone().into_js(ctx)?, data.into_repr())
                        .into_js(ctx)?,
                )
            }),
        );

        build_class!(FW::NESTED with NestedSuperCaster::<'_,  FW, CLS, SUPER, NESTED>::new(phase.map));
        Ok(())
    }
}
