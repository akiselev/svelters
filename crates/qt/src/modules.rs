use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_gui::QWindow;
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module, Native};

use crate::inner::*;

impl FrameworkModule<"QtCore"> for QT {
    fn define<P: Phase>(module: JsModule<Self, "QtCore", P>) -> Result<()> {
        module.class::<"QObject">()?;
        module.class::<"QApplication">()?;
        module.class::<"QMainWindow">()
    }
}

impl FrameworkClass<"QObject"> for QT {
    type Type = QObject;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P: Phase>(cls: JsClass<Self, "QObject", P>) -> Result<()> {
        cls.constructor0(|| {
            unsafe { QObject::new_0a() }
            // cls.constructor1(|var: QString| {
            // let mut args = QCoreApplicationArgs::new();
            // let (argc, argv) = args.get();
            // std::mem::forget(args);
            // QApplication::new_2a(argc, argv)
        })?;
        Ok(())
    }
}

impl FrameworkClass<"QApplication"> for QT {
    type Type = QApplication;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P: Phase>(cls: JsClass<Self, "QApplication", P>) -> Result<()> {
        cls.static_fn0("exec", || unsafe {
            QCoreApplication::exec();
        })?;
        cls.constructor0(|| unsafe {
            let mut args = QCoreApplicationArgs::new();
            let (argc, argv) = args.get();
            std::mem::forget(args);
            QApplication::new_2a(argc, argv)
        })?;
        Ok(())
    }
}

impl FrameworkClass<"QMainWindow"> for QT {
    type Type = QMainWindow;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn define<P: Phase>(cls: JsClass<Self, "QMainWindow", P>) -> Result<()> {
        cls.method0("show", |this: &QWidget| unsafe {
            this.show();
        })?;
        cls.method("hide", |this: &QMainWindow| unsafe {
            this.hide();
        })?;
        cls.static_fn("exec", || unsafe {
            QCoreApplication::exec();
        })?;
        cls.constructor0(|| unsafe { QMainWindow::new_0a() })?;
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
