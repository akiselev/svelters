use crate::inner::*;

pub struct JsModule<FW: FrameworkModule<MOD>, MOD, P>(P, pub(in crate) PhantomData<(FW, MOD)>);

impl<FW: FrameworkModule<MOD>, P, MOD> JsModule<FW, MOD, P> {
    pub fn class<CLS>(&self) -> Result<()>
    where
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        // JsClassLoader(PhantomData).build(self.0)
        Ok(())
    }
}

pub struct JsModuleLoader<FW, MOD, A>(pub(in crate) PhantomData<(FW, MOD, A)>);

impl<'a, 'b: 'a, FW, MOD, P1, P2> ModuleDef for JsModuleLoader<FW, MOD, (P1, P2)>
where
    FW: FrameworkModule<MOD>,
    JsModule<FW, MOD, P1>: for<'aa, 'bb> Implements<ModuleLoad<'aa, 'bb>>,
    JsModule<FW, MOD, P2>: for<'aa, 'bb> Implements<ModuleEval<'aa, 'bb>>,
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

impl<FW: FrameworkModule<MOD>, MOD, A> JsModuleLoader<FW, MOD, A> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
