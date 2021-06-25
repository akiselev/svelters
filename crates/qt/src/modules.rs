use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Loaded, Module, Native};

use crate::inner::*;

impl FrameworkModule<"QtCore"> for QT {
    type Output = impl JsModuleDef;

    fn get_module() -> Self::Output {
        hlist![QT::class::<"QObject">(), QT::class::<"QObject">()]
    }
}

impl FrameworkClass<"QObject"> for QT {
    type Type = QObject;
    type Wrapper = QPtr<QObject>;
    type Definition = impl JsClassDef;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn get_definition() -> Self::Definition {
        let c = QT::class::<"QObject">();
        HNil
    }

    fn register<'js>(ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        let c = QT::class::<"QObject">();
        // let funcs = hlist![c.constructor(|| unsafe { QObject::new_0a() })];
        rquickjs::Class::<Obj<QT, "QObject">>::register(ctx)?;
        let constructor = rquickjs::Class::<Obj<QT, "QObject">>::constructor(
            c.constructor(|| unsafe { QObject::new_0a() }),
        );
        module.set(
            "QObject",
            rquickjs::Func::new(
                "QObject",
                constructor;
            ),
        )?;

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
