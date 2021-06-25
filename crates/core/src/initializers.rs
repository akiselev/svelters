use crate::inner::*;

pub struct JsModule<T>(T);

impl<T> JsModule<T> {
    pub fn new(init: T) -> JsModule<T> {
        JsModule(init)
    }
}

pub trait JsModuleDef {
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    fn eval<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Loaded<Native>>) -> Result<()> {
        Ok(())
    }
}

impl<H, T> ModuleDef for JsModule<HCons<H, T>>
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
