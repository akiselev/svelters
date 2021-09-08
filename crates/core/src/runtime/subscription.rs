use std::collections::HashMap;

use rquickjs::{Function, Persistent};

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
        FUNC: 'static + for<'a, 'b> FnOnce(&'a CLS, Subscription, Disconnect) -> Result<()>,
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
                obj.with_cast(|data| func(data, sub, disconnect))
            };
            if self.callbacks.borrow().disconnect.borrow().is_none() {
                tracing::warn!("DisconnectFn not set after connect for {}", name);
            }
        }
        Ok(())
    }
}
