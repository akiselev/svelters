use crate::inner::*;

pub struct ModuleBuilder<MOD, P>(pub(crate) P, pub(crate) PhantomData<MOD>);

impl<MOD, P> ModuleBuilder<MOD, P> {
    #[inline(always)]
    pub fn class<CLS>(self) -> Result<Self>
    where
        CLS: Type,
        JsClassLoader<CLS>: Implements<P>,
    {
        let phase = JsClassLoader(PhantomData).build_phase(self.0)?;
        Ok(ModuleBuilder(phase, PhantomData))
    }
}
