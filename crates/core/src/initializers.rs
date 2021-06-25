use crate::inner::*;

pub struct JsModule<FW: Framework, const T: &'static str>(pub(in crate) PhantomData<FW>);
pub struct JsClass<FW: Framework, const T: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW, const T: &'static str> ModuleDef for JsModule<FW, T>
where
    FW: Framework + FrameworkModule<T>,
    <FW as FrameworkModule<T>>::Output: JsModuleDef,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        <<FW as FrameworkModule<T>>::Output as JsModuleDef>::load(ctx, module)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        <<FW as FrameworkModule<T>>::Output as JsModuleDef>::eval(ctx, module)?;
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

    fn eval<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        <T as JsModuleDef>::eval(ctx, module)?;
        <H as JsModuleDef>::eval(ctx, module)?;
        Ok(())
    }
}

impl JsModuleDef for HNil {
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        Ok(())
    }
}

impl<H, T> JsClassDef for HCons<H, T>
where
    H: JsClassDef,
    T: JsClassDef,
{
    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

impl JsClassDef for HNil {
    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

impl<FW, const T: &'static str> JsModuleDef for JsClass<FW, T>
where
    FW: Framework + FrameworkClass<T>,
    Obj<FW, T>: ClassDef,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        module.add(T)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        <FW as FrameworkClass<T>>::register(ctx, module)?;
        Ok(())
    }
}
