
use crate::inner::*;

pub struct Element<CLS, FUNC> {
    pub(in crate) name: &'static str,
    pub(in crate) func: FUNC,
    marker: PhantomData<CLS>,
}

impl<CLS, FUNC> Element<CLS, FUNC> {
    pub(in crate) fn new(name: &'static str, func: FUNC) -> Self {
        Element {
            name,
            func,
            marker: PhantomData,
        }
    }
}
