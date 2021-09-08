use crate::inner::*;

pub struct Method<CLS, ARGS, RET, THIS> {
    pub(crate) name: &'static str,
    pub(crate) func: THIS,
    marker: PhantomData<(CLS, ARGS, RET)>,
}

impl<THIS, ARGS, RET, CLS> Method<CLS, ARGS, RET, THIS> {
    pub(in crate) fn new(name: &'static str, func: THIS) -> Self {
        Method {
            name,
            func,
            marker: PhantomData,
        }
    }
}
