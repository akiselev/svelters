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

pub struct GtkProp<IMPL: ?Sized, GET, GRET, SET, SIN> {
    interface: &'static str,
    name: &'static str,
    getter: GET,
    setter: SET,
    marker: PhantomData<(GRET, SIN, IMPL)>,
}

impl<GET, GRET, SET, SIN> GtkProp<(), GET, GRET, SET, SIN> {
    #[inline(always)]
    pub fn new<IMPL>(
        interface: &'static str,
        name: &'static str,
        getter: GET,
        setter: SET,
    ) -> GtkProp<IMPL, GET, GRET, SET, SIN>
    where
        IMPL: 'static,
        GET: 'static + for<'js> Fn(Ctx<'js>, &IMPL) -> GRET,
        SET: 'static + for<'js> Fn(Ctx<'js>, &IMPL, SIN) -> Result<()>,
        SIN: 'static + for<'js> FromJs<'js>,
        GRET: 'static + for<'js> IntoJs<'js>,
    {
        GtkProp {
            name,
            getter,
            setter,
            interface,
            marker: PhantomData,
        }
    }
}

impl<IMPL, GET, GRET, SET, SIN> IProperty for GtkProp<IMPL, GET, GRET, SET, SIN>
where
    IMPL: 'static,
    GET: 'static + for<'js> Fn(Ctx<'js>, &IMPL) -> GRET,
    SET: 'static + for<'js> Fn(Ctx<'js>, &IMPL, SIN) -> Result<()>,
    SIN: 'static + for<'js> FromJs<'js>,
    GRET: 'static + for<'js> IntoJs<'js>,
{
    fn name<'a>(&'a self) -> Cow<'a, str> {
        self.name.into()
    }

    fn get<'js>(
        &self,
        ctx: Ctx<'js>,
        element: &dyn IElement,
    ) -> svelters_core::prelude::Result<Value<'js>> {
        let this: &dyn Upcast<IMPL> = element
            .query_ref()
            .ok_or_else(|| exception!("Element does not has an attribute named {}", self.name))?;
        let this = this.upcast();
        (self.getter)(ctx, &this).into_js(ctx)
    }

    fn set<'js>(
        &self,
        ctx: Ctx<'js>,
        element: &dyn IElement,
        value: Value<'js>,
    ) -> svelters_core::prelude::Result<()> {
        let this: &dyn Upcast<IMPL> = element
            .query_ref()
            .ok_or_else(|| exception!("Element does not implement the {} interface", self.name))?;
        let mut this = this.upcast();
        (self.setter)(ctx, &mut this, SIN::from_js(ctx, value)?)
    }
}
