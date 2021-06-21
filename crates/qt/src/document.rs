use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;
use std::sync::Arc;

use cpp::{cpp, cpp_class};
use qmetaobject::connections::ConnectionHandle;
use rquickjs::{
    bind, Class, Ctx, Error, FromJs, Function, IntoJs, Object, RegisteryKey, Rest, Result, Value,
};

use svelters_core::{get_class, Framework};

use crate::EventHandle;

#[allow(non_snake_case)]
#[bind(object, public)]
#[quickjs(bare)]
pub mod document {

    use crate::{QObject, Qt};

    use super::*;

    pub struct Document {
        pub(crate) events: HashMap<String, HashMap<RegisteryKey, EventHandle>>,
    }

    impl Document {
        #[quickjs(skip)]
        pub fn new() -> Self {
            let events = HashMap::new();
            Document { events }
        }

        #[quickjs(rename = "isTag")]
        pub fn is_tag(&self, tag: String) -> bool {
            false
        }

        #[quickjs(rename = "hasTag")]
        pub fn has_tag(&self, tag: String) -> bool {
            false
        }

        #[quickjs(rename = "createElement")]
        pub fn create_element<'js>(
            &self,
            ctx: Ctx<'js>,
            tag: String,
            args: Rest<Value<'js>>,
        ) -> Result<Class<'js, QObject>> {
            let cls = get_class(Qt::get_schema(), &tag)?;
            if let Some(constructor) = cls.constructor {
                tracing::info!(%tag, "Created {}", tag);
                let instance = Class::instance(ctx, (*constructor)(ctx, args.into_inner())?)?;
                Ok(instance)
            } else {
                tracing::error!(%tag, "Failed creating {}, no constructor", tag);
                Err(Error::Exception {
                    message: format!("Element {} has no constructor", tag),
                    file: "".to_string(),
                    line: -1,
                    stack: "".to_string(),
                })
            }
        }

        #[quickjs(rename = "createElementNS")]
        pub fn create_element_ns<'js>(
            &self,
            ctx: Ctx<'js>,
            _ns: String,
            tag: String,
            args: Rest<Value<'js>>,
        ) -> Result<Class<'js, QObject>> {
            self.create_element(ctx, tag, args)
        }

        #[quickjs(rename = "setAttribute")]
        pub fn set_attr<'js>(&self, ctx: Ctx<'js>, name: String, value: Value<'js>) -> Result<()> {
            todo()
        }

        #[quickjs(rename = "removeAttribute")]
        pub fn remove_attr<'js>(&self, ctx: Ctx<'js>, name: String) -> Result<()> {
            Ok(())
        }

        #[quickjs(rename = "getAttribute")]
        pub fn get_attr<'js>(&mut self, ctx: Ctx<'js>, name: String) -> Result<Value<'js>> {
            todo()
        }

        #[quickjs(rename = "createTextNode")]
        pub fn create_text_node<'js>(&self, ctx: Ctx<'js>, text: String) -> Result<Value<'js>> {
            Ok(text.into_js(ctx)?)
        }

        #[quickjs(rename = "callMethod")]
        pub fn call_method<'js>(
            &self,
            ctx: Ctx<'js>,
            name: String,
            args: Rest<Value<'js>>,
        ) -> Result<Value<'js>> {
            todo()
        }

        #[quickjs(rename = "addEventListener")]
        pub fn add_event_listener<'js>(
            &mut self,
            ctx: Ctx<'js>,
            name: String,
            func: Function<'js>,
        ) -> Result<()> {
            todo()
        }

        #[quickjs(rename = "removeEventListener")]
        pub fn remove_event_listener<'js>(
            &mut self,
            ctx: Ctx<'js>,
            name: String,
            func: Function<'js>,
        ) -> Result<()> {
            // let hadnerl
            todo()
        }
    }
}

pub(in crate) fn todo<T>() -> Result<T> {
    Err(Error::Exception {
        message: "NOT IMPLEMENTED".to_string(),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}
