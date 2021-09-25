use std::cell::RefMut;

use crate::inner::*;

pub struct TypeBuilder<'a> {
    pub(in crate) rt: RefMut<'a, SvRuntime>,
}

impl<'a> TypeBuilder<'a> {
    pub fn new(rt: RefMut<'a, SvRuntime>) -> Self {
        TypeBuilder { rt }
    }

    pub fn from_ctx<'js: 'a>(ctx: Ctx<'js>) -> Result<Self> {
        let rt: &Runtime = ctx.globals().get("document")?;
        Ok(TypeBuilder {
            rt: rt.inner.borrow_mut(),
        })
    }

    pub fn register_element(&mut self, tag_name: &'static str, func: ElementDefFn) -> &mut Self {
        self.rt.element_fns.insert(tag_name.into(), func);
        self
    }

    pub fn register_interface(
        &mut self,
        tag_name: &'static str,
        func: InterfaceDefFn,
    ) -> &mut Self {
        self.rt.interfaces.insert(tag_name.into(), func);
        self
    }
}
