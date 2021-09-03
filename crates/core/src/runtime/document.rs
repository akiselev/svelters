use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::inner::*;

pub struct DocInner {
    pub(in crate) events: Dispatcher,
    pub(in crate) elements: HashMap<&'static str, ElementType>,
}

pub struct Document {
    pub(in crate) inner: Rc<RefCell<DocInner>>,
}

impl Clone for Document {
    fn clone(&self) -> Self {
        panic!("Can't clone Document struct")
    }
}

impl Document {
    pub fn new() -> Document {
        Document {
            inner: Rc::new(RefCell::new(DocInner {
                events: Default::default(),
                elements: Default::default(),
            })),
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
        options: Opt<Object<'js>>,
    ) -> Result<Value<'js>> {
        self.create_element(ctx, name, options)
    }

    pub(in crate) fn create_element<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
        _options: Opt<Object<'js>>,
    ) -> Result<Value<'js>> {
        if name == "slot" {
            return Slot::new().into_js(ctx);
        }
        if let Some(element) = self.inner.borrow().elements.get(name.as_str()) {
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
