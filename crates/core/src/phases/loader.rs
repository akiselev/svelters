use crate::{build_class, inner::*};

#[derive(Clone)]
pub struct ModuleLoad<'a, 'js: 'a> {
    pub(in crate) rt: Rt,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
}

impl<'a, 'js> ModuleLoad<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, rt: Rt, module: &'a QJsModule<'js, Created>) -> Self {
        Self { rt, ctx, module }
    }
}

pub struct ModuleEval<'a, 'b, 'js> {
    pub(in crate) rt: Rt,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) doc: &'b mut Document,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
}

impl<'a, 'b, 'js> ModuleEval<'a, 'b, 'js> {
    pub fn new(
        ctx: Ctx<'js>,
        rt: Rt,
        module: &'a QJsModule<'js, Loaded<Native>>,
        doc: &'b mut Document,
    ) -> Self {
        Self {
            rt,
            ctx,
            doc,
            module,
        }
    }
}

pub struct JsModuleLoader<MOD>(MOD);

impl<MOD> ModuleDef for JsModuleLoader<MOD>
where
    MOD: Module,
{
    fn load<'js>(ctx: Ctx<'js>, module: &QJsModule<'js, Created>) -> Result<()> {
        let doc: &Document = ctx.globals().get("document")?;
        let rt = doc.rt.clone();
        let mut _mod = ModuleBuilder::<MOD, ModuleLoad<'_, '_>>(
            ModuleLoad::<'_, '_>::new(ctx, rt, module),
            PhantomData,
        );
        <MOD as Module>::define::<ModuleLoad<'_, '_>>(_mod)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &QJsModule<'js, Loaded<Native>>) -> Result<()> {
        let doc: &mut Document = ctx.globals().get("document")?;
        let rt = doc.rt.clone();
        let mut _mod = ModuleBuilder::<MOD, ModuleEval<'_, '_, '_>>(
            ModuleEval::<'_, '_, '_>::new(ctx, rt, exports, doc),
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
        build_class!(CLS with ConstructorEval::<'a, '_, 'c, CLS>::new(phase.rt.clone(), phase.ctx, phase.module, phase.doc));
        Ok(phase)
    }
}

impl<'a, 'b: 'a, CLS> Implements<ModuleLoad<'a, 'b>> for JsClassLoader<CLS>
where
    CLS: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: ModuleLoad<'a, 'b>) -> Result<ModuleLoad<'a, 'b>> {
        let key = phase.rt.register_type::<CLS>(phase.ctx);
        unsafe { CLS::metadata().init(key)? };
        build_class!(CLS with ConstructorLoad::<'a, 'b, CLS>::new(phase.ctx, phase.module));
        Ok(phase)
    }
}
