#![feature(associated_type_bounds)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(unboxed_closures)]
#![feature(const_fn_fn_ptr_basics)]

use std::sync::mpsc::SendError;

use application::Svelters;
use glib::MainContext;
use gtk4::{glib::subclass::prelude::*, prelude::*, Application};

use rquickjs::{Context, Ctx};

mod macros;

pub(crate) mod common;
mod core;
pub(crate) mod errors;
pub(crate) mod newtype;
pub(crate) mod obj_data;
mod object;
pub mod registry;
mod types;

pub use object::*;

pub use crate::macros::*;
pub use crate::types::*;

pub use crate::registry::Registry;
use gio::ApplicationFlags;
use gtk4::{gio, glib};

mod application;

pub(crate) mod modules {
    pub(crate) mod console;
    // pub(crate) mod loader;
    // pub(crate) mod timing;
}

// Due to name collision in types::Box
type StdBox<T> = std::boxed::Box<T>;

pub enum SvEvent {
    WithJs(StdBox<dyn Send + for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<()>>),
}

pub struct Handle {
    tx: glib::Sender<SvEvent>,
}

impl Handle {
    pub fn with<FN>(&self, func: FN) -> Result<(), SendError<SvEvent>>
    where
        FN: 'static + Send + for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<()>,
    {
        self.tx.send(SvEvent::WithJs(StdBox::new(func)))
    }
}

glib::wrapper! {
    pub struct SveltersApplication(ObjectSubclass<application::Svelters>)
        @extends gio::Application, gtk4::Application;
}

impl SveltersApplication {
    #[allow(clippy::new_without_default)]
    pub fn new(app_name: &str) -> Self {
        glib::Object::new(&[
            ("application-id", &app_name),
            ("flags", &ApplicationFlags::empty()),
        ])
        .expect("Failed to create SveltersApplication")
    }

    pub fn runtime(&self) -> &rquickjs::Runtime {
        let inner = Svelters::from_instance(&self);
        inner.runtime()
    }

    pub fn context(&self) -> &rquickjs::Context {
        let inner = Svelters::from_instance(&self);
        inner.context()
    }

    pub fn set_loader<R, L>(self, resolver: R, loader: L)
    where
        R: 'static + rquickjs::Resolver,
        L: 'static + rquickjs::Loader,
    {
        let inner = Svelters::from_instance(&self);
        inner.runtime().set_loader(resolver, loader);
    }

    pub fn with<R, FN>(&self, func: FN) -> R
    where
        FN: 'static + for<'js> FnOnce(Ctx<'js>) -> R,
    {
        let inner = Svelters::from_instance(self);
        inner.context().with(func)
    }

    pub fn handle(&self) -> Handle {
        let app = (*self).downgrade();
        let (tx, rx) = MainContext::channel::<SvEvent>(glib::Priority::default());
        rx.attach(None, move |msg| {
            let app = app.upgrade().unwrap();
            match msg {
                SvEvent::WithJs(func) => {
                    if let Err(err) = app.with(func) {
                        tracing::error!("Javascript Error: {}", err);
                    }
                }
            }
            glib::Continue(true)
        });
        Handle { tx }
    }

    #[tracing::instrument(skip(self))]
    pub fn cleanup(&self) {
        let ctx = self.context();
        tracing::warn!("Running gc...");
        ctx.runtime().run_gc();
        ctx.with(move |ctx| {
            tracing::warn!("Cleaning up Svelters...");
            ctx.eval::<(), _>("document?.cleanup?.()").log_error();
            ctx.eval::<(), _>("document?.cleanupStyles?.()").log_error();
            ctx.globals()
                .set("document", rquickjs::Value::new_undefined(ctx))
                .log_error();
            ctx.globals()
                .set("application", rquickjs::Value::new_undefined(ctx))
                .log_error();
            ctx.globals()
                .set("window", rquickjs::Value::new_undefined(ctx))
                .log_error();
            ctx.globals()
                .set("setTimeout", rquickjs::Value::new_undefined(ctx))
                .log_error();
            ctx.globals()
                .set("console", rquickjs::Value::new_undefined(ctx))
                .log_error();
            ctx.clear_registery();
            tracing::warn!("Done cleaning up Svelters...");
            rquickjs::Result::Ok(())
        })
        .log_error();
        tracing::warn!("Running gc again...");
        for i in 0..10 {
            ctx.runtime().run_gc();
        }
    }
}

pub fn initialize_types(ctx: Ctx) -> rquickjs::Result<()> {
    rquickjs::Class::<crate::Object>::register(ctx)?;
    types::generated::initialize_types(ctx)?;
    initialize_custom_types(ctx)?;
    for decl in inventory::iter::<InterfaceDeclaration> {
        (decl.func)(ctx);
    }
    ctx.initialize(SveltersApplication::static_type())?;
    types::generated::initialize_globals(ctx)?;
    initialize_custom_globals(ctx)?;
    Ok(())
}

pub struct InterfaceDeclaration {
    func: for<'x> fn(Ctx<'x>),
}

impl InterfaceDeclaration {
    pub const fn new(func: for<'x> fn(Ctx<'x>)) -> Self {
        Self { func }
    }
}
inventory::collect!(InterfaceDeclaration);

pub trait LogError {
    fn log_error(self);
}
impl<T, E: std::error::Error> LogError for Result<T, E> {
    fn log_error(self) {
        match self {
            Ok(_) => {}
            Err(err) => tracing::error!("Svelters Error: {}", err),
        }
    }
}
