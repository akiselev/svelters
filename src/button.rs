use qmetaobject::{
    connections::{connect, Signal, SignalInner},
    *,
};
use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};

use crate::{QWidgetPtr, Widget};

cpp! {{
    #include <memory>
    #include <QtWidgets/QPushButton>
    #include <QtWidgets/QAbstractButton>
    #include <QtCore/QPointer>
    #include <QtCore/QStringList>
    #include <QtCore/QString>
    #include <QtCore/QMetaObject>
    #include <QtCore/QDebug>
    #include <QtCore/QMetaMethod>
    #include <qmetaobject_rust.hpp>

    #include <string>
    #include <iostream>
}}

pub fn qpushbutton_new() -> Widget {
    let result = cpp!(unsafe [] -> QWidgetPtr as "QPointer<QWidget>" {
        QPointer<QWidget> btn = new QPushButton("testing button");
        return btn;
    });

    // run destructor
    Widget::new(result)
}

pub fn qpushbutton_clicked() -> Signal<fn(bool)> {
    unsafe {
        Signal::new(cpp!([] -> SignalInner as "SignalInner"  {
            return &QAbstractButton::clicked;
        }))
    }
}
