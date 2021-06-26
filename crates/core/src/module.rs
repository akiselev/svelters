use crate::inner::*;

pub struct JsModule<FW: Framework, const CLS: &'static str>(pub(in crate) PhantomData<FW>);

impl<'a, FW, const MOD: &'static str> ModuleDef for JsModule<FW, MOD>
where
    FW: Framework + FrameworkModule<MOD>,
    <FW as FrameworkModule<MOD>>::Output: JsModuleDef,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        <<FW as FrameworkModule<MOD>>::Output as JsModuleDef>::load(ctx, module)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, exports: &Module<'js, Loaded<Native>>) -> Result<()> {
        let module = <FW as FrameworkModule<MOD>>::get_module();
        module.eval(ctx, exports)?;
        Ok(())
    }
}

impl<H, T> JsModuleDef for HCons<H, T>
where
    H: JsModuleDef,
    T: JsModuleDef,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        <T as JsModuleDef>::load(ctx, module)?;
        <H as JsModuleDef>::load(ctx, module)?;
        Ok(())
    }

    fn eval<'js>(&'static self, ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        <T as JsModuleDef>::eval(&self.tail, ctx, module)?;
        <H as JsModuleDef>::eval(&self.head, ctx, module)?;
        Ok(())
    }
}

impl JsModuleDef for HNil {
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    fn eval<'js>(&'static self, ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        Ok(())
    }
}
