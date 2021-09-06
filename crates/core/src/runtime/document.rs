use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::inner::*;

pub struct DocInner {
    pub(in crate) types: HashMap<&'static str, ElementType>,
}

pub struct Document {
    pub(in crate) inner: Inner<DocInner>,
}

impl Clone for Document {
    fn clone(&self) -> Self {
        panic!("Can't clone Document struct")
    }
}

impl Document {
    pub fn new() -> Document {
        Document {
            inner: Inner::new(DocInner {
                types: Default::default(),
            }),
        }
    }

    pub(in crate) fn create_event<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _interface: String,
    ) -> Result<Value<'js>> {
        Event::custom(ctx)
    }

    pub(in crate) fn create_element_ns<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _ns: Value<'js>,
        name: String,
        _options: Opt<Object<'js>>,
    ) -> Result<Value<'js>> {
        self.create_element(ctx, name)
    }

    pub(in crate) fn create_element<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
    ) -> Result<Value<'js>> {
        if name == "slot" {
            return SlotNode::new().into_js(ctx);
        }
        if let Some(element) = self.inner.borrow().types.get(name.as_str()) {
            (element.constructor)(ctx)
        } else {
            Err(Error::new(ctx, format!("{} is not a valid element", name)))
        }
    }

    pub(in crate) fn dispatch_event<'js>(
        &mut self,
        _ctx: Ctx<'js>,
        _event: Object<'js>,
        _options: Opt<Object<'js>>,
    ) -> Result<()> {
        Ok(())
    }
}
