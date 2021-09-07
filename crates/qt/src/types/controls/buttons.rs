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
    QtCore,
};

define!(|cls: QAbstractButton| {
    props!(cls(this: QAbstractButton) {
        text: Q<QString> {
            Q::Box(this.text());
            this.set_text(text.to_ref())
        };
    });

    cls.superclass::<Qt<QWidget>>()?;
    event!(cls.clicked as SlotOfBool[val](val));
});

fn test() {}

define!(|cls: QPushButton| {
    cls.constructor0(|| unsafe { Ok(Qt::new(QPushButton::new())) })?
        .as_element("qPushButton", || unsafe { Ok(Qt::new(QPushButton::new())) })?
        .superclass::<Qt<QAbstractButton>>()?;
});
