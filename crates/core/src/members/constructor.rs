
use crate::inner::*;

pub struct Constructor<CLS, ARGS, THIS>(pub(crate) THIS, pub(crate) PhantomData<(CLS, ARGS)>)
where
    CLS: Type;

impl<THIS, ARGS, CLS> Constructor<CLS, ARGS, THIS>
where
    CLS: Type,
{
    pub(in crate) fn new(func: THIS) -> Self {
        Constructor(func, PhantomData)
    }
}
