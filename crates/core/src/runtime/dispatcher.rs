use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub(in crate) struct Dispatcher<'js> {
    pub(in crate) inner: Inner<DispInner<'js>>,
}

impl<'js> std::ops::Deref for Dispatcher<'js> {
    type Target = Inner<DispInner<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(in crate) struct DispInner<'js> {
    pub(in crate) subs: HashMap<&'static str, Callbacks<'js>>,
}

impl<'js> Dispatcher<'js> {
    pub fn new() -> Self {
        Self {
            inner: Inner::new(DispInner {
                subs: Default::default(),
            }),
        }
    }

    pub fn dispatch(&self, ctx: Ctx<'js>, event: Event<'js>) -> Result<()> {
        let inner = self.inner.borrow();
        let name = event
            ._type
            .as_ref()
            .ok_or_else(|| exception!("Event has no type"))?
            .as_str();
        if let Some(cb) = inner.subs.get(name) {
            let value = event.into_js(ctx)?;
            for func in cb.borrow().callbacks.iter() {
                func.call((value.clone(),))?;
            }
        }
        Ok(())
    }
}
