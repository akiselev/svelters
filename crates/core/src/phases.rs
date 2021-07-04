use crate::inner::*;

pub struct ConstructorLoad<FW: Container<CLS>, const CLS: &'static str> {
    framework: PhantomData<FW>,
}

impl<FW: Container<CLS>, const CLS: &'static str> ConstructorLoad<FW, CLS> {
    pub fn new() -> Self {
        Self {
            framework: PhantomData,
        }
    }
}

impl<FW: Container<CLS>, const CLS: &'static str> Phase for ConstructorLoad<FW, CLS> {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Module<'b, Created>);
    type Error = Error;
}

pub struct ConstructorEval<FW, const CLS: &'static str> {
    framework: PhantomData<FW>,
}

impl<FW: Container<CLS>, const CLS: &'static str> ConstructorEval<FW, CLS> {
    pub fn new() -> Self {
        Self {
            framework: PhantomData,
        }
    }
}

impl<FW: Container<CLS>, const CLS: &'static str> Phase for ConstructorEval<FW, CLS> {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Module<'b, Loaded<Native>>);
    type Error = Error;
}

pub struct InitProto<FW, const CLS: &'static str> {
    framework: PhantomData<FW>,
}

impl<FW: Container<CLS>, const CLS: &'static str> InitProto<FW, CLS> {
    pub fn new() -> Self {
        Self {
            framework: PhantomData,
        }
    }
}

impl<FW: Container<CLS>, const CLS: &'static str> Phase for InitProto<FW, CLS> {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Object<'b>);
    type Error = Error;
}

pub struct InitStatic<FW, const CLS: &'static str> {
    framework: PhantomData<FW>,
}

impl<FW: Container<CLS>, const CLS: &'static str> InitStatic<FW, CLS> {
    pub fn new() -> Self {
        Self {
            framework: PhantomData,
        }
    }
}

impl<FW: Container<CLS>, const CLS: &'static str> Phase for InitStatic<FW, CLS> {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Object<'b>);
    type Error = Error;
}

pub struct ModuleLoad;

impl ModuleLoad {
    pub fn new() -> Self {
        Self
    }
}

impl Phase for ModuleLoad {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Module<'b, Created>);
    type Error = Error;
}

pub struct ModuleEval;

impl ModuleEval {
    pub fn new() -> Self {
        Self
    }
}

impl Phase for ModuleEval {
    type Input<'a, 'b: 'a> = (Ctx<'b>, &'a Module<'b, Loaded<Native>>);
    type Error = Error;
}

// impl<'a, 'b: 'a, T, FW: Container<CLS>, const CLS: &'static str>
//     Implements<ConstructorLoad<FW, CLS>> for T
// {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }

// impl<'a, 'b: 'a, T, FW: Container<CLS>, const CLS: &'static str>
//     Implements<ConstructorEval<FW, CLS>> for T
// {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }

// impl<'a, 'b: 'a, T, FW: Container<CLS>, const CLS: &'static str> Implements<InitProto<FW, CLS>>
//     for T
// {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }

// impl<'a, 'b: 'a, T, FW: Container<CLS>, const CLS: &'static str> Implements<InitStatic<FW, CLS>>
//     for T
// {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }

// impl<'a, 'b: 'a, T> Implements<ModuleLoad> for T {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }

// impl<'a, 'b: 'a, T> Implements<ModuleEval> for T {
//     fn build(self, phase: &ModuleLoad) -> Result<()> {
//         todo!()
//     }
// }
