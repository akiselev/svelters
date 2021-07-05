use crate::inner::*;

#[derive(Clone, Copy)]
pub struct ModuleLoad<'a, 'js: 'a> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
}

impl<'a, 'js> ModuleLoad<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self { ctx, module }
    }
}

#[derive(Clone, Copy)]
pub struct ModuleEval<'a, 'b: 'a> {
    pub(in crate) ctx: Ctx<'b>,
    pub(in crate) module: &'a QJsModule<'b, Loaded<Native>>,
}

impl<'a, 'js> ModuleEval<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Loaded<Native>>) -> Self {
        Self { ctx, module }
    }
}

pub struct JsModuleLoader<FW, MOD>(MOD, PhantomData<FW>);

impl<'a, 'b: 'a, 'aa, 'bb: 'aa, FW, MOD> ModuleDef for JsModuleLoader<FW, MOD>
where
    FW: Framework,
    MOD: Module<FW>,
{
    fn load<'js>(ctx: Ctx<'js>, module: &QJsModule<'js, Created>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleLoad<'_, '_>>(
            ModuleLoad::<'_, '_>::new(ctx, module),
            PhantomData,
        );
        <MOD as Module<FW>>::define::<ModuleLoad<'_, '_>>(_mod)
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &QJsModule<'js, Loaded<Native>>) -> Result<()> {
        let _mod = JsModule::<FW, MOD, ModuleEval<'_, '_>>(
            ModuleEval::<'_, '_>::new(ctx, exports),
            PhantomData,
        );
        <MOD as Module<FW>>::define::<ModuleEval<'_, '_>>(_mod)
    }
}

impl<FW: Framework, MOD: Module<FW>> JsModuleLoader<FW, MOD> {
    pub fn new(module: MOD) -> Self {
        Self(module, PhantomData)
    }
}

pub struct JsClassLoader<FW, CLS>(pub(in crate) PhantomData<(FW, CLS)>);

impl<FW: TypeDef<CLS>, CLS> JsClassLoader<FW, CLS> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a, 'b: 'a, FW, CLS> Implements<ModuleEval<'a, 'b>> for JsClassLoader<FW, CLS>
where
    FW: TypeDef<CLS> + Wraps<CLS>,
{
    fn build(self, phase: &ModuleEval<'a, 'b>) -> Result<()> {
        let cls: Type<FW, CLS, ConstructorEval<'a, 'b, FW, CLS>> =
            Type::new(ConstructorEval::new(phase.ctx, phase.module));
        <FW as TypeDef<CLS>>::define(cls)?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS> Implements<ModuleLoad<'a, 'b>> for JsClassLoader<FW, CLS>
where
    FW: TypeDef<CLS> + Wraps<CLS>,
{
    fn build(self, phase: &ModuleLoad<'a, 'b>) -> Result<()> {
        let cls: Type<FW, CLS, ConstructorLoad<'a, 'b, FW, CLS>> =
            Type::new(ConstructorLoad::new(phase.ctx, phase.module));
        <FW as TypeDef<CLS>>::define(cls)?;
        Ok(())
    }
}
