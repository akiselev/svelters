use crate::inner::*;
pub struct ConstructorLoad<'a, 'js, FW: Container<CLS>, CLS> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Created>,
    framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Container<CLS>, CLS> ConstructorLoad<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, CLS> Phase for ConstructorLoad<'a, 'js, FW, CLS> {}

pub struct ConstructorEval<'a, 'js, FW, CLS> {
    ctx: Ctx<'js>,
    module: &'a Module<'js, Loaded<Native>>,
    framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Container<CLS>, CLS> ConstructorEval<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Loaded<Native>>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, CLS> Phase for ConstructorEval<'a, 'js, FW, CLS> {}

pub struct InitProto<'a, 'js, FW, CLS> {
    ctx: Ctx<'js>,
    obj: &'a Object<'js>,
    framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Container<CLS>, CLS> InitProto<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, CLS> Phase for InitProto<'a, 'js, FW, CLS> {}

pub struct InitStatic<'a, 'js, FW, CLS> {
    ctx: Ctx<'js>,
    obj: &'a Object<'js>,
    framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Container<CLS>, CLS> InitStatic<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'js, FW: Container<CLS>, CLS> Phase for InitStatic<'a, 'js, FW, CLS> {}

#[derive(Clone, Copy)]
pub struct ModuleLoad<'a, 'js: 'a> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a Module<'js, Created>,
}

impl<'a, 'js> ModuleLoad<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Created>) -> Self {
        Self { ctx, module }
    }
}

impl<'a, 'js> Phase for ModuleLoad<'a, 'js> {}

#[derive(Clone, Copy)]
pub struct ModuleEval<'a, 'b: 'a> {
    pub(in crate) ctx: Ctx<'b>,
    pub(in crate) module: &'a Module<'b, Loaded<Native>>,
}

impl<'a, 'js> ModuleEval<'a, 'js> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Loaded<Native>>) -> Self {
        Self { ctx, module }
    }
}

impl<'a, 'js> Phase for ModuleEval<'a, 'js> {}

// default impl<T, FW: Container<CLS>, CLS: 'static> Implements<ConstructorLoad<FW, CLS>> for T {
//     fn build<'a, 'b: 'a>(
//         self,
//         phase: <ConstructorLoad<FW, CLS> as Phase>::Input<'a, 'b>,
//     ) -> Result<()> {
//         todo!()
//     }
// }

// default impl<T, FW: Container<CLS>, CLS: 'static> Implements<ConstructorEval<FW, CLS>> for T {
//     fn build<'a, 'b: 'a>(
//         self,
//         phase: <ConstructorEval<FW, CLS> as Phase>::Input<'a, 'b>,
//     ) -> Result<()> {
//         todo!()
//     }
// }

// default impl<T, FW: Container<CLS>, CLS: 'static> Implements<InitProto<FW, CLS>> for T {
//     fn build<'a, 'b: 'a>(self, phase: <InitProto<FW, CLS> as Phase>::Input<'a, 'b>) -> Result<()> {
//         todo!()
//     }
// }

// default impl<T, FW: Container<CLS>, CLS: 'static> Implements<InitStatic<FW, CLS>> for T {
//     fn build<'a, 'b: 'a>(self, phase: <InitStatic<FW, CLS> as Phase>::Input<'a, 'b>) -> Result<()> {
//         todo!()
//     }
// }

// default impl<T> Implements<ModuleLoad> for T {
//     fn build<'a, 'b: 'a>(self, phase: <ModuleLoad as Phase>::Input<'a, 'b>) -> Result<()> {
//         todo!()
//     }
// }

// default impl<T> Implements<ModuleEval> for T {
//     fn build<'a, 'b: 'a>(self, phase: <ModuleEval as Phase>::Input<'a, 'b>) -> Result<()> {
//         todo!()
//     }
// }
