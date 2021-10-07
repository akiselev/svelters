#![allow(warnings)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(in_band_lifetimes)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(proc_macro_hygiene)]
#![feature(let_chains)]
#![feature(trait_alias)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]
#![feature(associated_type_bounds)]
#![recursion_limit = "1024"]

use std::{any::Any, borrow::Cow, future::Future, marker::PhantomData, rc::Rc, time::Duration};

use glib::MainContext;
use gtk4::{prelude::*, Application, Widget};
use macros::props;
use rquickjs::{
    BuiltinResolver, ClassDef, Context, Ctx, Error, Executor, ExecutorSpawner, FromJs, Function,
    IntoJs, Loader, ModuleLoader, Object, Resolver, Result, This, Value,
};
use smallbox::smallbox;
use static_assertions::assert_impl_all;
use svelters_core::{exception, prelude::*};

use crate::application::application_window;
pub use crate::wrapper::Gtk;

pub(in crate) mod application;
pub(in crate) mod core;
pub(in crate) mod wip;

mod elements;
mod jsenum;
mod macros;
mod wrapper;

pub mod impls {
    mod event;
    mod method;
    mod property;

    pub use event::*;
    pub use method::*;
    pub use property::*;
}

pub(in crate) mod inner {
    pub(in crate) use crate::impls::*;
    pub(in crate) use std::{
        borrow::Cow,
        cell::Cell,
        convert::{TryFrom, TryInto},
        marker::PhantomData,
        ops::{Add, Deref, DerefMut},
    };

    pub(in crate) use frunk::{hlist, hlist::Selector, HCons, HNil};
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use rquickjs::{
        ClassDef, ClassId, Context, Ctx, Error, FromJs, IntoJs, ModuleDef, Object, RefsMarker,
        Result, Value,
    };

    pub(in crate) use svelters_core::{exception, prelude::*};

    pub(in crate) use crate::jsenum::*;
    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::wrapper::Gtk;
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

pub fn initialize_gtk(app: Application) -> impl FnOnce(&Svelters) -> Result<()> {
    move |sv| {
        sv.spawn_executor(GtkExecutor);
        sv.with_types(|builder| elements::initialize_elements(builder))?;
        sv.with_js(|ctx| {
            rquickjs::Class::<Gtk<Application>>::register(ctx)?;
            let app = Gtk(app);
            ctx.globals().set("application", app)?;
            let doc_obj: Object = ctx.globals().get("document")?;
            let func: Function = doc_obj.get("createElement")?;
            let window: Value = func.call((This(doc_obj), "applicationWindow"))?;
            ctx.globals().set("window", window)?;
            Result::<_>::Ok(())
        })?;
        sv.execute_pending_jobs().unwrap();
        Ok(())
    }
}
