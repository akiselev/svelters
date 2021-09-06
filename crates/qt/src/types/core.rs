use std::{ffi::CStr, ops::Deref, os::raw::c_char};

use frunk::hlist::IntoTuple2;
use qt_core::{
    cpp_core::{CastFrom, CppBox, CppDeletable, NullPtr, Ptr, Ref, StaticDowncast, StaticUpcast},
    QBox, QCoreApplication, QCoreApplicationArgs, QObject, QPtr, QString, SlotOfBool,
};
use qt_gui::QWindow;
use qt_widgets::{QAbstractButton, QApplication, QMainWindow, QPushButton, QWidget};
use rquickjs::{AsFunction, Class, Loaded, Module as QJsModule, Native};

use crate::{
    impls::containers::{DerefWrapper, Qt},
    inner::*,
    QtCore, QtWidgets,
};

define!(|cls: QObject| {
    cls.constructor0(|| unsafe { Ok(Qt::new(QObject::new_0a())) })?
        .method0("className", |this: &Qt<QObject>| -> Result<String> {
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
        })?
        .method0("test", |this: &Qt<QObject>| {
            println!("REAL >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?
        .static_fn0("staticTest", || {
            println!("STATIC >>>>>>>>>>>>>>>>>>");
            Ok(())
        })?;
});

define!(|cls: QWidget| {
    cls.constructor0(|| unsafe { Ok(Qt::new(QWidget::new_0a())) })?
        .method0("show", |this: &Qt<QWidget>| unsafe {
            this.show();
            Ok(())
        })?
        .method0("hide", |this: &Qt<QWidget>| unsafe {
            this.hide();
            Ok(())
        })?
        .superclass::<Qt<QObject>>()?;
});

define!(|cls: QApplication| {
    // cls.static_fn0("exec", || unsafe {
    //     QCoreApplication::exec();
    // })?
    cls.constructor0(|| unsafe {
        let mut args = QCoreApplicationArgs::new();
        let (argc, argv) = args.get();
        std::mem::forget(args);
        Ok(Qt::new(QApplication::new_2a(argc, argv)))
    })?
    .superclass::<Qt<QObject>>()?;
});
