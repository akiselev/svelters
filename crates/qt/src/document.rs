use lazy_static::lazy_static;

use svelters::class::{ClassDef, ClassID};

pub struct Document {}

impl ClassDef for Document {
    fn class_id() -> ClassID {
        lazy_static! {
            static ref CLASS_ID: ClassID = ClassID::new();
        }
        *CLASS_ID
    }

    unsafe fn class_init() -> libquickjs_sys::JSValue {
        todo!()
    }
}
