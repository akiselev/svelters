use crate::{build_class, inner::*};

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

pub struct JsModuleLoader<MOD>(MOD);

impl<'a, 'b: 'a, 'aa, 'bb: 'aa, MOD> ModuleDef for JsModuleLoader<MOD>
where
    MOD: Module,
{
    fn load<'js>(ctx: Ctx<'js>, module: &QJsModule<'js, Created>) -> Result<()> {
        let mut _mod = ModuleBuilder::<MOD, ModuleLoad<'_, '_>>(
            ModuleLoad::<'_, '_>::new(ctx, module),
            PhantomData,
        );
        <MOD as Module>::define::<ModuleLoad<'_, '_>>(&mut _mod)
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &QJsModule<'js, Loaded<Native>>) -> Result<()> {
        let mut _mod = ModuleBuilder::<MOD, ModuleEval<'_, '_>>(
            ModuleEval::<'_, '_>::new(ctx, exports),
            PhantomData,
        );
        <MOD as Module>::define::<ModuleEval<'_, '_>>(&mut _mod)
    }
}

impl<MOD> JsModuleLoader<MOD>
where
    MOD: Module,
{
    pub fn new(module: MOD) -> Self {
        Self(module)
    }
}

pub struct JsClassLoader<CLS>(pub(in crate) PhantomData<(CLS)>);

impl<CLS> JsClassLoader<CLS>
where
    CLS: Type,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a, 'b: 'a, CLS> Implements<ModuleEval<'a, 'b>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut ModuleEval<'a, 'b>) -> Result<()> {
        unsafe { CLS::metadata().init()? };
        build_class!(CLS with ConstructorEval::<'a, 'b, CLS>::new(phase.ctx, phase.module));
        Ok(())
    }
}

impl<'a, 'b: 'a, CLS> Implements<ModuleLoad<'a, 'b>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut ModuleLoad<'a, 'b>) -> Result<()> {
        build_class!(CLS with ConstructorLoad::<'a, 'b, CLS>::new(phase.ctx, phase.module));
        Ok(())
    }
}

impl<T, P> Implements<P> for JsClassLoader<T> {
    #[inline(always)]
    default fn build(self, _phase: &mut P) -> Result<()> {
        Ok(())
    }
}

impl<T, P> Implements<P> for JsModuleLoader<T> {
    #[inline(always)]
    default fn build(self, _phase: &mut P) -> Result<()> {
        Ok(())
    }
}
