use std::{collections::HashMap, rc::Rc};

use rquickjs::{Function, Persistent};

use crate::inner::*;

pub struct EventHandler {
    func: Persistent<Function<'static>>,
    disconnect: Option<Box<dyn FnOnce()>>,
}

impl EventHandler {
    pub fn new(func: Persistent<Function<'static>>, disconnect: Option<Box<dyn FnOnce()>>) -> Self {
        Self { func, disconnect }
    }
}

impl rquickjs::HasRefs for EventHandler {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.func.mark_refs(marker)
    }
}

impl std::cmp::PartialEq<Persistent<Function<'static>>> for EventHandler {
    fn eq(&self, other: &Persistent<Function<'static>>) -> bool {
        self.func == *other
    }
}

pub struct HandlerList {
    handlers: Vec<EventHandler>,
}

impl rquickjs::HasRefs for HandlerList {
    fn mark_refs(&self, marker: &RefsMarker) {
        for handler in self.handlers.iter() {
            handler.mark_refs(marker)
        }
    }
}

impl HandlerList {
    pub fn new() -> Self {
        Self { handlers: vec![] }
    }

    pub fn remove_handler(&mut self, func: Persistent<Function<'static>>) -> Result<()> {
        for handler in self.handlers.iter_mut() {
            if *handler == func {
                if let Some(func) = handler.disconnect.take() {
                    func();
                }
            }
        }
        self.handlers.retain(|handler| *handler != func);
        Ok(())
    }
}

#[derive(Default)]
pub struct EventHandlers {
    map: HashMap<String, HandlerList>,
}

impl Clone for EventHandlers {
    fn clone(&self) -> Self {
        panic!("Can't clone event handlers struct")
    }
}

impl EventHandlers {
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
            for handler in list.handlers.iter_mut() {
                let func: Function = handler.func.clone().restore(ctx)?;
                func.call((event.clone(),))?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn add_event_handler(
        &mut self,
        name: String,
        func: Persistent<Function<'static>>,
        disconnect: Option<Box<dyn FnOnce()>>,
    ) -> Result<()> {
        let list = self.map.entry(name).or_insert_with(|| HandlerList::new());
        list.handlers.push(EventHandler::new(func, disconnect));
        Ok(())
    }

    pub fn remove_event_handler(
        &mut self,
        name: String,
        func: Persistent<Function<'static>>,
    ) -> Result<()> {
        if let Some(list) = self.map.get_mut(&name) {
            list.remove_handler(func)
        } else {
            Ok(())
        }
    }
}

impl rquickjs::HasRefs for EventHandlers {
    fn mark_refs(&self, marker: &RefsMarker) {
        for handler in self.map.values() {
            handler.mark_refs(marker)
        }
    }
}
