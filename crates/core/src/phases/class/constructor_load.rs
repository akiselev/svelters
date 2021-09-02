use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'js, CLS> ConstructorLoad<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, CLS, R> Implements<ConstructorLoad<'a, 'b, CLS>> for Constructor<CLS, (), T>
where
    CLS: Type,
    T: Fn() -> R,
{
    #[inline(always)]
    fn build(self, phase: &mut ConstructorLoad<'a, 'b, CLS>) -> Result<()> {
        phase.module.add(<CLS as Type>::name())?;
        Ok(())
    }
}
