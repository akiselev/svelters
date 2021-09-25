use std::any::{Any, TypeId};
use std::{collections::HashMap, rc::Rc};

use rquickjs::Function;
use smallvec::SmallVec;

use crate::inner::*;

pub type ElementDefFn = &'static fn() -> &'static Metadata;

/// Type metadata for a type registered with Svelters
pub struct Metadata {
    pub(in crate) type_id: TypeId,
    pub(in crate) obj_id: ClassId,
    pub(in crate) impl_id: ClassId,
}

impl Metadata {
    pub const fn new<CLS: Any>() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
            impl_id: ClassId::new(),
            type_id: TypeId::of::<CLS>(),
        }
    }

    pub(in crate) unsafe fn init(&mut self) -> Result<()> {
        self.obj_id.init();
        self.impl_id.init();
        Ok(())
    }
}
