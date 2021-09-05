use rquickjs::{bind, HasRefs, Object, Persistent};

#[allow(unused_imports)]
use crate::inner::*;

#[derive(Clone, Eq, PartialEq)]
pub(in crate) struct SlotInner<'js> {
    pub(in crate) name: String,
    pub(in crate) node: NodeData<'js>,
}


#[derive(Clone, Eq, PartialEq)]
pub struct Slot<'js>(pub(in super) Inner<SlotInner<'js>>);

impl<'js> Slot<'js> {
    // constructor
    pub fn new() -> Self {
        Self(Inner::new(SlotInner {
            name: "default".to_string(),
            node: NodeData::new()
        }))
    }

    pub(in crate) fn borrow<'a>(&'a self) -> std::cell::Ref<'a, SlotInner<'js>> where 'js: 'a{
        self.0.borrow()
    }

    pub(in crate) fn borrow_mut<'a>(&self) -> std::cell::RefMut<SlotInner<'js>> where 'js: 'a {
        self.0.borrow_mut()
    }

    pub fn get_name(&self) -> String {
        self.borrow().name.clone()
    }

    pub fn set_name(&self, name: String) {
        self.borrow_mut().name = name
    }
}

