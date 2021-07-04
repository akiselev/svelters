use crate::inner::*;

pub struct ConstructorLoad<'a, 'js, FW: Container<CLS>, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a Module<'js, Created>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
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

pub struct ConstructorEval<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a Module<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
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
