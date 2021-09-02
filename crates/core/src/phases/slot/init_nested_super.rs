use crate::{build_class, inner::*};

pub struct NestedSuperCaster<'a, ROOT: Type + Repr, CLS, SUPER> {
    pub(in crate) map: &'a mut MetaFuncMap<ROOT>,
    pub(in crate) framework: PhantomData<(ROOT, CLS, SUPER)>,
}

impl<'a, CLS, ROOT: Type + Repr, SUPER> NestedSuperCaster<'a, ROOT, CLS, SUPER> {
    pub fn new(map: &'a mut MetaFuncMap<ROOT>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, CLS, INTER, ROOT, SUPER> Implements<NestedSuperCaster<'a, ROOT, CLS, INTER>>
    for Superclass<SUPER>
where
    CLS: Type,
    ROOT: Type + Repr,
    ReprOf<ROOT>: Caster<SUPER>,
    for<'x> &'x SUPER: IntoRepr<SUPER>,
    INTER: Type,
    SUPER: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperCaster<'a, ROOT, CLS, INTER>) -> Result<()> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<ROOT> = this.as_ref();
                obj.inner.with_cast(|data| {
                    Result::<_>::Ok(
                        Virt::<'_, SUPER>::new(this.clone().into_js(ctx)?, data.into_repr())
                            .into_js(ctx)?,
                    )
                })
            }),
        );

        build_class!(SUPER with NestedSuperCaster::<'_, ROOT, INTER, SUPER>::new(phase.map));

        Ok(())
    }
}
