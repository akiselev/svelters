use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};

use crate::{QWidget, Widget};

cpp! {{
    #include <memory>
    #include <QtWidgets/QPushButton>
    #include <QtCore/QPointer>
}}

pub fn qpushbutton_new() -> Widget {
    let result = cpp!(unsafe [] -> QWidget as "QPointer<QWidget>" {
        return new QPushButton("testing button");
    });

    // run destructor
    Widget::new(result)
}
