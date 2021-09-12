use crate::inner::*;

pub struct Superclass<SUPER>(PhantomData<SUPER>);

impl<SUPER> Superclass<SUPER> {
    pub(in crate) fn new() -> Self {
        Superclass(PhantomData)
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitProto<'a, 'b, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Upcast<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: InitProto<'a, 'b, CLS>) -> Result<InitProto<'a, 'b, CLS>> {
        build!(SUPER::InitProto::<'_, '_, CLS>[phase.ctx, phase.obj]);
        Ok(phase)
    }
}

impl<'a, 'b: 'a, BASE, SUPER> Implements<InitStatic<'a, 'b, BASE>> for members::Superclass<SUPER>
where
    BASE: Upcast<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: InitStatic<'a, 'b, BASE>) -> Result<InitStatic<'a, 'b, BASE>> {
        build!(SUPER::InitStatic::<'_, '_, BASE>[phase.ctx, phase.obj]);
        Ok(phase)
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitType<'a, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Upcast<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: InitType<'a, CLS>) -> Result<InitType<'a, CLS>> {
        phase.element.inherits::<SUPER>();
        build!(SUPER::InitType::<'_, CLS>[phase.element]);
        Ok(phase)
    }
}

impl<'a, 'b, 'c, CLS, SUPER> Implements<InitElementProto<'a, 'b, 'c, CLS>>
    for members::Superclass<SUPER>
where
    CLS: Type,
    SUPER: Type,
    CLS: Upcast<SUPER>,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitElementProto<'a, 'b, 'c, CLS>,
    ) -> Result<InitElementProto<'a, 'b, 'c, CLS>> {
        build!(SUPER::InitElementProto::<'a, 'b, 'c, CLS>[
            phase.ctx, phase.obj
        ]);
        Ok(phase)
    }
}

impl<'a, 'b: 'a, CLS, SUPER> Implements<InitVirt<'a, 'b, CLS>> for members::Superclass<SUPER>
where
    CLS: Type + Upcast<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: InitVirt<'a, 'b, CLS>) -> Result<InitVirt<'a, 'b, CLS>> {
        build!(SUPER::InitVirt::<'_, '_, CLS>[phase.ctx, phase.obj]);
        Ok(phase)
    }
}
