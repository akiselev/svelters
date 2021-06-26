use crate::inner::*;

pub(in crate) mod constructor;
pub(in crate) mod method;
pub(in crate) mod static_fn;
pub(in crate) mod impls {
    pub(in crate) mod constructor;
    pub(in crate) mod method;
    pub(in crate) mod static_fn;
}

pub struct JsClass<FW: Framework, const CLS: &'static str>(pub(in crate) PhantomData<FW>);

impl<FW: Framework, const CLS: &'static str> JsClass<FW, CLS> {
    pub const fn constructor<A, R, F>(&self, func: F) -> ClassConstructor<FW, CLS, A, R, F>
    where
        FW: Container<CLS>,
        F: Constructor<FW, CLS, A, R>,
    {
        ClassConstructor::new(func)
    }

    pub const fn method<A, R, F>(
        &self,
        name: &'static str,
        func: F,
    ) -> ClassMethod<FW, CLS, A, R, F>
    where
        FW: Container<CLS>,
        F: Method<FW, CLS, A, R>,
    {
        ClassMethod::new(name, func)
    }

    pub const fn static_fn<A, R, F>(
        &self,
        name: &'static str,
        func: F,
    ) -> ClassStaticFunc<FW, CLS, A, R, F>
    where
        FW: Container<CLS>,
        F: StaticFunc<FW, CLS, A, R>,
    {
        ClassStaticFunc::new(name, func)
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
        println!("Initializing {}", CLS);
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
    fn init_proto<'js>(&'static self, ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        self.tail.init_proto(ctx, proto)?;
        self.head.init_proto(ctx, proto)?;
        Ok(())
    }
}
impl<H, T> ClassStatic for HCons<H, T>
where
    H: ClassStatic,
    T: ClassStatic,
{
    fn init_static<'js>(&'static self, ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        self.tail.init_static(ctx, proto)?;
        self.head.init_static(ctx, proto)?;
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
