use std::{ffi::CStr, ops::Deref, os::raw::c_char};

use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, Ref, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_gui::QWindow;
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module as QJsModule, Native};

use crate::{
    impls::containers::{DerefWrapper, QtWrapper},
    inner::*,
    QtCore,
};

impl Module<QT> for QtCore {
    fn define<P>(module: &mut ModuleBuilder<QT, Self, P>) -> Result<()>
    where
        QT: Framework,
    {
        module.class::<QObject>()?;
        module.class::<QWidget>()?;
        module.class::<QApplication>()?;
        module.class::<QMainWindow>()?;
        Ok(())
    }
}

impl Repr<QT> for QObject {
    type Type = QtWrapper<QObject>;
}

impl Type<QT> for QObject {
    const CLASS_NAME: &'static str = "QObject";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    fn define<P>(cls: &mut TypeBuilder<QT, Self, P>) -> Result<()> {
        cls.constructor0(|| {
            unsafe { QObject::new_0a() }
            // cls.constructor1(|var: QString| {
            // let mut args = QCoreApplicationArgs::new();
            // let (argc, argv) = args.get();
            // std::mem::forget(args);
            // QApplication::new_2a(argc, argv)
        })?;
        cls.method0("className", |this: &QObject| -> Result<String> {
            let name = unsafe {
                let obj = this.meta_object();
                CStr::from_ptr(obj.class_name() as *const c_char).to_owned()
            };
            name.into_string().map_err(|err| Error::Exception {
                message: err.to_string(),
                file: file!().to_string(),
                line: line!().try_into().unwrap(),
                stack: "".to_string(),
                rust_stack: Some("place2".to_string()),
            })
        })?;
        cls.method0("test", |this: &QObject| {
            println!("REAL >>>>>>>>>>>>>>>>>>");
        })?;
        cls.static_fn0("staticTest", || {
            println!("STATIC >>>>>>>>>>>>>>>>>>");
        })?;
        Ok(())
    }
}

impl Repr<QT> for QWidget {
    type Type = QtWrapper<QWidget>;
}

impl Type<QT> for QWidget {
    const CLASS_NAME: &'static str = "QWidget";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    fn define<P>(cls: &mut TypeBuilder<QT, Self, P>) -> Result<()> {
        cls.constructor0(|| {
            unsafe { QWidget::new_0a() }
            // cls.constructor1(|var: QString| {
            // let mut args = QCoreApplicationArgs::new();
            // let (argc, argv) = args.get();
            // std::mem::forget(args);
            // QApplication::new_2a(argc, argv)
        })?;
        cls.method0("show", |this: &QWidget| unsafe {
            this.show();
        })?;
        cls.method0("hide", |this: &QWidget| unsafe {
            this.hide();
        })?;
        cls.superclass::<QObject>()?;
        Ok(())
    }
}

impl Repr<QT> for QApplication {
    type Type = QtWrapper<QApplication>;
}

impl Type<QT> for QApplication {
    const CLASS_NAME: &'static str = "QApplication";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    fn define<P>(cls: &mut TypeBuilder<QT, Self, P>) -> Result<()> {
        // cls.static_fn0("exec", || unsafe {
        //     QCoreApplication::exec();
        // })?;
        cls.constructor0(|| unsafe {
            let mut args = QCoreApplicationArgs::new();
            let (argc, argv) = args.get();
            std::mem::forget(args);
            QApplication::new_2a(argc, argv)
        })?;
        Ok(())
    }
}

impl Repr<QT> for QMainWindow {
    type Type = QtWrapper<QMainWindow>;
}

impl Type<QT> for QMainWindow {
    const CLASS_NAME: &'static str = "QMainWindow";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    fn define<P>(cls: &mut TypeBuilder<QT, Self, P>) -> Result<()> {
        cls.static_fn0("exec", || unsafe {
            QCoreApplication::exec();
        })?;
        cls.constructor0(|| unsafe { QMainWindow::new_0a() })?;
        cls.superclass::<QWidget>()?;
        Ok(())
    }
}

// static QOBJECT: impl Instantiate =
//     qt.class::<QObject, _>(hlist![qt.init(|| unsafe { QObject::new_0a() })]);
// static QWIDGET: impl Instantiate =
//     qt.class::<QWidget, _>(hlist![qt.init(|| unsafe { QWidget::new_0a() })]);
// static QMAINWINDOW: impl Instantiate =
//     qt.class::<QMainWindow, _>(hlist![qt.init(|| unsafe { QMainWindow::new_0a() })]);
// static QPUSHBUTTOn: impl Instantiate =
//     qt.class::<QPushButton, _>(hlist![qt.init(|| unsafe { QPushButton::new() })]);
