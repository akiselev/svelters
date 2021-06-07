use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};

pub struct Widget {
    pub(crate) ptr: QWidget,
}

cpp! {{
    #include <memory>
    #include <QtCore/QPointer>
    #include <QtWidgets/QWidget>
}}

cpp_class!(
    /// Wrap a Qt QWidget
    pub unsafe struct QWidget as "QPointer<QWidget>"
);
impl Widget {
    pub fn new(ptr: QWidget) -> Widget {
        Widget { ptr }
    }

    // pub fn exec(&self) {
    //     cpp!(unsafe [self as "QPointer<QWidget> *"] {
    //         qode::InjectCustomRunLoop((*self)->exec);
    //     })
    // }
}
