use rquickjs::{bind, HasRefs, Object, Persistent};

use crate::inner::*;

#[bind(object)]
#[quickjs(bare)]
pub(in crate) mod slotdef {
    use super::*;

    #[derive(HasRefs)]
    pub struct Slot {
        pub(in crate) name: String,
        #[quickjs(has_refs)]
        pub(in crate) children: Vec<Persistent<Object<'static>>>,
    }

    impl Slot {
        // constructor
        #[quickjs(skip)]
        pub fn new() -> Self {
            Self {
                name: "default".to_string(),
                children: vec![],
            }
        }

        #[quickjs(rename = "slot", get)]
        pub fn get_name(&self) -> String {
            self.name.clone()
        }

        #[quickjs(rename = "slot", set)]
        pub fn set_name(&mut self, name: String) {
            self.name = name
        }
    }
}

pub use self::slotdef::Slot;
