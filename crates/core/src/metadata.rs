use std::{collections::HashMap, lazy::OnceCell};

use lazy_static::lazy_static;
use parking_lot::Once;

use crate::{build_class, inner::*};

pub struct Metadata {
    pub(in crate) obj_id: ClassId,
    pub(in crate) virt_id: ClassId,
}

impl Metadata {
    pub const fn new() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
            virt_id: ClassId::new(),
        }
    }

    pub(in crate) unsafe fn init(&mut self) -> Result<()> {
        self.obj_id.init();
        self.virt_id.init();
        Ok(())
    }
}

pub(in crate) type MetaFuncMap<FW, CLS> = HashMap<
    &'static str,
    Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Obj<FW, CLS>>) -> Result<Value<'a>>>,
>;
