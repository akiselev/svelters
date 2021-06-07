#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use napi::{
    threadsafe_function::{ThreadSafeCallContext, ThreadsafeFunctionCallMode},
    CallContext, Env, JsFunction, JsNumber, JsObject, JsUndefined, Property, Result,
};
use qmetaobject::connections::connect;

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

#[js_function(1)]
fn qpushbutton_on_clicked(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut Widget = ctx.env.unwrap(&this)?;
    let func = ctx.get::<JsFunction>(0)?;
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
        &[Property::new(&env, "onClicked")?.with_method(qpushbutton_on_clicked)],
    )?;
    exports.set_named_property("QMainWindow", qmainwindow)?;
    exports.set_named_property("QApplication", qapplication)?;
    exports.set_named_property("QPushButton", qpushbutton)?;

    Ok(())
}
