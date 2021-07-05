use std::ops::Deref;

use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, Ref, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_gui::QWindow;
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module, Native};

use crate::{
    impls::containers::{DerefWrapper, QtWrapper},
    inner::*,
    QtCore,
};

impl FrameworkModule<QT> for QtCore {
    fn define<P>(module: JsModule<QT, Self, P>) -> Result<()>
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

impl TypeDef<QObject> for QT {
    const CLASS_NAME: &'static str = "QObject";

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P>(cls: Type<Self, QObject, P>) -> Result<()> {
        cls.constructor0(|| {
            unsafe { QObject::new_0a() }
            // cls.constructor1(|var: QString| {
            // let mut args = QCoreApplicationArgs::new();
            // let (argc, argv) = args.get();
            // std::mem::forget(args);
            // QApplication::new_2a(argc, argv)
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

impl TypeDef<QWidget> for QT {
    const CLASS_NAME: &'static str = "QWidget";

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P>(cls: Type<Self, QWidget, P>) -> Result<()> {
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

impl TypeDef<QApplication> for QT {
    const CLASS_NAME: &'static str = "QApplication";

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P>(cls: Type<Self, QApplication, P>) -> Result<()> {
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

impl TypeDef<QMainWindow> for QT {
    const CLASS_NAME: &'static str = "QMainWindow";

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P>(cls: Type<Self, QMainWindow, P>) -> Result<()> {
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
