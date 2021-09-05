
use crate::inner::*;

pub struct HSlot<TY, APP, INS, REM> {
    append: APP,
    insert: INS,
    remove: REM,
    types: PhantomData<TY>,
}

impl<TY, APP: HList, INS: HList, REM: HList> HSlot<TY, APP, INS, REM> {
    pub fn new() -> HSlot<CNil, HNil, HNil, HNil> { HSlot { append: HNil, insert: HNil, remove: HNil, types: PhantomData } }

    pub fn with<T, APPIN, INSIN, REMIN>(self, appendfn: APPIN, insertfn: INSIN, removefn: REMIN) -> HSlot<
        Coproduct<T, TY>,
        HCons<APPIN, APP>,
        HCons<INSIN, INS>,
        HCons<REMIN, REM>,
    > where
        APPIN: for<'js> Fn(Node<'js>, Virt<T>) -> Result<()>,
        REMIN: for<'js> Fn(Node<'js>, Virt<T>) -> Result<()>,
        INSIN: for<'js> Fn(Node<'js>, Virt<T>, Node<'js>) -> Result<()>,
    {
        let HSlot { append, insert, remove, .. } = self;
        HSlot { 
            append: append.prepend(appendfn),
            insert: insert.prepend(insertfn),
            remove: remove.prepend(removefn),
            types: PhantomData
        }
    }
}

