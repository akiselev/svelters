use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

pub trait SubscribeFn = for<'js> Fn(Element<'js>, Function<'js>) -> Result<()>;
pub type DisconnectFn = Box<dyn for<'js_> FnOnce(Element<'js_>) -> Result<()>>;

pub struct Callback {
    pub(in crate) key: RegisteryKey,
}

impl Callback {
    pub fn new(key: RegisteryKey) -> Self {
        Self { key }
    }
}

pub struct EventSub {
    pub(in crate) target_id: RegisteryKey,
    pub(in crate) callbacks: Vec<Callback>,
    pub(in crate) disconnect: Option<DisconnectFn>,
}

impl EventSub {
    pub fn new(target_id: RegisteryKey) -> Self {
        Self {
            target_id,
            callbacks: vec![],
            disconnect: None,
        }
    }

    pub fn remove_callback<'js>(
        &mut self,
        ctx: Ctx<'js>,
        func: Function<'js>,
    ) -> Result<Option<DisconnectFn>> {
        let pos = {
            self.callbacks.iter().position(|callback| {
                if let Some(callback) = ctx.get_register(callback.key) {
                    callback == *func.as_value()
                } else {
                    false
                }
            })
        };
        if let Some(index) = pos {
            let callback = self.callbacks.remove(index);
            if self.callbacks.len() == 0 {
                Ok(self.disconnect.take())
            } else {
                Ok(None)
            }
        } else {
            Err(Error::new(ctx, "Event handler not registered"))
        }
    }
}
