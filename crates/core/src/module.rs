use crate::inner::*;

pub struct ModuleBuilder<MOD, P>(pub(crate) P, pub(crate) PhantomData<(MOD)>);

impl<MOD, P> ModuleBuilder<MOD, P> {
    #[inline(always)]
    pub fn class<CLS>(&mut self) -> Result<()>
    where
        CLS: Type,
        JsClassLoader<CLS>: Implements<P>,
    {
        JsClassLoader(PhantomData).build(&mut self.0)?;
        Ok(())
    }
}
