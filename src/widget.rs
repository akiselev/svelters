use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};
use qmetaobject::connections::ConnectionHandle;

pub struct Widget {
    pub(crate) ptr: QWidget,
    pub(crate) handles: HashMap<&'static str, ConnectionHandle>,
}

cpp! {{
    #include <memory>
    #include <QtCore/QPointer>
    #include <QtCore/QObject>
    #include <QtWidgets/QWidget>
}}

cpp_class!(
    /// Wrap a Qt QWidget
    pub unsafe struct QWidget as "QPointer<QWidget>"
);

impl Widget {
    pub fn new(ptr: QWidget) -> Widget {
        let handles = HashMap::new();
        Widget { ptr, handles }
    }

    pub unsafe fn as_sender(&self) -> *const c_void {
        cpp!(unsafe [self as "QPointer<QWidget> *"] -> *const c_void as "void *" {
            return (QObject*) *self;
        })
    }

    pub fn add_handle(&mut self, name: &'static str, handle: ConnectionHandle) {
        self.handles.insert(name, handle);
    }

    // pub fn exec(&self) {
    //     cpp!(unsafe [self as "QPointer<QWidget> *"] {
    //         qode::InjectCustomRunLoop((*self)->exec);
    //     })
    // }
}
