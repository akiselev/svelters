use std::future::Future;
use std::time::Duration;

use futures_util::stream::StreamExt;
use glib::MainContext;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use once_cell::unsync::OnceCell;
use rquickjs::{Context, Ctx, Executor, ExecutorSpawner, IntoJs, Runtime};

use crate::{initialize_types, LogError};

#[derive(Default)]
pub struct Svelters {
    rt: OnceCell<Runtime>,
    ctx: OnceCell<Context>,
}

// impl Drop for Svelters {
//     fn drop(&mut self) {
//         match cleanup_context(self.context()) {
//             Ok(_) => {}
//             Err(err) => tracing::error!("Error dropping Svelters: {}", err),
//         }
//     }
// }

impl Svelters {
    pub fn runtime(&self) -> &Runtime {
        self.rt.get_or_init(|| {
            rquickjs::Runtime::new().expect("Could not initialize Javascript Runtime")
        })
    }

    pub fn context(&self) -> &Context {
        self.ctx.get_or_init(|| {
            rquickjs::Context::full(self.runtime())
                .expect("Could not initialize Javascript Context")
        })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Svelters {
    const NAME: &'static str = "SveltersApplication";
    type Type = super::SveltersApplication;
    type ParentType = gtk4::Application;
}

impl ObjectImpl for Svelters {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        tracing::warn!("Svelters constructed");
    }

    fn dispose(&self, _obj: &Self::Type) {
        // self.parent_dispose(obj);
        tracing::warn!("Svelters disposed");
    }
}

/// When our application starts, the `startup` signal will be fired.
/// This gives us a chance to perform initialisation tasks that are not directly
/// related to showing a new window. After this, depending on how
/// the application is started, either `activate` or `open` will be called next.
impl ApplicationImpl for Svelters {
    /// `gio::Application::activate` is what gets called when the
    /// application is launched by the desktop environment and
    /// aksed to present itself.
    fn activate(&self, app: &Self::Type) {
        tracing::warn!("SveltersApplication::activate");
        let app = (*app.downcast_ref::<super::SveltersApplication>().unwrap()).clone();
        spawn_local(async move {
            let mut stream = glib::interval_stream_seconds(10);
            while stream.next().await.is_some() {
                app.runtime().run_gc();
            }
        });
    }

    fn startup(&self, app: &Self::Type) {
        tracing::warn!("SveltersApplication::startup");
        self.parent_startup(app);

        let app = app.downcast_ref::<super::SveltersApplication>().unwrap();
        let inner = Svelters::from_instance(app);
        inner.runtime().spawn_executor(GtkExecutor);
        let res = inner.context().with(|ctx| {
            crate::modules::console::init(ctx)?;
            initialize_types(ctx)?;
            let globals = ctx.globals();
            globals.set("document", rquickjs::Object::new(ctx)?)?;
            let app = crate::newtype::Gtk::new((*app).clone());
            globals.set("application", app.into_js(ctx)?)?;

            let obj = rquickjs::Object::new(ctx)?;
            globals.set("window", obj)?;

            rquickjs::Result::Ok(())
        });
        inner.runtime().run_gc();
        if let Err(err) = res {
            tracing::error!("Error initializing Svelters: {}", err);
            panic!("Could not initialize Svelters");
        }
    }

    fn shutdown(&self, app: &Self::Type) {
        self.parent_shutdown(app);

        let app = app.downcast_ref::<super::SveltersApplication>().unwrap();
        let inner = Svelters::from_instance(app);

        tracing::warn!("Svelters::Application::shutdown");
    }
}

impl GtkApplicationImpl for Svelters {}

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
