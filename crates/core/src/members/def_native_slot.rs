use crate::inner::*;

pub struct NativeSlot<CLS, SLOT> {
    pub(in crate) slot: SLOT,
    marker: PhantomData<CLS>,
}

impl<CLS, SLOT> NativeSlot<CLS, SLOT> {
    pub(in crate) fn new(slot: SLOT) -> Self {
        NativeSlot {
            slot,
            marker: PhantomData,
        }
    }
}
