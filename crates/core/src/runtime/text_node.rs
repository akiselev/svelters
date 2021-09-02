use rquickjs::{bind, HasRefs, Object, Persistent};

use crate::inner::*;

#[bind(object)]
#[quickjs(bare)]
pub(in crate) mod textnode {
    use super::*;

    pub struct TextNode {
        pub(in crate) mounted: bool,
        pub(in crate) text: String,
    }

    impl TextNode {
        // constructor
        #[quickjs(skip)]
        pub fn new(text: String) -> Self {
            Self { text }
        }

        #[quickjs(rename = "text", get)]
        pub fn get_text(&self) -> String {
            self.text.clone()
        }

        #[quickjs(retext = "text", set)]
        pub fn set_text(&mut self, text: String) {
            self.text = text
        }
    }
}

pub use self::slotdef::Slot;
