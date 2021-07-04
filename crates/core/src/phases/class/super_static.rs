use crate::inner::*;

pub struct InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS, SUPER)>,
}

impl<'a, 'js, FW: Container<CLS>, CLS, SUPER> InitSuperStatic<'a, 'js, FW, CLS, SUPER> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}
