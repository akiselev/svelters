use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub struct Subscription {
    pub(in crate) name: &'static str,
    pub(in crate) ctx: Context,
    pub(in crate) sub: Inner<EventSub>,
    pub(in crate) doc_cell: OnceCell<Document>,
    pub(in crate) error_handler: Option<RegisteryKey>,
    pub(in crate) dispatcher_cell: OnceCell<Dispatcher>,
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

    pub fn disconnect<CLS, FUNC>(self, func: FUNC) -> Result<()>
    where
        CLS: 'static,
        FUNC: 'static + for<'a> FnOnce(&'a CLS, Subscription) -> Result<()>,
    {
        let sub = self.clone();
        self.sub.borrow_mut().disconnect = Some(Box::new(move |el: Element| {
            tracing::info!("Disconnecting {}", sub.name);
            el.with_cast(|this| func(this, sub).to_result(el.ctx()))?;
            Ok(())
        }));
        Ok(())
    }
}

fn dispatch_inner<'js, T: IntoJs<'js>>(
    ctx: Ctx<'js>,
    this: &Subscription,
    detail: T,
) -> Result<()> {
    let detail = detail.into_js(ctx)?;
    let subs = this.sub.borrow();
    let obj = ctx
        .get_register(subs.target_id)
        .ok_or_else(|| Error::new(ctx, "Target not found in registry"))?;
    let event = Event::new(ctx, this.name, obj, detail)?;
    let event = event.into_js(ctx)?;
    for sub in subs.callbacks.iter() {
        let obj = ctx
            .get_register(sub.key)
            .ok_or_else(|| Error::new(ctx, "Callback not found in registry"))?;
        let func = obj
            .as_function()
            .ok_or_else(|| Error::new(ctx, "Callback not a function"))?;

        match Function::call(func, (event.clone(),)) {
            Ok(()) => (),
            Err(err) => tracing::error!("Exception in event handler: {}", err),
        }
    }
    Ok(())
}

pub struct SubscriptionBuilder<'js> {
    pub(in crate) name: &'static str,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) sub: Inner<EventSub>,
    pub(in crate) element: Element<'js>,
}

impl<'js> SubscriptionBuilder<'js> {
    pub fn subscribe(self, func: Function<'js>) -> Self {
        let key = self.ctx.register(func.into_value());
        self.sub.borrow_mut().callbacks.push(Callback::new(key));
        self
    }

    pub fn connect<CLS, FUNC>(self, func: FUNC) -> Result<()>
    where
        CLS: Clone + Type,
        FUNC: 'static + for<'a> FnOnce(&'a CLS, Subscription) -> Result<()>,
    {
        let name = self.name;
        let disconnected = {
            let event = self.sub.borrow();
            event.disconnect.is_none()
        };
        if disconnected {
            let sub = Subscription {
                name,
                ctx: Context::from_ctx(self.ctx)?,
                sub: self.sub.clone(),
                doc_cell: OnceCell::new(),
                dispatcher_cell: OnceCell::new(),
                error_handler: None,
            };
            {
                let sub = sub.clone();
                let obj = self.element.cast_as::<CLS>()?;
                func(obj.as_inner(), sub)?;
            };
            if sub.sub.borrow().disconnect.is_none() {
                tracing::warn!("DisconnectFn not set after connect for {}", name);
            }
        }
        Ok(())
    }
}
