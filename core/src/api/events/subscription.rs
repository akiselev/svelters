use std::any::type_name;

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
        tracing::info!("Dispatch outer");
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
    tracing::info!("Dispatch inner");
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
    pub(in crate) node: ElementNode<'js>,
    pub(in crate) callbacks: Callbacks<'js>,
}

impl<'js> SubscriptionBuilder<'js> {
    pub fn subscribe(self, func: Function<'js>) -> Self {
        self.callbacks.inner.borrow_mut().callbacks.push(func);
        self
    }

    pub fn connect<CLS: 'static + ?Sized, FUNC>(self, func: FUNC) -> Result<()>
    where
        FUNC: 'static + for<'a, 'b> FnOnce(&'a CLS, Subscription, Disconnect<CLS>) -> Result<()>,
    {
        let name = self.name;
        let disconnected = {
            let event = self.callbacks.inner.borrow();
            let inner = event.disconnect.borrow();
            inner.is_none()
        };
        if disconnected {
            let target_id = self.ctx.register(self.node.clone().into_js(self.ctx)?);
            let sub = Subscription {
                name,
                target_id,
                ctx: Context::from_ctx(self.ctx)?,
                error_handler: None,
            };
            {
                // Disconnect wrapper to make returning the disconnect function
                // cleaner, otherwise it looks ugly: `Ok(Box::new(move |...| { ... }))`
                let disconnect = Disconnect::new(
                    self.callbacks.inner.borrow().disconnect.clone(),
                    sub.clone(),
                );
                let inner = self.node.borrow();
                let this = inner.object.query_ref::<CLS>().ok_or_else(|| {
                    exception!(
                        "Element {} does not implement {}",
                        self.node.r#type.tag_name,
                        type_name::<CLS>()
                    )
                })?;
                func(this, sub, disconnect)
            };
            if self.callbacks.inner.borrow().disconnect.borrow().is_none() {
                tracing::warn!("DisconnectFn not set after connect for {}", name);
            }
        }
        Ok(())
    }
}

pub trait SubscribeFn = for<'js> Fn(&ElementNode<'js>, Function<'js>) -> Result<()>;
pub type DisconnectFn = Box<dyn for<'js_> FnOnce(&ElementNode<'js_>) -> Result<()>>;

#[must_use]
pub struct Disconnect<CLS: ?Sized> {
    pub(in crate) func: Inner<Option<DisconnectFn>>,
    pub(in crate) sub: Subscription,
    cls: PhantomData<CLS>,
}

pub(in crate) struct CbInner<'js> {
    pub(in crate) callbacks: Vec<Function<'js>>,
    pub(in crate) disconnect: Inner<Option<DisconnectFn>>,
}

impl<CLS: 'static + ?Sized> Disconnect<CLS> {
    pub(in crate) fn new(func: Inner<Option<DisconnectFn>>, sub: Subscription) -> Self {
        Self {
            func,
            sub,
            cls: PhantomData,
        }
    }

    pub fn with<FUNC>(self, func: FUNC) -> Result<()>
    where
        FUNC: 'static + for<'a> FnOnce(&'a CLS, Subscription) -> Result<()>,
    {
        let sub = self.sub;
        *self.func.borrow_mut() = Some(Box::new(move |el: &ElementNode| {
            tracing::info!("Disconnecting {}", sub.name);
            let node = el.borrow();
            let this = node.object.query_ref::<CLS>().ok_or_else(|| {
                exception!(
                    "Element {} does not implement {}",
                    el.r#type.tag_name,
                    type_name::<CLS>()
                )
            })?;
            func(this, sub)?;
            Ok(())
        }));
        Ok(())
    }
}

#[derive(Clone)]
pub struct Callbacks<'js> {
    pub(in crate) inner: Inner<CbInner<'js>>,
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
            self.inner
                .borrow()
                .callbacks
                .iter()
                .position(|callback| callback.as_object() == func.as_object())
        };
        if let Some(index) = pos {
            let mut this = self.inner.borrow_mut();
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
