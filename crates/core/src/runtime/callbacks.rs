use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

pub type DisconnectFn = Box<dyn for<'js_> FnOnce(Node<'js_>) -> Result<()>>;

pub struct Callback {
    pub(in crate) key: RegisteryKey,
    pub(in crate) disconnect: DisconnectFn,
}

impl Callback {
    pub fn new(key: RegisteryKey, disconnect: DisconnectFn) -> Self {
        Self { key, disconnect }
    }
}

pub struct Subscriptions {
    pub(in crate) callbacks: Vec<Callback>,
}

impl Subscriptions {
    pub fn new() -> Self {
        Self { callbacks: vec![] }
    }

    pub fn remove_callback<'js>(
        &mut self,
        ctx: Ctx<'js>,
        func: Function<'js>,
    ) -> Result<DisconnectFn> {
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
            Ok(callback.disconnect)
        } else {
            Err(Error::new(ctx, "Event handler not registered"))
        }
    }
}
