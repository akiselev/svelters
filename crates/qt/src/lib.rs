#![recursion_limit = "1024"]
#![allow(unused)]
#![allow(non_camel_case_types)]
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use document::document::Document;
use phf::phf_map;
use qmetaobject::connections::ConnectionHandle;
use qttypes::*;

use rquickjs::{Ctx, Value};
use svelters_core::{EventHandler, Framework};

pub mod application;
pub mod document;

mod elements;
mod object;

// pub use document::Document;
pub use object::{qobject::QObject, QPtr, QRawPtr};

pub(crate) use elements::FRAMEWORK;

pub(in crate) type Class = svelters_core::Class<Qt>;
pub(in crate) type Method = svelters_core::Method<Qt>;
pub(in crate) type Prop = svelters_core::Prop<Qt>;
pub(in crate) type Event = svelters_core::Event<Qt>;

pub(in crate) type ClassMap = svelters_core::ClassMap<Qt>;
pub(in crate) type MethodMap = svelters_core::MethodMap<Qt>;
pub(in crate) type PropMap = svelters_core::PropMap<Qt>;
pub(in crate) type EventMap = svelters_core::EventMap<Qt>;

pub(in crate) type Constructor = svelters_core::Constructor<Qt>;
pub(in crate) type ConnectFn = svelters_core::ConnectFn<Qt>;
pub(in crate) type GetterFn = svelters_core::GetterFn<Qt>;
pub(in crate) type SetterFn = svelters_core::SetterFn<Qt>;
pub(in crate) type MethodFn = svelters_core::MethodFn<Qt>;
pub(in crate) type InsertFn = svelters_core::InsertFn<Qt>;

#[derive(Clone)]
pub struct Qt {}

impl Qt {
    pub fn new() -> Self {
        Self {}
    }
}

impl Framework for Qt {
    type App = application::Application;
    type Object = QObject;
    type Handler = EventHandle;

    fn init_app<'js>(&self, ctx: Ctx<'js>) -> anyhow::Result<Self::App> {
        let globals = ctx.globals();
        globals.init_def::<object::Qobject>()?;
        globals.init_def::<document::Document>()?;
        let instance = rquickjs::Class::instance(ctx, Document::new())?;
        globals.set("document", instance)?;
        globals.set("window", Value::new_null(ctx))?;
        Ok(application::Application::new())
    }

    fn get_schema() -> &'static svelters_core::FrameworkSchema<Self> {
        &FRAMEWORK
    }
}

pub struct EventHandle(pub(in crate) ConnectionHandle);

impl EventHandler for EventHandle {
    fn disconnect(&mut self) -> anyhow::Result<bool> {
        self.0.disconnect();
        Ok(true)
    }
}
