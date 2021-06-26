use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Loaded, Module, Native};

use crate::inner::*;

impl FrameworkModule<"QtCore"> for QT {
    type Output = impl ModuleDef;

    fn get_module() -> Self::Output {
        hlist![QT::class::<"QObject">(), QT::class::<"QObject">()]
    }
}

impl FrameworkClass<"QObject"> for QT {
    type Proto;
    type Static;
    type Constructor<'a>;
    type Type = QObject;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn get_proto() -> &'static Self::Proto {
        todo!()
    }

    fn get_static() -> &'static Self::Static {
        todo!()
    }

    fn get_constructor<'a>() -> &'static Self::Constructor<'a> {
        todo!()
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
