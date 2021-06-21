use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use anyhow::{anyhow, Result};
use cpp::{cpp, cpp_class};
use libquickjs_sys::{JSContext, JSValue};
use phf::phf_map;
use qttypes::*;

use crate::{defs::*, QObject};

cpp! {{
    #include <memory>
    #include <QtCore/QObject>
    #include <QtCore/QString>
    #include <QtCore/QPointer>
    #include <QtWidgets/QApplication>
    #include <QtWidgets/QMainWindow>
    #include <QtWidgets/QPushButton>
}}

static QOBJECT: Class = Class {
    name: "qObject",
    superclasses: &[],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {},
    constructor: None,
};

static QAPPLICATION: Class = Class {
    name: "qApplication",
    superclasses: &[&QOBJECT],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {
        // "exec" => Method {
        //     name: "exec",
        //     func: &qapplication_exec as &MethodFn
        // },
    },
    constructor: None,
    // constructor: Some(&qapplication_new as &Constructor),
};

static QWIDGET: Class = Class {
    name: "qWidget",
    superclasses: &[&QOBJECT],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {},
    constructor: None,
};

static QMAINWINDOW: Class = Class {
    name: "qMainWindow",
    superclasses: &[&QWIDGET],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {
        // "setCentralWidget" => Method {
        //     name: "setCentralWidget",
        //     func: &qmainwindow_set_central_widget as &MethodFn
        // },
        // "show" => Method {
        //     name: "show",
        //     func: &qmainwindow_show as &MethodFn
        // },
        // "hide" => Method {
        //     name: "hide",
        //     func: &qmainwindow_hide as &MethodFn
        // }
    },
    constructor: None,
    // constructor: Some(&qmainwindow_new as &Constructor),
};

static QPUSHBUTTON: Class = Class {
    name: "qPushButton",
    superclasses: &[&QWIDGET],
    props: phf_map! {
        // "text" => Prop {
        //     name: "text",
        //     getter: &qpushbutton_get_text as &GetterFn,
        //     setter: Some(&qpushbutton_set_text as &SetterFn),
        // }
    },
    events: phf_map! {},
    methods: phf_map! {
        // "exec" => Method {
        //     name: "exec",
        //     func: &qpushbutton_exec as &MethodFn
        // }
    },
    constructor: None,
    // constructor: Some(&qpushbutton_new as &Constructor),
};

pub(crate) static ELEMENTS: ClassMap = phf_map! {
    "qObject" => &QOBJECT,
    "qApplication" => &QAPPLICATION,
    "qMainWindow" => &QMAINWINDOW,
    "qPushButton" => &QPUSHBUTTON,
};

// pub fn qpushbutton_get_text<'a>(this: &'a QObject, ctx: *mut JSContext) -> Result<JSValue> {
//     this.check_tag(QPUSHBUTTON.name)?;
//     let ptr = &*this.ptr;
//     let res = cpp!(unsafe [ptr as "QPointer<QPushButton> *"] -> QString as "QString" {
//         // if (!ptr->isNull()) {
//             return (*ptr)->text();
//         // }
//     });
//     let val: String = res.into();

//     Ok(val.into_js(ctx)?)
// }

// pub fn qpushbutton_set_text<'a>(
//     this: &mut QObject,
//     ctx: *mut JSContext,
//     val: JSValue,
// ) -> Result<()> {
//     this.check_tag(QPUSHBUTTON.name)?;
//     let ptr = &*this.ptr;
//     let val_str = val
//         .as_string()
//         .ok_or_else(|| anyhow!("QPushButton property 'text' is a string"))?
//         .to_string()?;
//     let val: QString = val_str.into();
//     cpp!(unsafe [ptr as "QPointer<QPushButton> *", val as "QString"] {
//         // if (!ptr->isNull()) {
//             (*ptr)->setText(val);
//         // }
//     });
//     Ok(())
// }

