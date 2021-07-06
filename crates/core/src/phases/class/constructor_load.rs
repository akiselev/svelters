use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW> ConstructorLoad<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ConstructorLoad<'a, 'b, FW, CLS>>
    for Constructor<FW, CLS, (), R, T>
where
    FW: Framework,
    CLS: Type<FW>,
    T: Fn() -> R,
{
    #[inline(always)]
    fn build(self, phase: &mut ConstructorLoad<'a, 'b, FW, CLS>) -> Result<()> {
        phase.module.add(<CLS as Type<FW>>::name())?;
        Ok(())
    }
}
