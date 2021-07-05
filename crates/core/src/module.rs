use crate::inner::*;

pub struct JsModule<FW, MOD, P>(pub(crate) P, pub(crate) PhantomData<(FW, MOD)>);

impl<FW, MOD, P> JsModule<FW, MOD, P> {
    pub fn class<CLS>(&self) -> Result<()>
    where
        CLS: 'static,
        FW: TypeDef<CLS> + Wraps<CLS>,
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        println!(">>>>>>>>>>>>>>>xx");
        JsClassLoader(PhantomData).build(&self.0)?;
        Ok(())
    }
}
