use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, FW: Container<CLS>, const CLS: &'static str> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Created>,
    framework: PhantomData<FW>,
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> ConstructorLoad<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> Phase
    for ConstructorLoad<'a, 'js, FW, CLS>
{
    type Error = Error;
}

pub struct ConstructorEval<'a, 'js, FW, const CLS: &'static str> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Loaded<Native>>,
    framework: PhantomData<FW>,
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> ConstructorEval<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Loaded<Native>>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> Phase
    for ConstructorEval<'a, 'js, FW, CLS>
{
    type Error = Error;
}

pub struct InitProto<'a, 'js, FW, const CLS: &'static str> {
    ctx: Ctx<'js>,
    obj: &'a Object<'js>,
    framework: PhantomData<FW>,
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> InitProto<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> Phase for InitProto<'a, 'js, FW, CLS> {
    type Error = Error;
}

pub struct InitStatic<'a, 'js, FW, const CLS: &'static str> {
    ctx: Ctx<'js>,
    obj: &'a Object<'js>,
    framework: PhantomData<FW>,
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> InitStatic<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, const CLS: &'static str> Phase for InitStatic<'a, 'js, FW, CLS> {
    type Error = Error;
}

pub struct ModuleLoad<'a, 'js: 'a> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Created>,
}

impl<'a, 'js> ModuleLoad<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Created>) -> Self {
        Self { ctx, module }
    }
}

impl<'a, 'js> Phase for ModuleLoad<'a, 'js> {
    type Error = Error;
}

pub struct ModuleEval<'a, 'js> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Loaded<Native>>,
}

impl<'a, 'js> ModuleEval<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Loaded<Native>>) -> Self {
        Self { ctx, module }
    }
}

impl<'a, 'js> Phase for ModuleEval<'a, 'js> {
    type Error = Error;
}
