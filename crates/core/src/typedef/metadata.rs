use std::collections::HashMap;

use crate::inner::*;

pub struct Metadata {
    pub(in crate) obj_id: ClassId,
    pub(in crate) virt_id: ClassId,
    pub(in crate) el_id: ClassId,
}

impl Metadata {
    pub const fn new() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
            virt_id: ClassId::new(),
            el_id: ClassId::new(),
        }
    }

    pub(in crate) unsafe fn init(&mut self) -> Result<()> {
        self.obj_id.init();
        self.virt_id.init();
        self.el_id.init();
        Ok(())
    }
}

pub(in crate) type ObjCasterMap<CLS> =
    HashMap<&'static str, Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Obj<CLS>>) -> Result<Value<'a>>>>;
pub(in crate) type VirtCasterMap<CLS> = HashMap<
    &'static str,
    Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Virt<CLS>>) -> Result<Value<'a>>>,
>;
pub(in crate) type ElementCasterMap<CLS> = HashMap<
    &'static str,
    Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Element<CLS>>) -> Result<Value<'a>>>,
>;
