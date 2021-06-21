use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use phf::phf_map;
use qmetaobject::{connect, Signal, SignalInner};
use qttypes::*;
use rquickjs::{Context, Error, FromJs, IntoJs, Opt, RegisteryKey, Result, Value};
use svelters_core::FrameworkSchema;

use super::*;

cpp! {{
    #include <memory>
    #include <QtCore/QObject>
    #include <QtCore/QString>
    #include <QtCore/QPointer>
    #include <QtWidgets/QApplication>
    #include <QtWidgets/QMainWindow>
    #include <QtWidgets/QPushButton>
    #include "qmetaobject_rust.hpp"
}}

pub fn qpushbutton_get_text<'js>(ctx: Ctx<'js>, this: &'js QObject) -> Result<Value<'js>> {
    this.check_tag(QPUSHBUTTON.name)?;
    let ptr = &*this.ptr;
    let res = cpp!(unsafe [ptr as "QPointer<QPushButton> *"] -> QString as "QString" {
        // if (!ptr->isNull()) {
            return (*ptr)->text();
        // }
    });
    let val: String = res.into();

    Ok(val.into_js(ctx)?)
}

pub fn qpushbutton_set_text<'js>(
    ctx: Ctx<'js>,
    this: &'js mut QObject,
    val: Value<'js>,
) -> Result<()> {
    this.check_tag(QPUSHBUTTON.name)?;
    let ptr = &*this.ptr;
    let val_str = val
        .as_string()
        .ok_or_else(|| Error::Exception {
            message: "qPushButton prop text only supports strings".to_string(),
            file: "".to_string(),
            line: -1,
            stack: "".to_string(),
        })?
        .to_string()?;
    let val: QString = val_str.into();
    cpp!(unsafe [ptr as "QPointer<QPushButton> *", val as "QString"] {
        // if (!ptr->isNull()) {
            (*ptr)->setText(val);
        // }
    });
    Ok(())
}

pub fn qmainwindow_set_central_widget<'js>(
    ctx: Ctx<'js>,
    this: &'js mut QObject,
    target: &'js mut QObject,
    anchor: Opt<&'js mut QObject>,
) -> Result<bool> {
    this.check_tag(QMAINWINDOW.name)?;
    target.check_tags(QWIDGET.name)?;
    let target = &*target.ptr;
    let ptr = &*this.ptr;
    cpp!(unsafe [ptr as "QPointer<QMainWindow> *", target as "QPointer<QWidget> *"] {
        if (!ptr->isNull() && !target->isNull()) {
            (*ptr)->setCentralWidget(*target);
        }
    });
    Ok(true)
}

pub fn qmainwindow_show<'js>(
    ctx: Ctx<'js>,
    this: &'js mut QObject,
    args: Vec<Value<'js>>,
) -> Result<Value<'js>> {
    this.check_tag(QMAINWINDOW.name)?;
    let ptr = &*this.ptr;
    cpp!(unsafe [ptr as "QPointer<QMainWindow> *"] {
        (*ptr)->show();
    });
    Ok(Value::new_undefined(ctx))
}

pub fn qmainwindow_hide<'js>(
    ctx: Ctx<'js>,
    this: &'js mut QObject,
    args: Vec<Value<'js>>,
) -> Result<Value<'js>> {
    this.check_tag(QMAINWINDOW.name)?;
    let ptr = &*this.ptr;
    cpp!(unsafe [ptr as "QPointer<QMainWindow> *"] {
        (*ptr)->hide();
    });
    Ok(Value::new_undefined(ctx))
}

pub fn qmainwindow_new<'js>(ctx: Ctx<'js>, args: Vec<Value<'js>>) -> Result<QObject> {
    let ptr = cpp!(unsafe [] -> Pin<Box<QPtr>> as "QPointer<QObject>*" {
        QPointer<QObject>* ptr = new QPointer<QObject>();
        *ptr = new QMainWindow();
        return ptr;
    });

    Ok(QObject::new(QMAINWINDOW.name, ptr))
}

pub fn qpushbutton_new<'js>(ctx: Ctx<'js>, args: Vec<Value<'js>>) -> Result<QObject> {
    let ptr = cpp!(unsafe [] -> Pin<Box<QPtr>> as "QPointer<QObject>*" {
        QPointer<QObject>* ptr = new QPointer<QObject>();
        *ptr = new QPushButton();
        return ptr;
    });

    Ok(QObject::new(QPUSHBUTTON.name, ptr))
}

pub fn qpushbutton_on_clicked<'a, 'b>(
    ctx: Context,
    this: &'b mut QObject,
    func: RegisteryKey,
) -> Result<EventHandle> {
    this.check_tag(QPUSHBUTTON.name)?;
    let signal: Signal<fn(bool)> = unsafe {
        Signal::new(cpp!([] -> SignalInner as "SignalInner"  {
            return &QAbstractButton::clicked;
        }))
    };
    tracing::info!("`clicked` event handler added");
    let key = func.clone();
    let callback = move |val: &bool| {
        let val = *val;
        ctx.with(|ctx| {
            tracing::info!("Calling `clicked` event handler");
            let func = ctx.get_register(key).unwrap();
            let func = func.as_function().unwrap();
            func.call::<_, ()>((val,)).unwrap();
        })
    };
    let conn = unsafe { connect(this.raw_ptr(), signal, callback) };
    Ok(EventHandle(conn))
}
