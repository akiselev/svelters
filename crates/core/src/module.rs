use crate::inner::*;

pub struct ModuleBuilder<FW, MOD, P>(pub(crate) P, pub(crate) PhantomData<(FW, MOD)>);

impl<FW, MOD, P> ModuleBuilder<FW, MOD, P> {
    #[inline(always)]
    pub fn class<CLS>(&mut self) -> Result<()>
    where
        CLS: Type<FW>,
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        JsClassLoader(PhantomData).build(&mut self.0)?;
        Ok(())
    }
}
