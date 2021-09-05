#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(in_band_lifetimes)]
#![recursion_limit = "1024"]

use std::{borrow::Cow, future::Future, time::Duration};

use frunk::hlist;
pub use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
pub use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{
    BuiltinResolver, Context, Ctx, Error, Loader, ModuleLoader, Resolver, Result, Value,
};
use static_assertions::assert_impl_all;
use svelters_core::prelude::*;

use crate::impls::containers::Qt;

#[derive(Debug, Clone, Copy)]
pub struct QT;
pub struct QtCore;

impl Framework for QT {
    type Loader = impl Loader;
    type Resolver = impl Resolver;
    type AppTask = impl Future<Output = anyhow::Result<()>>;

    fn init_app(ctx: Context) -> Result<Self::AppTask> {
        Ok(Box::pin(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            let app = unsafe {
                let mut args = QCoreApplicationArgs::new();
                let (argc, argv) = args.get();
                // This prevents a segfault
                std::mem::forget(args);

                QApplication::new_2a(argc, argv)
            };
            loop {
                unsafe {
                    QCoreApplication::process_events_0a();
                    QCoreApplication::send_posted_events_0a();
                    QCoreApplication::process_events_0a();
                }
                interval.tick().await;
            }
            Ok(())
        }))
    }

    fn get_loader() -> (Self::Resolver, Self::Loader) {
        let resolver = BuiltinResolver::default().with_module("QtCore");
        let loader = ModuleLoader::default().with_module("QtCore", QT::module(QtCore));
        (resolver, loader)
    }

    fn name() -> &'static str {
        "QT"
    }
}

mod modules;
mod impls {
    pub mod containers;
}

pub(in crate) mod inner {
    pub(in crate) use std::{
        borrow::Cow,
        cell::Cell,
        convert::{TryFrom, TryInto},
        marker::PhantomData,
        ops::{Add, Deref, DerefMut},
    };

    pub(in crate) use frunk::{hlist, hlist::Selector, HCons, HNil, Hlist};
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use qt_core::cpp_core::{
        CastFrom, CppBox, CppDeletable, DynamicCast, Ptr, StaticUpcast,
    };
    pub(in crate) use qt_core::{QBox, QObject, QPtr};
    pub(in crate) use rquickjs::{
        ClassDef, ClassId, Context, Ctx, Error, FromJs, IntoJs, ModuleDef, Object, RefsMarker,
        Result, Value,
    };

    pub(in crate) use svelters_core::prelude::*;

    pub(in crate) use crate::QT;
}
