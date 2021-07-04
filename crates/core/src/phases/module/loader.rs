use crate::inner::*;

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
