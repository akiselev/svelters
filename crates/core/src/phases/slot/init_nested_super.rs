use crate::{build_class, inner::*};

pub struct NestedSuperCaster<'a, FW: Framework, ROOT: Type<FW> + Repr<FW>, CLS, SUPER> {
    pub(in crate) map: &'a mut MetaFuncMap<FW, ROOT>,
    pub(in crate) framework: PhantomData<(ROOT, CLS, SUPER)>,
}

impl<'a, FW: Framework, CLS, ROOT: Type<FW> + Repr<FW>, SUPER>
    NestedSuperCaster<'a, FW, ROOT, CLS, SUPER>
{
    pub fn new(map: &'a mut MetaFuncMap<FW, ROOT>) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, FW: Framework, CLS, INTER, ROOT, SUPER>
    Implements<NestedSuperCaster<'a, FW, ROOT, CLS, INTER>> for Superclass<FW, INTER, SUPER>
where
    CLS: Type<FW>,
    ROOT: Type<FW> + Repr<FW>,
    ReprOf<FW, ROOT>: Cast<SUPER>,
    for<'x> CastedAs<'x, FW, ROOT, SUPER>: IntoRepr<FW, SUPER>,
    INTER: Type<FW>,
    SUPER: Type<FW>,
{
    #[inline(always)]
    fn build(self, phase: &mut NestedSuperCaster<'a, FW, ROOT, CLS, INTER>) -> Result<()> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj: &Obj<FW, ROOT> = this.as_ref();
                let data = obj.inner.cast();
                Result::<_>::Ok(
                    Virt::<'_, FW, SUPER>::new(this.clone().into_js(ctx)?, data.into_repr())
                        .into_js(ctx)?,
                )
            }),
        );

        build_class!(FW::SUPER with NestedSuperCaster::<'_, FW, ROOT, INTER, SUPER>::new(phase.map));

        Ok(())
    }
}
