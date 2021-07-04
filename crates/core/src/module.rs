use crate::inner::*;

pub struct JsModule<'a, 'b: 'a, FW: FrameworkModule<MOD>, const MOD: &'static str, P: Phase>(
    P::Input<'a, 'b>,
    pub(in crate) PhantomData<FW>,
);

impl<'a, 'b: 'a, FW: FrameworkModule<MOD>, P: 'static + Phase, const MOD: &'static str>
    JsModule<'a, 'b, FW, MOD, P>
{
    pub fn class<const CLS: &'static str>(&self) -> Result<()>
    where
        FW: FrameworkClass<CLS>,
        JsClassLoader<FW, CLS>: Implements<P>,
    {
        JsClassLoader(PhantomData).build(self.0)
    }
}

pub struct JsModuleLoader<FW, const MOD: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW, const MOD: &'static str> ModuleDef for JsModuleLoader<FW, MOD>
where
    FW: FrameworkModule<MOD>,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleLoad>((ctx, module), PhantomData);
        <FW as FrameworkModule<MOD>>::define(_mod)
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &Module<'js, Loaded<Native>>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleEval>((ctx, exports), PhantomData);
        <FW as FrameworkModule<MOD>>::define(_mod)
    }
}

impl<FW: FrameworkModule<MOD>, const MOD: &'static str> JsModuleLoader<FW, MOD> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