// pub fn qmainwindow_set_central_widget<'a>(
//     this: &mut QObject,
//     ctx: *mut JSContext,
//     args: JSValue,
// ) -> Result<JSValue> {
//     this.check_tag(QMAINWINDOW.name)?;
//     let mut args = args.into_inner();
//     if !(args.len() >= 1) {
//         return Err(anyhow!("Expected at least one argument"));
//     }
//     let target: &QObject = FromJs::from_js(
//         ctx,
//         args.get(0)
//             .cloned()
//             .ok_or_else(|| anyhow!("First argument must implement QWidget"))?,
//     )?;
//     target.check_tags(QWIDGET.name)?;
//     let target = &*target.ptr;
//     let ptr = &*this.ptr;
//     cpp!(unsafe [ptr as "QPointer<QMainWindow> *", target as "QPointer<QWidget> *"] {
//         if (!ptr->isNull() && !target->isNull()) {
//             (*ptr)->setCentralWidget(*target);
//         }
//     });
//     Ok(Value::new_undefined(ctx))
// }

// pub fn qmainwindow_show<'a>(
//     this: &mut QObject,
//     ctx: *mut JSContext,
//     args: JSValue,
// ) -> Result<JSValue> {
//     this.check_tag(QMAINWINDOW.name)?;
//     let ptr = &*this.ptr;
//     cpp!(unsafe [ptr as "QPointer<QMainWindow> *"] {
//         (*ptr)->show();
//     });
//     Ok(Value::new_undefined(ctx))
// }

// pub fn qmainwindow_hide<'a>(
//     this: &mut QObject,
//     ctx: *mut JSContext,
//     args: JSValue,
// ) -> Result<JSValue> {
//     this.check_tag(QMAINWINDOW.name)?;
//     let ptr = &*this.ptr;
//     cpp!(unsafe [ptr as "QPointer<QMainWindow> *"] {
//         (*ptr)->hide();
//     });
//     Ok(Value::new_undefined(ctx))
// }

// pub fn qapplication_exec<'a>(
//     this: &mut QObject,
//     ctx: *mut JSContext,
//     args: JSValue,
// ) -> Result<JSValue> {
//     this.check_tag(QAPPLICATION.name)?;
//     let ptr = &*this.ptr;
//     cpp!(unsafe [ptr as "QPointer<QApplication> *"] {
//         (*ptr)->exec();
//     });
//     Ok(Value::new_undefined(ctx))
// }

// fn qapplication_new<'a>(ctx: *mut JSContext, args: JSValue) -> Result<QObject> {
//     let mut arguments: Vec<*mut c_char> = std::env::args()
//         .map(|arg| CString::new(arg.into_bytes()).expect("argument contains invalid c-string!"))
//         .map(|arg| arg.into_raw())
//         .collect();
//     let argc: i32 = arguments.len() as i32 - 1;
//     let argv: *mut *mut c_char = arguments.as_mut_ptr();
//     let ptr = cpp!(unsafe [
//         argc as "int",
//         argv as "char **"
//     ] -> Pin<Box<QPtr>> as "QPointer<QObject>*" {
//         // Static variables when used inside function are initialized only once
//         static int _argc  = argc;
//         static char **_argv = nullptr;
//         // this is *real* initialization, and it would also happen only once
//         if (_argv == nullptr) {
//             // copy the arguments
//             _argv = new char *[argc + 1];
//             // argv should be null terminated
//             _argv[argc] = nullptr;
//             for (int i = 0; i < argc; ++i) {
//                 _argv[i] = new char[strlen(argv[i]) + 1];
//                 strcpy(_argv[i], argv[i]);
//             }
//         }
//         QPointer<QObject>* ptr = new QPointer<QObject>();
//         *ptr = new QApplication(_argc, _argv);
//         return ptr;
//     });

//     // run destructor
//     for arg in arguments {
//         let _ = unsafe { CString::from_raw(arg) };
//     }

//     Ok(QObject::new(QAPPLICATION.name, ptr))
// }

// fn qmainwindow_new<'a>(ctx: *mut JSContext, args: JSValue) -> Result<QObject> {
//     let ptr = cpp!(unsafe [] -> Pin<Box<QPtr>> as "QPointer<QObject>*" {
//         QPointer<QObject>* ptr = new QPointer<QObject>();
//         *ptr = new QMainWindow();
//         return ptr;
//     });

//     Ok(QObject::new(QMAINWINDOW.name, ptr))
// }

// fn qpushbutton_new<'a>(ctx: *mut JSContext, args: JSValue) -> Result<QObject> {
//     let ptr = cpp!(unsafe [] -> Pin<Box<QPtr>> as "QPointer<QObject>*" {
//         QPointer<QObject>* ptr = new QPointer<QObject>();
//         *ptr = new QPushButton();
//         return ptr;
//     });

//     Ok(QObject::new(QPUSHBUTTON.name, ptr))
// }
