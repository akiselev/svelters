use crate::inner::*;

pub struct JsModule<FW: Framework, const CLS: &'static str>(pub(in crate) PhantomData<FW>);
pub struct JsClass<FW: Framework, const CLS: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW: Framework, const CLS: &'static str> JsClass<FW, CLS> {
    pub fn constructor<A, R, F>(&self, func: F) -> ClassConstructor<FW, CLS, A, R, F>
    where
        FW: Container<CLS>,
        F: Constructor<FW, CLS, A, R>,
    {
        ClassConstructor::new(func)
    }
}

impl<'a, FW, const MOD: &'static str> ModuleDef for JsModule<FW, MOD>
where
    FW: Framework + FrameworkModule<MOD>,
    <FW as FrameworkModule<MOD>>::Output: JsModuleDef,
{
    fn load<'js>(ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        <<FW as FrameworkModule<MOD>>::Output as JsModuleDef>::load(ctx, module)?;
        Ok(())
    }

    fn eval<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        JsModule::<FW, MOD>::eval(ctx, module)?;
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

impl<H, T> ClassProto for HCons<H, T>
where
    H: ClassProto,
    T: ClassProto,
{
    fn init_proto<'js>(&'static self, _ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}
impl<H, T> ClassStatic for HCons<H, T>
where
    H: ClassStatic,
    T: ClassStatic,
{
    fn init_static<'js>(&'static self, _ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

impl ClassProto for HNil {
    fn init_proto<'js>(&'static self, _ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

impl ClassStatic for HNil {
    fn init_static<'js>(&'static self, _ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }
}
