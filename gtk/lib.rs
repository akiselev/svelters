use std::{any::Any, borrow::Cow, future::Future, marker::PhantomData, rc::Rc, time::Duration};

use gio::{prelude::ApplicationExt, ApplicationFlags};
use glib::{
    translate::{Stash, ToGlibPtr},
    MainContext,
};
use gtk4::{ffi::GtkApplication, prelude::*, Application};

use rquickjs::{
    qjs::{JSContext, JSValue},
    Context, Executor, ExecutorSpawner, ObjectDef, Runtime,
};

pub(crate) mod common;

mod object;

pub use object::*;

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
        ctx.globals().init_def::<Types>()?;

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

pub struct Types;

impl ObjectDef for Types {
    fn init<'js>(ctx: rquickjs::Ctx<'js>, object: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
        todo!()
    }
}
