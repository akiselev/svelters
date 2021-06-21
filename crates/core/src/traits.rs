use anyhow::{anyhow, Result};
use std::borrow::Cow;

use crate::def::FrameworkSchema;
use crate::inner::*;

pub trait Framework: Clone + 'static {
    // type App: Application;
    // type Object: ObjectKind;

    // fn init_app(&self) -> Result<Self::App>;
    // fn get_schema(&self) -> FrameworkSchema;
}

pub trait Application {
    fn has_events(&self) -> bool;
    fn process_system_events(&self, timeout_ms: u32) -> bool;
    fn process_framework_events(&self, timeout_ms: u32) -> bool;
}

pub trait ObjectKind {
    fn get_tab(&self) -> Cow<'static, str>;
    fn is_live(&self) -> bool;

    fn get_id(&self) -> String;
    fn set_id(&mut self, id: &str);

    // fn add_event_listener(
    //     &self,
    //     name: String,
    //     func_id: SavedFunction,
    //     func: Box<dyn for<'js> Fn(Ctx<'js>, Vec<Value<'js>>)>,
    // );

    // fn remove_event_listener(&self, name: String, func_id: SavedFunction);

    // fn get_attribute(&self, name: String, func_id: SavedFunction);
}
