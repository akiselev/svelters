use crate::inner::*;

pub struct StaticFn<CLS, ARGS, RET, THIS>(
    pub(crate) &'static str,
    pub(crate) THIS,
    PhantomData<(CLS, ARGS, RET)>,
)
where
    CLS: Type;

impl<THIS, ARGS, RET, CLS> StaticFn<CLS, ARGS, RET, THIS>
where
    CLS: Type,
{
    pub(in crate) fn new(name: &'static str, func: THIS) -> Self {
        StaticFn(name, func, PhantomData)
    }
}
