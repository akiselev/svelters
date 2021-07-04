use crate::inner::*;

pub struct InitProto<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
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
