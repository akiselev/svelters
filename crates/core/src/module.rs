use crate::inner::*;

pub struct JsModule<FW: FrameworkModule<MOD>, MOD, P: Phase>(
    P,
    pub(in crate) PhantomData<(FW, MOD)>,
);

impl<FW: FrameworkModule<MOD>, P: Phase, MOD> JsModule<FW, MOD, P> {
    pub fn class<CLS>(&self) -> Result<()>
    where
        FW: FrameworkClass<CLS>,
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        // JsClassLoader(PhantomData).build(self.0)
        Ok(())
    }
}

pub struct JsModuleLoader<FW, MOD>(pub(in crate) PhantomData<(FW, MOD)>);

impl<FW, MOD> ModuleDef for JsModuleLoader<FW, MOD>
where
    FW: FrameworkModule<MOD>,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleLoad>(ModuleLoad::new(ctx, module), PhantomData);
        <FW as FrameworkModule<MOD>>::define(_mod)
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &Module<'js, Loaded<Native>>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleEval>(ModuleEval::new(ctx, exports), PhantomData);
        <FW as FrameworkModule<MOD>>::define(_mod)
    }
}

impl<FW: FrameworkModule<MOD>, MOD> JsModuleLoader<FW, MOD> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
