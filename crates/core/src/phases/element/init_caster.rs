use std::{collections::HashMap, lazy::OnceCell};

use crate::{build_class, inner::*};

pub struct InitElementCaster<'a, CLS: Type> {
    pub(in crate) map: &'a mut ElementCasterMap,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'b, CLS: Type> InitElementCaster<'a, CLS> {
    pub fn new(map: &'a mut ElementCasterMap) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, CLS, SUPER> Implements<InitElementCaster<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type + Clone,
    CLS: Caster<SUPER>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElementCaster<CLS>) -> Result<()> {
        phase.map.insert(
            SUPER::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj = Obj::<CLS>::from_js_ref(ctx, this.inner.as_value().clone())?;
                obj.inner.with_cast(|data| {
                    let virt = Virt::<SUPER>::new((*data).clone());
                    let value = virt.into_js(ctx)?;
                    Result::<_>::Ok(value)
                })
            }),
        );

        build_class!(SUPER with InitElementSuperCaster::<'_, CLS, SUPER>::new(phase.map));

        Ok(())
    }
}

pub struct InitElementSuperCaster<'a, CLS: Type, SUPER> {
    pub(in crate) map: &'a mut ElementCasterMap,
    pub(in crate) framework: PhantomData<(CLS, SUPER)>,
}

impl<'a, CLS: Type, SUPER> InitElementSuperCaster<'a, CLS, SUPER> {
    pub fn new(map: &'a mut ElementCasterMap) -> Self {
        Self {
            map,
            framework: PhantomData,
        }
    }
}

impl<'a, CLS, SUPER, NESTED> Implements<InitElementSuperCaster<'a, CLS, SUPER>>
    for members::Superclass<NESTED>
where
    CLS: Type,
    SUPER: Type,
    NESTED: Clone + Type,
    CLS: Caster<NESTED>,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElementSuperCaster<'a, CLS, SUPER>) -> Result<()> {
        phase.map.insert(
            NESTED::CLASS_NAME,
            Box::new(|ctx, this| {
                let obj = Obj::<CLS>::from_js_ref(ctx, this.inner.as_value().clone())?;
                obj.inner.with_cast(|data| {
                    Result::<_>::Ok(Virt::<NESTED>::new((*data).clone()).into_js(ctx)?)
                })
            }),
        );

        build_class!(NESTED with InitElementSuperCaster::<'_, CLS, NESTED>::new(phase.map));
        Ok(())
    }
}
