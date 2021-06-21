use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use lazy_static::lazy_static;
use qmetaobject::connections::ConnectionHandle;
use thiserror::Error as ThisError;

use svelters::class::{ClassDef, ClassID};

cpp! {{
    #include <memory>
    #include <QtCore/QObject>
    #include <QtCore/QPointer>
    #include <QtWidgets/QApplication>

}}

cpp_class!(
    pub unsafe struct QPtr as "QPointer<QObject>"
);

cpp_class!(
    pub unsafe struct QRawPtr as "QObject*"
);

#[repr(u8)]
enum ObjectKind {
    Element,
}

#[repr(C)]
pub struct QObject {
    ptr: QPtr,
    kind: ObjectKind,
}

impl ClassDef for QObject {
    fn class_id() -> ClassID {
        lazy_static! {
            static ref CLASS_ID: ClassID = ClassID::new();
        }
        *CLASS_ID
    }

    unsafe fn class_init() -> libquickjs_sys::JSValue {
        todo!()
    }
}
