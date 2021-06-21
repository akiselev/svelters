use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;
use std::sync::Arc;

use cpp::{cpp, cpp_class};
use either::{Either, Left, Right};
use qmetaobject::connections::ConnectionHandle;
use rquickjs::{
    bind, Context, Ctx, Error, FromJs, Function, IntoJs, Object, Opt, RegisteryKey, Rest, Result,
    Value,
};
use svelters_core::ObjectKind;
use thiserror::Error as ThisError;

cpp! {{
    #include <memory>
    #include <QtCore/QObject>
    #include <QtCore/QPointer>

}}

cpp_class!(
    pub unsafe struct QPtr as "QPointer<QObject>"
);

cpp_class!(
    pub unsafe struct QRawPtr as "QObject*"
);

#[allow(non_snake_case)]
#[bind(object, public)]
#[quickjs(bare)]
pub mod qobject {

    use svelters_core::{
        get_class, get_event, get_getter, get_insert, get_method, get_setter, Framework, GetterFn,
    };

    use crate::{EventHandle, Qt};

    use super::*;
    use parking_lot::FairMutex;

    pub struct QObject {
        pub(crate) tag: &'static str,
        pub(crate) ptr: Pin<Box<QPtr>>,
        pub(crate) events: HashMap<String, HashMap<RegisteryKey, EventHandle>>,
    }

    impl QObject {
        #[quickjs(skip)]
        pub fn new(tag: &'static str, ptr: Pin<Box<QPtr>>) -> Self {
            let events = HashMap::new();
            QObject { tag, ptr, events }
        }

        #[quickjs(skip)]
        pub fn check_tag(&self, tag: &str) -> Result<()> {
            if self.tag != tag {
                Err(Error::Exception {
                    message: format!("Expected element of type {}, found {}", tag, self.tag),
                    file: "".to_string(),
                    line: -1,
                    stack: "".to_string(),
                })
            } else {
                Ok(())
            }
        }

        #[quickjs(skip)]
        pub fn check_tags(&self, tag: &str) -> Result<()> {
            let cls = get_class(Qt::get_schema(), self.tag)?;
            if cls.implements(tag) {
                Ok(())
            } else {
                Err(Error::Exception {
                    message: format!("Expected element implementing class {}", tag),
                    file: "".to_string(),
                    line: -1,
                    stack: "".to_string(),
                })
            }
        }

        #[quickjs(rename = "isTag")]
        pub fn is_tag(&self, tag: String) -> bool {
            self.check_tag(&tag).is_ok()
        }

        #[quickjs(skip)]
        pub fn raw_ptr(&self) -> *mut c_void {
            let ptr = &*self.ptr;
            cpp!(unsafe [ptr as "const QPointer<QObject> *"] -> *mut c_void as "QObject *" {
                return *ptr;
            })
        }

        #[quickjs(rename = "hasTag")]
        pub fn has_tag(&self, tag: String) -> bool {
            self.check_tags(&tag).is_ok()
        }

        #[quickjs(rename = "setAttribute")]
        pub fn set_attr<'js>(
            &'js mut self,
            ctx: Ctx<'js>,
            name: String,
            value: Value<'js>,
        ) -> Result<()> {
            let setter = get_setter(Qt::get_schema(), self.tag, &name)?;
            (*setter)(ctx, self, value)
        }

        #[quickjs(rename = "removeAttribute")]
        pub fn remove_attr<'js>(&'js self, ctx: Ctx<'js>, name: String) -> Result<()> {
            Ok(())
        }

        #[quickjs(rename = "getAttribute")]
        pub fn get_attr<'js>(&'js mut self, ctx: Ctx<'js>, name: String) -> Result<Value<'js>> {
            let getter = get_getter(Qt::get_schema(), self.tag, &name)?;
            (*getter)(ctx, self)
        }

        #[quickjs(rename = "callMethod")]
        pub fn call_method<'js>(
            &'js mut self,
            ctx: Ctx<'js>,
            name: String,
            args: Rest<Value<'js>>,
        ) -> Result<Value<'js>> {
            let method = get_method(Qt::get_schema(), self.tag, &name)?;
            (*method)(ctx, self, args.into_inner())
        }

        #[quickjs(rename = "insertBefore")]
        pub fn insert_before<'js>(
            &'js mut self,
            ctx: Ctx<'js>,
            obj: Either<&'js mut QObject, String>,
            anchor: Opt<&'js mut QObject>,
        ) -> Result<bool> {
            match obj {
                Left(obj) => {
                    let method = get_insert(Qt::get_schema(), self.tag)?;
                    (*method)(ctx, self, obj, anchor)
                }
                Right(val) => {
                    // Err(Error::new(
                    //     ctx,
                    //     format!("Text nodes are not allowed: {}", val),
                    // ))
                    tracing::warn!(%val, "Text nodes are not allowed: {}", val);
                    Ok(false)
                }
            }
        }

        #[quickjs(rename = "addEventListener")]
        pub fn add_event_listener<'js>(
            &'js mut self,
            ctx: Ctx<'js>,
            name: String,
            func: Function<'js>,
            _options: Opt<Object<'js>>,
        ) -> Result<()> {
            tracing::info!(%name, "Adding event listener for {} on {}", name, self.tag);
            let event = get_event(Qt::get_schema(), self.tag, &name)?;
            let key = ctx.register(func.into());
            let context = Context::from_ctx(ctx)?;
            let conn = { (*event)(context, self, key)? };
            let handlers = self.events.entry(name).or_insert_with(HashMap::default);
            handlers.insert(key, conn);
            Ok(())
        }

        #[quickjs(rename = "removeEventListener")]
        pub fn remove_event_listener<'js>(
            &'js mut self,
            ctx: Ctx<'js>,
            name: String,
            func: Function<'js>,
            _options: Opt<Object<'js>>,
        ) -> Result<()> {
            tracing::info!(%name, "Adding event listener for {} on {}", name, self.tag);
            let event = get_event(Qt::get_schema(), self.tag, &name)?;
            let handlers = self.events.entry(name).or_insert_with(HashMap::default);
            let found: Vec<_> = handlers
                .iter_mut()
                .filter_map(|(key, conn)| match ctx.get_register(*key) {
                    Some(handler) if Some(&func) == handler.as_function() => {
                        ctx.deregister(*key);
                        conn.0.disconnect();
                        Some(*key)
                    }
                    _ => None,
                })
                .collect();
            found.into_iter().for_each(|key| {
                handlers.remove(&key);
            });
            Ok(())
        }
    }
}

impl ObjectKind for qobject::QObject {
    fn get_tag(&self) -> std::borrow::Cow<'static, str> {
        self.tag.into()
    }
}
