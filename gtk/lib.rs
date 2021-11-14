#![feature(associated_type_bounds)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(unboxed_closures)]

use std::future::Future;

use gio::prelude::ApplicationExt;
use glib::MainContext;
use gtk4::{prelude::*, Application};

use rquickjs::{
    AsFunction, ClassDef, Constructor, Context, Ctx, Executor, ExecutorSpawner, IntoJs, ObjectDef,
};

pub(crate) mod common;

mod core;
pub(crate) mod errors;
pub(crate) mod newtype;
pub(crate) mod obj_data;
mod object;
pub(crate) mod traits;
mod types;
mod widget;

pub use object::*;
pub use widget::*;

use crate::core::enums::Enum;
use common::Gtk;

pub struct Types;

impl ObjectDef for Types {
    fn init<'js>(ctx: rquickjs::Ctx<'js>, object: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
        ctx.register_type::<Object>("GObject")?;
        ctx.register_type::<Gtk<gtk4::Widget>>("Widget")?;
        ctx.register_type::<Gtk<gtk4::Application>>("Application")?;
        ctx.register_constructor("CssProvider", || Ok(Gtk(gtk4::CssProvider::new())))?;
        ctx.register_constructor("Button", || Ok(Gtk(gtk4::Button::new())))?;
        ctx.register_constructor("Window", || Ok(Gtk(gtk4::Window::new())))?;
        ctx.register_constructor("ApplicationWindow", |ctx: Ctx<'js>| {
            let globals = ctx.globals();
            let app = globals.get::<_, Gtk<Application>>("application")?;
            Ok(Gtk(gtk4::ApplicationWindow::new(&app.0)))
        })?;
        ctx.register_constructor("Box", |orientation: Enum<gtk4::Orientation>| {
            Ok(Gtk(gtk4::Box::new(orientation.into(), 0)))
        })?;
        // ctx.register_type::<Gtk<gtk4::Window>>("Window")?;
        Ok(())
    }
}

pub trait RegisterType<'js> {
    fn register_type<T: ClassDef>(&self, name: &str) -> rquickjs::Result<()>;

    fn register_constructor<FN, A, T>(&self, name: &str, func: FN) -> rquickjs::Result<()>
    where
        T: 'static + ClassDef,
        FN: 'static + Fn<A, Output = rquickjs::Result<T>>,
        Constructor<T, FN>: AsFunction<'js, A, rquickjs::Result<T>>;
}

impl<'js> RegisterType<'js> for Ctx<'js> {
    fn register_type<T: ClassDef>(&self, name: &str) -> rquickjs::Result<()> {
        rquickjs::Class::<T>::register(*self)?;
        let proto = rquickjs::Class::<T>::prototype(*self)?;
        self.globals().set(name, proto)
    }

    fn register_constructor<FN, A, T>(&self, name: &str, func: FN) -> rquickjs::Result<()>
    where
        T: 'static + ClassDef,
        FN: 'static + Fn<A, Output = rquickjs::Result<T>>,
        Constructor<T, FN>: AsFunction<'js, A, rquickjs::Result<T>>,
    {
        rquickjs::Class::<T>::register(*self)?;
        self.globals().set(
            name,
            rquickjs::Func::new(name, rquickjs::Class::<T>::constructor(func)),
        )
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
    app.connect_startup(move |app| {
        // func(app);
    });

    app.connect_activate(move |app| {
        func(app);
    });

    app.run();
}
