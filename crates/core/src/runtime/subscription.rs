use rquickjs::{Function, HasRefs, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub struct Subscription {
    pub(in crate) name: &'static str,
    pub(in crate) ctx: Context,
    pub(in crate) target_id: RegisteryKey,
    pub(in crate) error_handler: Option<RegisteryKey>,
}

impl Subscription {
    pub fn dispatch<T: for<'js> IntoJs<'js>>(&self, detail: T) {
        self.ctx.with(move |ctx| {
            let res = dispatch_inner(ctx, self, detail);
            match res {
                Ok(()) => (),
                Err(err) => tracing::error!("Exception in event dispatch: {}", err),
            }
        })
    }
}

fn dispatch_inner<'js, T: IntoJs<'js>>(
    ctx: Ctx<'js>,
    this: &Subscription,
    detail: T,
) -> Result<()> {
    let detail = detail.into_js(ctx)?;
    let value = ctx
        .get_register(this.target_id)
        .ok_or_else(|| Error::new(ctx, "Target not found in registry"))?;
    let el: Element = Element::from_js(ctx, value.clone())?;
    let dispatcher = el.dispatcher()?;
    let event = Event::new(ctx, this.name, value, detail)?;
    dispatcher.dispatch(ctx, event)?;
    Ok(())
}

pub struct SubscriptionBuilder<'js> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) name: &'static str,
    pub(in crate) element: Element<'js>,
    pub(in crate) callbacks: Callbacks<'js>,
}

impl<'js> SubscriptionBuilder<'js> {
    pub fn subscribe(self, func: Function<'js>) -> Self {
        self.callbacks.borrow_mut().callbacks.push(func);
        self
    }

    pub fn connect<CLS, FUNC>(self, func: FUNC) -> Result<()>
    where
        CLS: Clone + Type,
        FUNC: 'static + for<'a, 'b> FnOnce(&'a CLS, Subscription, Disconnect<CLS>) -> Result<()>,
    {
        let name = self.name;
        let disconnected = {
            let event = self.callbacks.borrow();
            let inner = event.disconnect.borrow();
            inner.is_none()
        };
        if disconnected {
            let target_id = self.ctx.register(self.element.el_node.clone().into_value());
            let sub = Subscription {
                name,
                target_id,
                ctx: Context::from_ctx(self.ctx)?,
                error_handler: None,
            };
            {
                // Disconnect wrapper to make returning the disconnect function
                // cleaner, otherwise it looks ugly: `Ok(Box::new(move |...| { ... }))`
                let disconnect =
                    Disconnect::new(self.callbacks.borrow().disconnect.clone(), sub.clone());
                let obj = self.element.cast_as::<CLS>()?;
                func(&obj.cls, sub, disconnect)
            };
            if self.callbacks.borrow().disconnect.borrow().is_none() {
                tracing::warn!("DisconnectFn not set after connect for {}", name);
            }
        }
        Ok(())
    }
}

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
            let obj = Obj::<CLS>::from_rt(&node.rt, node.idx)?;
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
