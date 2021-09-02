use crate::{build_class, inner::*};

pub struct InitSuperCaster<'a, CLS: Type + Repr, SUPER> {
    pub(in crate) map: &'a mut MetaFuncMap<CLS>,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, CLS: Type + Repr, SUPER> InitSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut MetaFuncMap<CLS>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, CLS, SUPER, NESTED> Implements<InitSuperCaster<'a, CLS, SUPER>>
    for Superclass<SUPER, NESTED>
where
    CLS: Type + Repr,
    SUPER: Type,
    NESTED: Type,
    ReprOf<CLS>: Cast<NESTED>,
    for<'x> CastedAs<'x, CLS, NESTED>: IntoRepr<NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitSuperCaster<'a, CLS, SUPER>) -> Result<()> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<CLS> = this.as_ref();
                let data = obj.inner.cast();
                Result::<_>::Ok(
                    Virt::<'_, NESTED>::new(this.clone().into_js(ctx)?, data.into_repr())
                        .into_js(ctx)?,
                )
            }),
        );

        build_class!(NESTED with NestedSuperCaster::<'_, CLS, SUPER, NESTED>::new(phase.map));
        Ok(())
    }
}
