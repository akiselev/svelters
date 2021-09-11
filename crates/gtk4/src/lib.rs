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
#![recursion_limit = "1024"]

use std::{borrow::Cow, future::Future, time::Duration};

use glib::MainContext;
use rquickjs::{
    BuiltinResolver, Context, Ctx, Error, Executor, ExecutorSpawner, Loader, ModuleLoader,
    Resolver, Result, Value,
};
use static_assertions::assert_impl_all;
use svelters_core::prelude::*;

pub use crate::wrapper::Gtk;

mod macros;
mod wrapper;

mod types {
    mod core;
    mod windows;
}

pub(in crate) mod inner {
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

    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::wrapper::Gtk;
}
mod module {
    use super::Gtk;
    use glib::object::Object as GObject;
    use gtk4::{Application, ApplicationWindow, Window};

    crate::macros::module!(Gtk: GObject, Application, Window, ApplicationWindow);
}

pub struct GtkExecutor;

impl ExecutorSpawner for GtkExecutor {
    type JoinHandle = ();

    fn spawn_executor(self, task: Executor) -> Self::JoinHandle {
        let ctx = MainContext::default();
        ctx.spawn_local(task)
    }
}
