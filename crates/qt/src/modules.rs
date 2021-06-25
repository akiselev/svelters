use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, Ptr, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString,
};
use qt_widgets::{QApplication, QMainWindow, QPushButton, QWidget};

use crate::inner::*;

impl FrameworkModule<"QtCore"> for QT {
    type Output = impl ModuleDef;

    fn get_module() -> Self::Output {
        JsModule::new(hlist![&QOBJECT])
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
