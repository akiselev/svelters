use std::{collections::HashMap};

use rquickjs::{Function, Persistent};

use crate::inner::*;

pub struct Callback {
    func: Persistent<Function<'static>>,
    disconnect: Option<Box<dyn FnOnce()>>,
}

impl Callback {
    pub fn new(func: Persistent<Function<'static>>, disconnect: Option<Box<dyn FnOnce()>>) -> Self {
        Self { func, disconnect }
    }
}

impl rquickjs::HasRefs for Callback {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.func.mark_refs(marker)
    }
}

impl std::cmp::PartialEq<Persistent<Function<'static>>> for Callback {
    fn eq(&self, other: &Persistent<Function<'static>>) -> bool {
        self.func == *other
    }
}

pub struct Subscriptions {
    callbacks: Vec<Callback>,
}

impl rquickjs::HasRefs for Subscriptions {
    fn mark_refs(&self, marker: &RefsMarker) {
        for callback in self.callbacks.iter() {
            callback.mark_refs(marker)
        }
    }
}

impl Subscriptions {
    pub fn new() -> Self {
        Self { callbacks: vec![] }
    }

    pub fn remove_callback(&mut self, func: Persistent<Function<'static>>) -> Result<()> {
        for callback in self.callbacks.iter_mut() {
            if *callback == func {
                if let Some(func) = callback.disconnect.take() {
                    func();
                }
            }
        }
        self.callbacks.retain(|callback| *callback != func);
        Ok(())
    }
}

#[derive(Default)]
pub struct Dispatcher {
    map: HashMap<String, Subscriptions>,
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
        if let Some(list) = self.map.get_mut(&name) {
            let event = event.into_js(ctx)?;
            for callback in list.callbacks.iter_mut() {
                let func: Function = callback.func.clone().restore(ctx)?;
                func.call((event.clone(),))?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn add_event_callback(
        &mut self,
        name: String,
        func: Persistent<Function<'static>>,
        disconnect: Option<Box<dyn FnOnce()>>,
    ) -> Result<()> {
        let list = self.map.entry(name).or_insert_with(|| Subscriptions::new());
        list.callbacks.push(Callback::new(func, disconnect));
        Ok(())
    }

    pub fn remove_event_callback(
        &mut self,
        name: String,
        func: Persistent<Function<'static>>,
    ) -> Result<()> {
        if let Some(list) = self.map.get_mut(&name) {
            list.remove_callback(func)
        } else {
            Ok(())
        }
    }
}

impl rquickjs::HasRefs for Dispatcher {
    fn mark_refs(&self, marker: &RefsMarker) {
        for callback in self.map.values() {
            callback.mark_refs(marker)
        }
    }
}
