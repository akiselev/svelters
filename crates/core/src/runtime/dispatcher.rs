use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub struct Dispatcher {
    map: Inner<HashMap<String, Subscriptions>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
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
