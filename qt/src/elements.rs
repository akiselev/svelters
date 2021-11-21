use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use phf::phf_map;
use qttypes::*;
use rquickjs::{Error, FromJs, IntoJs, Opt, Result, Value};
use svelters_core::FrameworkSchema;

mod impls;
use crate::*;
use impls::*;

static QOBJECT: Class = Class {
    name: "qObject",
    superclasses: &[],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {},
    constructor: None,
    insert: None,
};

static QWIDGET: Class = Class {
    name: "qWidget",
    superclasses: &[&QOBJECT],
    props: phf_map! {},
    events: phf_map! {},
    methods: phf_map! {},
    constructor: None,
    insert: None,
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
        "show" => Method {
            name: "show",
            func: &qmainwindow_show as &MethodFn
        },
        "hide" => Method {
            name: "hide",
            func: &qmainwindow_hide as &MethodFn
        }
    },
    insert: Some(&qmainwindow_set_central_widget as &InsertFn),
    constructor: Some(&qmainwindow_new as &Constructor),
};

static QPUSHBUTTON: Class = Class {
    name: "qPushButton",
    superclasses: &[&QWIDGET],
    props: phf_map! {
        "text" => Prop {
            name: "text",
            getter: &qpushbutton_get_text as &GetterFn,
            setter: Some(&qpushbutton_set_text as &SetterFn),
        }
    },
    events: phf_map! {
        "clicked" => Event {
            name: "clicked",
            connect: &qpushbutton_on_clicked as &ConnectFn
        }
    },
    methods: phf_map! {},
    insert: None,
    constructor: Some(&qpushbutton_new as &Constructor),
};

pub(crate) static ELEMENTS: ClassMap = phf_map! {
    "qObject" => &QOBJECT,
    "qMainWindow" => &QMAINWINDOW,
    "qPushButton" => &QPUSHBUTTON,
};

pub(crate) static FRAMEWORK: FrameworkSchema<Qt> = FrameworkSchema { classes: &ELEMENTS };
