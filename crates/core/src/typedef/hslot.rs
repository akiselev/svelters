
use crate::inner::*;


#[derive(Clone)]
pub struct BasicSlot<APP, INS, REM> {
    append: APP,
    insert: INS,
    remove: REM,
}

impl<APP, INS, REM> BasicSlot<APP, INS, REM> {
    pub fn new(append: APP, insert: INS, remove: REM) -> BasicSlot<APP, INS, REM>
    where
        APP: for<'js> Fn(Node<'js>, Node<'js>) -> Result<()>,
        REM: for<'js> Fn(Node<'js>, Node<'js>) -> Result<()>,
        INS: for<'js> Fn(Node<'js>, Node<'js>, Node<'js>) -> Result<()>,
    {
        BasicSlot { append, insert, remove }
    }

}

impl<APP, INS, REM> NativeSlot for BasicSlot<APP, INS, REM>
where
    APP: Clone + 'static + for<'js> Fn(Node<'js>, Node<'js>) -> Result<()>,
    REM: Clone + 'static + for<'js> Fn(Node<'js>, Node<'js>) -> Result<()>,
    INS: Clone + 'static + for<'js> Fn(Node<'js>, Node<'js>, Node<'js>) -> Result<()>,
{
    fn append_child<'js>(&self, this: Node<'js>, node: Node<'js>) -> Result<()> {
        (self.append)(this, node)
    }

    fn remove_child<'js>(&self, this: Node<'js>, node: Node<'js>) -> Result<()> {
        (self.remove)(this, node)
    }

    fn insert_before<'js>(&self, this: Node<'js>, node: Node<'js>, anchor: Node<'js>) -> Result<()> {
        (self.insert)(this, node, anchor)
    }
}

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

