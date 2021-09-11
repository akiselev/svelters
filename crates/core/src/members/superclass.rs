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
        build_class!(SUPER with InitProto::<'_, '_, CLS>::new(phase.ctx, phase.obj));
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
        build_class!(SUPER with InitStatic::<'_, '_, BASE>::new(phase.ctx, phase.obj));
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
        build_class!(SUPER with InitType::<'_, CLS>::new(phase.element));
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
        build_class!(SUPER with InitElementProto::<'a, 'b, 'c, CLS>::new(phase.ctx, phase.obj));
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
        build_class!(SUPER with InitVirt::<'_, '_, CLS>::new(phase.ctx, phase.obj));
        Ok(phase)
    }
}
