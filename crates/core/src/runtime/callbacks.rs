use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

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
    callbacks: Vec<Callback>,
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

#[derive(Default)]
pub struct Dispatcher {
    map: Inner<HashMap<String, Subscriptions>>,
}

impl Clone for Dispatcher {
    fn clone(&self) -> Self {
        panic!("Can't clone event callbacks struct")
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub fn dispatch_event<'js, T: IntoJs<'js>>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
        event: Event<Value<'js>>,
    ) -> Result<bool> {
        let mut this = self.map.borrow_mut();

        if let Some(list) = this.get_mut(&name) {
            let event = event.into_js(ctx)?;
            for callback in list.callbacks.iter_mut() {
                let callback = ctx
                    .get_register(callback.key)
                    .and_then(|val| val.into_function())
                    .expect("Event handler not found");
                callback.call((event.clone(),))?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn add_event_callback(
        &mut self,
        name: String,
        key: RegisteryKey,
        disconnect: DisconnectFn,
    ) -> Result<()> {
        let mut this = self.map.borrow_mut();
        let list = this.entry(name).or_insert_with(|| Subscriptions::new());
        list.callbacks.push(Callback::new(key, disconnect));
        Ok(())
    }

    pub fn remove_event_callback<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
        func: Function<'js>,
    ) -> Result<DisconnectFn> {
        let mut this = self.map.borrow_mut();
        if let Some(list) = this.get_mut(&name) {
            list.remove_callback(ctx, func)
        } else {
            Err(Error::new(ctx, "Event handler not registered"))
        }
    }
}

pub type DisconnectFn = Box<dyn for<'js_> FnOnce(Node<'js_>) -> Result<()>>;
