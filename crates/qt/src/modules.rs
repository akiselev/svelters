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
    type Output = impl JsModuleDef;

    fn get_module() -> &'static Self::Output {
        static MODULE: impl JsModuleDef = hlist![
            include::<QT, "QObject">(),
            include::<QT, "QApplication">(),
            include::<QT, "QMainWindow">()
        ];
        &MODULE
    }
}

impl FrameworkClass<"QObject"> for QT {
    type Proto = impl ClassProto;
    type Static = impl ClassStatic;
    type Constructor = impl JsModuleDef;
    type Type = QObject;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn get_proto() -> &'static Self::Proto {
        static PROTO: impl ClassProto = {
            let cls = include::<QT, "QObject">();
            hlist![cls.method("className", |this: &QObject| unsafe {
                this.meta_object().class_name()
            }),]
        };
        &PROTO
    }

    fn get_static() -> &'static Self::Static {
        static STATIC: impl ClassStatic = hlist![];
        &STATIC
    }

    fn get_constructor() -> &'static Self::Constructor {
        static CONSTRUCTOR: impl JsModuleDef = {
            let cls = include::<QT, "QObject">();
            hlist![cls.constructor(|| unsafe { QObject::new_0a() })]
        };
        &CONSTRUCTOR
    }
}

impl FrameworkClass<"QApplication"> for QT {
    type Proto = impl ClassProto;
    type Static = impl ClassStatic;
    type Constructor = impl JsModuleDef;
    type Type = QApplication;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn get_proto() -> &'static Self::Proto {
        static PROTO: impl ClassProto = hlist![];
        &PROTO
    }

    fn get_static() -> &'static Self::Static {
        static STATIC: impl ClassStatic = {
            let cls = include::<QT, "QApplication">();
            hlist![cls.static_fn("exec", || unsafe {
                QCoreApplication::exec();
            })]
        };
        &STATIC
    }

    fn get_constructor() -> &'static Self::Constructor {
        static CONSTRUCTOR: impl JsModuleDef = {
            let cls = include::<QT, "QApplication">();
            hlist![cls.constructor(|| unsafe {
                let mut args = QCoreApplicationArgs::new();
                let (argc, argv) = args.get();
                std::mem::forget(args);
                QApplication::new_2a(argc, argv)
            })]
        };
        &CONSTRUCTOR
    }
}

impl FrameworkClass<"QMainWindow"> for QT {
    type Proto = impl ClassProto;
    type Static = impl ClassStatic;
    type Constructor = impl JsModuleDef;
    type Type = QMainWindow;

    unsafe fn get_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    fn get_proto() -> &'static Self::Proto {
        static PROTO: impl ClassProto = {
            let cls = include::<QT, "QMainWindow">();
            hlist![
                cls.method("show", |this: &QMainWindow| unsafe {
                    this.show();
                }),
                cls.method("hide", |this: &QMainWindow| unsafe {
                    this.hide();
                })
            ]
        };
        &PROTO
    }

    fn get_static() -> &'static Self::Static {
        static STATIC: impl ClassStatic = {
            let cls = include::<QT, "QMainWindow">();
            hlist![cls.static_fn("exec", || unsafe {
                QCoreApplication::exec();
            })]
        };
        &STATIC
    }

    fn get_constructor() -> &'static Self::Constructor {
        static CONSTRUCTOR: impl JsModuleDef = {
            let cls = include::<QT, "QMainWindow">();
            hlist![cls.constructor(|| unsafe { QMainWindow::new_0a() })]
        };
        &CONSTRUCTOR
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
