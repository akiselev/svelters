#![feature(associated_type_bounds)]
#![feature(backtrace)]
#![feature(decl_macro)]

use std::future::Future;

use gio::prelude::ApplicationExt;
use glib::MainContext;
use gtk4::{prelude::*, Application};

use rquickjs::{ClassDef, Context, Ctx, Executor, ExecutorSpawner, IntoJs, ObjectDef};

pub(crate) mod common;

mod core;
mod object;
mod types;
mod widget;

pub use object::*;
pub use widget::*;

use common::Gtk;

pub struct Types;

impl ObjectDef for Types {
    fn init<'js>(ctx: rquickjs::Ctx<'js>, object: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
        ctx.register_type::<Object>("Object")?;
        ctx.register_type::<Gtk<gtk4::Widget>>("Widget")?;
        ctx.register_type::<Gtk<gtk4::Application>>("Application")?;
        ctx.register_type::<Gtk<gtk4::ApplicationWindow>>("ApplicationWindow")?;
        // ctx.register_type::<Gtk<gtk4::Window>>("Window")?;
        ctx.register_type::<Gtk<gtk4::Button>>("Button")?;
        ctx.register_type::<Gtk<gtk4::Box>>("Box")?;
        Ok(())
    }
}

pub trait RegisterType {
    fn register_type<T: ClassDef>(&self, name: &str) -> rquickjs::Result<()>;
}

impl<'js> RegisterType for Ctx<'js> {
    fn register_type<T: ClassDef>(&self, name: &str) -> rquickjs::Result<()> {
        rquickjs::Class::<T>::register(*self)?;
        let proto = rquickjs::Class::<T>::prototype(*self)?;
        self.globals().set(name, proto)
    }
}

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
        let globals = ctx.globals();
        globals.init_def::<Types>()?;
        globals.set("document", rquickjs::Object::new(ctx)?)?;
        let app = types::application::Application::new(app.clone());
        globals.set("application", app.into_js(ctx)?)?;

        rquickjs::Result::<_>::Ok(())
    })?;

    Ok(app)
}

pub fn start<FUNC: 'static + Fn(&Application)>(app: Application, func: FUNC) {
    app.connect_activate(move |app| {
        func(app);
    });

    app.run();
}
