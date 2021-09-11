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

use rquickjs::{
    BuiltinResolver, Context, Ctx, Error, Loader, ModuleLoader, Resolver, Result, Value,
};
use static_assertions::assert_impl_all;
use svelters_core::prelude::*;

pub use crate::wrapper::Gtk;

mod macros;
mod wrapper;

mod types {
    mod object;
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

    pub(in crate) use svelters_core::prelude::*;

    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::wrapper::Gtk;
}

macros::module!(Gtk: glib::object::Object,);
