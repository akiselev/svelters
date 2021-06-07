#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::{ffi::CString, os::raw::c_char};

use napi::{
    threadsafe_function::{ThreadSafeCallContext, ThreadsafeFunctionCallMode},
    CallContext, Env, JsFunction, JsNumber, JsObject, JsString, JsStringUtf8, JsUndefined,
    JsUnknown, Property, Result,
};
use qmetaobject::{connections::connect, QString};

#[cfg(all(
    unix,
    not(target_env = "musl"),
    not(target_arch = "aarch64"),
    not(target_arch = "arm"),
    not(debug_assertions)
))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(all(windows, target_arch = "x86_64"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod application;
mod button;
mod events;
mod logging;
mod main_window;
mod widget;

pub use widget::{QWidget, Widget};

use application::QApplication;
use main_window::QMainWindow;

#[js_function]
fn qmainwindow_constructor(ctx: CallContext) -> Result<JsUndefined> {
    let mut this: JsObject = ctx.this_unchecked();
    ctx.env.wrap(&mut this, QMainWindow::new())?;

    ctx.env.get_undefined()
}

#[js_function]
fn qmainwindow_show(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QMainWindow = ctx.env.unwrap(&this)?;
    node.show();
    ctx.env.get_undefined()
}

#[js_function(1)]
fn qmainwindow_set_central_widget(ctx: CallContext) -> Result<JsUndefined> {
    let widget = ctx.get::<JsObject>(0)?;
    let widget: &mut Widget = ctx.env.unwrap(&widget)?;
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QMainWindow = ctx.env.unwrap(&this)?;
    node.set_central_widget(widget);
    ctx.env.get_undefined()
}

#[js_function(1)]
fn qmainwindow_insert_before(ctx: CallContext) -> Result<JsUndefined> {
    let child = ctx.get::<JsObject>(0)?;
    let child: &mut Widget = ctx.env.unwrap(&child)?;
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QMainWindow = ctx.env.unwrap(&this)?;
    node.set_central_widget(child);
    ctx.env.get_undefined()
}

#[js_function]
fn qapplication_constructor(ctx: CallContext) -> Result<JsUndefined> {
    let mut this: JsObject = ctx.this_unchecked();
    ctx.env.wrap(&mut this, QApplication::new())?;

    ctx.env.get_undefined()
}

#[js_function]
fn qpushbutton_constructor(ctx: CallContext) -> Result<JsUndefined> {
    let mut this: JsObject = ctx.this_unchecked();
    ctx.env.wrap(&mut this, button::qpushbutton_new())?;

    ctx.env.get_undefined()
}

#[js_function(2)]
fn qpushbutton_set_attribute(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut Widget = ctx.env.unwrap(&this)?;
    let name_str = ctx.get::<JsString>(0)?.into_utf8()?;
    let name = name_str.as_slice();
    let prop: *const c_char = CString::new(name).unwrap().into_raw();
    let value = QString::from(ctx.get::<JsString>(1)?.into_utf8()?.as_str()?);
    node.set_attribute(prop, value);
    // let _ = CString::from_raw(prop);
    ctx.env.get_undefined()
}

#[js_function(2)]
fn qpushbutton_get_attribute(ctx: CallContext) -> Result<JsUnknown> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut Widget = ctx.env.unwrap(&this)?;
    let name_str = ctx.get::<JsString>(0)?.into_utf8()?;
    let name = name_str.as_slice();
    let prop: *const c_char = CString::new(name).unwrap().into_raw();
    node.get_attribute(prop);
    // let _ = CString::from_raw(prop);
    Ok(ctx.env.get_undefined()?.into_unknown())
}

#[js_function(2)]
fn qpushbutton_add_event_listener(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut Widget = ctx.env.unwrap(&this)?;
    let event = ctx.get::<JsString>(0)?;
    let func = ctx.get::<JsFunction>(1)?;
    let tsfn =
        ctx.env
            .create_threadsafe_function(&func, 0, |ctx: ThreadSafeCallContext<bool>| {
                let mut res = vec![];
                if let Ok(val) = ctx.env.get_boolean(ctx.value) {
                    res.push(val)
                }
                Ok(res)
            })?;
    let handle = unsafe {
        connect(
            node.as_sender(),
            button::qpushbutton_clicked(),
            move |result: &bool| {
                tsfn.call(Ok(*result), ThreadsafeFunctionCallMode::Blocking);
            },
        )
    };
    node.add_handle("clicked", handle);
    ctx.env.get_undefined()
}

#[js_function]
fn qapplication_exec(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QApplication = ctx.env.unwrap(&this)?;
    node.exec();
    ctx.env.get_undefined()
}

#[module_exports]
pub fn init(mut exports: JsObject, env: Env) -> Result<()> {
    logging::init();

    tracing::warn!(x = "asd", "Testing logging");
    log::warn!("an example trace log");
    println!("============");

    let qmainwindow = env.define_class(
        "QMainWindow",
        qmainwindow_constructor,
        &[
            Property::new(&env, "show")?.with_method(qmainwindow_show),
            Property::new(&env, "setCentralWidget")?.with_method(qmainwindow_set_central_widget),
            Property::new(&env, "insertBefore")?.with_method(qmainwindow_set_central_widget),
        ],
    )?;
    let qapplication = env.define_class(
        "QApplication",
        qapplication_constructor,
        &[Property::new(&env, "exec")?.with_method(qapplication_exec)],
    )?;
    let qpushbutton = env.define_class(
        "QPushButton",
        qpushbutton_constructor,
        &[
            Property::new(&env, "addEventListener")?.with_method(qpushbutton_add_event_listener),
            Property::new(&env, "getAttribute")?.with_method(qpushbutton_get_attribute),
            Property::new(&env, "setAttribute")?.with_method(qpushbutton_set_attribute),
        ],
    )?;
    exports.set_named_property("QMainWindow", qmainwindow)?;
    exports.set_named_property("QApplication", qapplication)?;
    exports.set_named_property("QPushButton", qpushbutton)?;

    Ok(())
}
