use crate::inner::*;

pub struct Event<CLS, FUNC> {
    pub(in crate) name: &'static str,
    pub(in crate) func: FUNC,
    marker: PhantomData<CLS>,
}

impl<CLS, FUNC> Event<CLS, FUNC> {
    pub(in crate) fn new(name: &'static str, func: FUNC) -> Self {
        Event {
            name,
            func,
            marker: PhantomData,
        }
    }
}
