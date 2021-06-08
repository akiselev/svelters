use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};
use qmetaobject::connections::ConnectionHandle;
use qmetaobject::QString;
use qmetaobject::QVariant;

pub struct Widget {
    pub(crate) ptr: QWidgetPtr,
    pub(crate) handles: HashMap<&'static str, ConnectionHandle>,
}

cpp! {{
    #include <memory>
    #include <QtCore/QPointer>
    #include <QtCore/QDebug>
    #include <QtCore/QObject>
    #include <QtWidgets/QWidget>
}}

cpp_class!(
    pub unsafe struct QWidgetPtr as "QPointer<QWidget>"
);

cpp_class!(
    pub unsafe struct QWidget as "QWidget *"
);

cpp_class!(
    pub unsafe struct QObjectPtr as "QPointer<QObject>"
);

cpp_class!(
    pub unsafe struct QObject as "QObject *"
);

impl Widget {
    pub fn new(ptr: QWidgetPtr) -> Widget {
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

    pub fn set_attribute(&self, name: *const c_char, value: QString) {
        let var: QVariant = value.into();
        cpp!(unsafe [self as "QPointer<QWidget> *", name as "const char*", var as "QVariant"] {
            qDebug() << "Setting property " << name << " to " << var << " on " << *self;
            (*self)->setProperty(name, var);
        })
    }

    pub fn get_attribute(&self, name: *const c_char) -> Option<QVariant> {
        let res = cpp!(unsafe [self as "QPointer<QWidget> *", name as "const char*"] -> QVariant as "QVariant" {
            QVariant res = (*self)->property(name);
            qDebug() << "Getting property " << name << " as " << res << " on " << *self;
            return res;
        });
        Some(res)
    }

    // pub fn exec(&self) {
    //     cpp!(unsafe [self as "QPointer<QWidget> *"] {
    //         qode::InjectCustomRunLoop((*self)->exec);
    //     })
    // }
}
