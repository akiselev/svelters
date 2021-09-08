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

pub struct ModuleEval<'a, 'b, 'js> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) doc: &'b mut Document,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
}

impl<'a, 'b, 'js> ModuleEval<'a, 'b, 'js> {
    pub fn new(
        ctx: Ctx<'js>,
        module: &'a QJsModule<'js, Loaded<Native>>,
        doc: &'b mut Document,
    ) -> Self {
        Self { ctx, doc, module }
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
        <MOD as Module>::define::<ModuleLoad<'_, '_>>(_mod)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &QJsModule<'js, Loaded<Native>>) -> Result<()> {
        let globals = ctx.globals();
        let value: Value<'js> = globals.get("document")?;
        let document: &mut Document = Document::from_js_mut(ctx, value)?;
        let mut _mod = ModuleBuilder::<MOD, ModuleEval<'_, '_, '_>>(
            ModuleEval::<'_, '_, '_>::new(ctx, exports, document),
            PhantomData,
        );
        <MOD as Module>::define::<ModuleEval<'_, '_, '_>>(_mod)?;
        Ok(())
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

pub struct JsClassLoader<CLS>(pub(in crate) PhantomData<CLS>);

impl<CLS> JsClassLoader<CLS>
where
    CLS: Type,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a, 'b, 'c: 'a + 'b, CLS> Implements<ModuleEval<'a, 'b, 'c>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: ModuleEval<'a, 'b, 'c>) -> Result<ModuleEval<'a, 'b, 'c>> {
        unsafe { CLS::metadata().init()? };
        build_class!(CLS with ConstructorEval::<'a, '_, 'c, CLS>::new(phase.ctx, phase.module, phase.doc));
        Ok(phase)
    }
}

impl<'a, 'b: 'a, CLS> Implements<ModuleLoad<'a, 'b>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: ModuleLoad<'a, 'b>) -> Result<ModuleLoad<'a, 'b>> {
        build_class!(CLS with ConstructorLoad::<'a, 'b, CLS>::new(phase.ctx, phase.module));
        Ok(phase)
    }
}

// impl<T, P> Implements<P> for JsClassLoader<T> {
//     #[inline(always)]
//     default fn build_phase(self, _phase: &mut P) -> Result<()> {
//         Ok(())
//     }
// }

// impl<T, P> Implements<P> for JsModuleLoader<T> {
//     #[inline(always)]
//     default fn build_phase(self, _phase: &mut P) -> Result<()> {
//         Ok(())
//     }
// }
