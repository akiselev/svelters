use std::{
    any::{Any, TypeId},
    marker::Unsize,
    mem::{align_of, size_of},
};

use smallbox::{smallbox, SmallBox};

use crate::inner::*;

pub type Rt = Runtime;

pub struct Runtime {
    inner: Rc<RefCell<RT>>,
}

struct RT {
    types: SlotMap<Idx, Instance>,
}

pub(in crate) struct Instance {
    typeid: TypeId,
    data: SmallBox<dyn Any, smallbox::space::S2>,
}

impl Instance {
    pub(in crate) fn new<CLS: Sized + Type + Any>(data: CLS) -> Self
    where
        [(); { size_of::<CLS>() }]: ,
    {
        let typeid = TypeId::of::<CLS>();
        Self {
            typeid,
            data: smallbox!(data),
        }
    }
}

pub(in crate) type Idx = InstanceKey;
new_key_type! { pub(in crate) struct InstanceKey; }
