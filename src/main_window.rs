use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};

use crate::Widget;

cpp! {{
    #include <memory>
    #include <QtWidgets/QMainWindow>
    #include <QtCore/QPointer>
}}

cpp_class!(
    /// Wrap a Qt QMainWindow
    ///
    /// Note that since there can only be one Application in the process, creating two
    /// QMainWindow at the same time is not allowed. Doing that will panic.
    pub unsafe struct QMainWindow as "QPointer<QMainWindow>"
);
impl QMainWindow {
    /// Create a new MainWindow
    pub fn new() -> QMainWindow {
        let result = cpp!(unsafe [] -> QMainWindow as "QPointer<QMainWindow>" {
            return new QMainWindow();
        });

        // run destructor
        result
    }

    /// Show main window
    pub fn show(&self) {
        cpp!(unsafe [self as "QPointer<QMainWindow> *"] {
            (*self)->resize(250, 150);
            (*self)->setWindowTitle("Simple example");
            (*self)->show();
        })
    }

    /// Set Central Widget
    pub fn set_central_widget(&self, widget: &Widget) {
        let ptr = &widget.ptr;
        cpp!(unsafe [self as "QPointer<QMainWindow> *", ptr as "QPointer<QWidget> *"] {
            (*self)->setCentralWidget(*ptr);
        })
    }
}
