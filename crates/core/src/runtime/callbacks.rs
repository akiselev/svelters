use std::collections::HashMap;

use rquickjs::{Function, HasRefs, Persistent};

use crate::inner::*;

pub trait SubscribeFn = for<'js> Fn(Element<'js>, Function<'js>) -> Result<()>;
pub type DisconnectFn = Box<dyn for<'js_> FnOnce(Element<'js_>) -> Result<()>>;

#[must_use]
pub struct Disconnect<CLS> {
    pub(in crate) func: Inner<Option<DisconnectFn>>,
    pub(in crate) sub: Subscription,
    cls: Class<CLS>,
}

pub(in crate) struct CbInner<'js> {
    pub(in crate) callbacks: Vec<Function<'js>>,
    pub(in crate) disconnect: Inner<Option<DisconnectFn>>,
}

impl<CLS: Type> Disconnect<CLS> {
    pub(in crate) fn new(func: Inner<Option<DisconnectFn>>, sub: Subscription) -> Self {
        Self {
            func,
            sub,
            cls: Class::new(),
        }
    }

    pub fn with<FUNC>(self, func: FUNC) -> Result<()>
    where
        CLS: 'static,
        FUNC: 'static + for<'a> FnOnce(&'a CLS, Subscription) -> Result<()>,
    {
        let sub = self.sub;
        *self.func.borrow_mut() = Some(Box::new(move |el: Element| {
            tracing::info!("Disconnecting {}", sub.name);
            let node = el.node()?;
            let obj = Obj::<CLS>::from_rt(&node.rt, node.idx)
                .ok_or_else(|| exception!("Could not find Obj<{}>", CLS::CLASS_NAME))?;
            func(&*obj, sub)?;
            Ok(())
        }));
        Ok(())
    }
}

#[derive(Clone)]
pub(in crate) struct Callbacks<'js> {
    pub(in crate) inner: Inner<CbInner<'js>>,
}

impl<'js> std::ops::Deref for Callbacks<'js> {
    type Target = Inner<CbInner<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js> HasRefs for Callbacks<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.inner
            .borrow()
            .callbacks
            .iter()
            .for_each(|func| func.as_object().mark(marker));
    }
}

impl<'js> Callbacks<'js> {
    pub fn new() -> Self {
        Callbacks {
            inner: Inner::new(CbInner {
                callbacks: vec![],
                disconnect: Inner::new(None),
            }),
        }
    }

    pub fn remove_callback(
        &self,
        ctx: Ctx<'js>,
        func: Function<'js>,
    ) -> Result<Option<DisconnectFn>> {
        let pos = {
            self.borrow()
                .callbacks
                .iter()
                .position(|callback| callback.as_object() == func.as_object())
        };
        if let Some(index) = pos {
            let mut this = self.borrow_mut();
            let callback = this.callbacks.remove(index);
            if this.callbacks.len() == 0 {
                Ok(this.disconnect.take())
            } else {
                Ok(None)
            }
        } else {
            Err(Error::new(ctx, "Event handler not registered"))
        }
    }
}
