use rquickjs::{bind, HasRefs, Object, Persistent};

#[allow(unused_imports)]
use crate::inner::*;

pub(in crate) struct SlotInner {
    pub(in crate) name: String,
    pub(in crate) children: Vec<Persistent<Object<'static>>>,
}

#[bind(object)]
#[quickjs(bare)]
pub(in crate) mod slotdef {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[derive(Clone)]
    pub struct Slot(pub(in super) Rc<RefCell<SlotInner>>);

    impl Slot {
        // constructor
        #[quickjs(skip)]
        pub fn new() -> Self {
            Self(Rc::new(RefCell::new(SlotInner {
                name: "default".to_string(),
                children: vec![],
            })))
        }

        #[quickjs(skip)]
        pub(in crate) fn borrow(&self) -> std::cell::Ref<SlotInner> {
            self.0.borrow()
        }

        #[quickjs(skip)]
        pub(in crate) fn borrow_mut(&self) -> std::cell::RefMut<SlotInner> {
            self.0.borrow_mut()
        }

        #[quickjs(rename = "slot", get)]
        pub fn get_name(&self) -> String {
            self.borrow().name.clone()
        }

        #[quickjs(rename = "slot", set)]
        pub fn set_name(&self, name: String) {
            self.borrow_mut().name = name
        }
    }
}

pub use self::slotdef::Slot;
