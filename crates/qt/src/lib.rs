#![feature(generic_associated_types)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(in_band_lifetimes)]
#![recursion_limit = "1024"]
use std::{borrow::Cow, future::Future, time::Duration};

use frunk::hlist;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{Context, Ctx, Error, Result, Value};
use static_assertions::assert_impl_all;
use svelters_core::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct QT;

impl Framework for QT {
    type AppTask = impl Future<Output = anyhow::Result<()>>;

    fn init_app<'js>(&self, ctx: Context) -> Result<Self::AppTask> {
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
}

static qt: Core<QT> = Core::new(QT);

static QOBJECT: impl Instantiate =
    qt.class::<QObject, _>(hlist![qt.init(|| unsafe { QObject::new_0a() })]);
static QWIDGET: impl Instantiate =
    qt.class::<QWidget, _>(hlist![qt.init(|| unsafe { QWidget::new_0a() })]);
static QMAINWINDOW: impl Instantiate =
    qt.class::<QMainWindow, _>(hlist![qt.init(|| unsafe { QMainWindow::new_0a() })]);
static QPUSHBUTTOn: impl Instantiate =
    qt.class::<QPushButton, _>(hlist![qt.init(|| unsafe { QPushButton::new() })]);

mod impls {
    mod cast;
    mod containers;
}

pub(in crate) mod inner {
    pub(in crate) use std::borrow::Cow;
    pub(in crate) use std::cell::Cell;
    pub(in crate) use std::marker::PhantomData;
    pub(in crate) use std::ops::{Add, Deref, DerefMut};

    pub(in crate) use frunk::hlist::Selector;
    pub(in crate) use frunk::{HCons, HNil};
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use qt_core::cpp_core::{
        CastFrom, CppBox, CppDeletable, DynamicCast, Ptr, StaticUpcast,
    };
    pub(in crate) use qt_core::{QBox, QObject, QPtr};
    pub(in crate) use rquickjs::{
        ClassDef, ClassId, Context, Ctx, Error, FromJs, IntoJs, Object, RefsMarker, Result, Value,
    };

    pub(in crate) use svelters_core::prelude::*;

    pub(in crate) use crate::QT;
}
