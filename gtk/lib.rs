#![feature(associated_type_bounds)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(unboxed_closures)]
#![feature(const_fn_fn_ptr_basics)]

use std::future::Future;

use gio::prelude::ApplicationExt;
use glib::MainContext;
use gtk4::{prelude::*, Application};

use rquickjs::{Context, Ctx, Executor, ExecutorSpawner, IntoJs};

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

pub struct GtkExecutor;

impl ExecutorSpawner for GtkExecutor {
    type JoinHandle = ();

    fn spawn_executor(self, task: Executor) -> Self::JoinHandle {
        let ctx = MainContext::default();
        ctx.spawn_local(task)
    }
}

pub fn spawn(fut: impl 'static + Future<Output = ()> + Send) {
    let ctx = MainContext::default();
    ctx.spawn(fut)
}

pub fn spawn_local(fut: impl 'static + Future<Output = ()>) {
    let ctx = MainContext::default();
    ctx.spawn_local(fut)
}

pub fn initialize_gtk(ctx: &Context, name: &str) -> rquickjs::Result<Application> {
    let app = Application::builder()
        .application_id(name)
        // .flags(ApplicationFlags::HANDLES_OPEN | ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    ctx.with(|ctx| {
        initialize_types(ctx)?;
        let globals = ctx.globals();
        globals.set("document", rquickjs::Object::new(ctx)?)?;
        let app = types::application::Application::new(app.clone());
        globals.set("application", app.into_js(ctx)?)?;

        rquickjs::Result::<_>::Ok(())
    })?;

    Ok(app)
}

pub fn initialize_types(ctx: Ctx) -> rquickjs::Result<()> {
    rquickjs::Class::<crate::Object>::register(ctx)?;
    types::generated::initialize_types(ctx)?;
    for decl in inventory::iter::<InterfaceDeclaration> {
        (decl.func)(ctx);
    }
    types::generated::initialize_global(ctx)?;
    Ok(())
}

pub fn start<FUNC: 'static + Fn(&Application)>(app: Application, func: FUNC) {
    app.connect_activate(move |app| {
        func(app);
    });

    app.run();
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
