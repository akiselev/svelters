use crate::inner::*;

pub struct Superclass<SUPER>(PhantomData<SUPER>);

impl<SUPER> Superclass<SUPER> {
    pub(in crate) fn new() -> Self {
        Superclass(PhantomData)
    }
}
