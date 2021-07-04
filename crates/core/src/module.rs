use crate::inner::*;

pub struct JsModule<FW, MOD, P>(P, PhantomData<(FW, MOD)>);

impl<FW, MOD, P> JsModule<FW, MOD, P> {
    pub fn class<CLS>(&self) -> Result<()>
    where
        CLS: 'static,
        FW: FrameworkClass<CLS>,
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        println!(">>>>>>>>>>>>>>>xx");
        JsClassLoader(PhantomData).build(&self.0)?;
        Ok(())
    }
}

pub struct JsModuleLoader<FW, MOD>(MOD, PhantomData<FW>);

impl<'a, 'b: 'a, 'aa, 'bb: 'aa, FW, MOD> ModuleDef for JsModuleLoader<FW, MOD>
where
    FW: Framework,
    MOD: FrameworkModule<FW>,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleLoad<'_, '_>>(
            ModuleLoad::<'_, '_>::new(ctx, module),
            PhantomData,
        );
        <MOD as FrameworkModule<FW>>::define::<ModuleLoad<'_, '_>>(_mod)
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &Module<'js, Loaded<Native>>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleEval<'_, '_>>(
            ModuleEval::<'_, '_>::new(ctx, exports),
            PhantomData,
        );
        <MOD as FrameworkModule<FW>>::define::<ModuleEval<'_, '_>>(_mod)
    }
}

impl<FW: Framework, MOD: FrameworkModule<FW>> JsModuleLoader<FW, MOD> {
    pub fn new(module: MOD) -> Self {
        Self(module, PhantomData)
    }
}
