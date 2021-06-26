use crate::inner::*;

pub struct JsClass<FW: Framework, const CLS: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW: Framework, const CLS: &'static str> JsClass<FW, CLS> {
    pub const fn constructor<A, R, F>(&self, func: F) -> ClassConstructor<FW, CLS, A, R, F>
    where
        FW: Container<CLS>,
        F: Constructor<FW, CLS, A, R>,
    {
        ClassConstructor::new(func)
    }
}

impl<FW: Framework, const CLS: &'static str> JsModuleDef for JsClass<FW, CLS>
where
    FW: Container<CLS>,
    Obj<FW, CLS>: ClassDef,
{
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()> {
        <<FW as FrameworkClass<CLS>>::Constructor>::load(_ctx, _module)?;
        Ok(())
    }

    fn eval<'js>(
        &'static self,
        ctx: Ctx<'js>,
        exports: &Module<'js, Loaded<Native>>,
    ) -> Result<()> {
        rquickjs::Class::<Obj<FW, CLS>>::register(ctx)?;
        let def = <FW as FrameworkClass<CLS>>::get_constructor();
        def.eval(ctx, exports)?;
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
