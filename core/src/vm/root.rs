use crate::inner::*;
use query_interface::{interfaces, vtable_for, ObjectClone, ObjectPartialEq};
use smallbox::smallbox;

#[derive(Clone, PartialEq, Debug)]
pub struct RootElement {}

pub trait IRootElement {}

impl IRootElement for RootElement {}

interfaces!(
    RootElement: dyn IRootElement,
    dyn ObjectClone,
    dyn ObjectPartialEq
);

pub fn root_element(this: TypeBuilder) -> ElementDef {
    ElementDef::new(this, "root", |ctx: Ctx| Ok(smallbox!(RootElement {}))).build()
}
