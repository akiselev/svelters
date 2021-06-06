#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::convert::TryInto;

use napi::{CallContext, Env, JsNumber, JsObject, JsUndefined, Property, Result};

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

mod main_window;

use main_window::{QApplication, QMainWindow};

#[js_function(1)]
fn qmainwindow_constructor(ctx: CallContext) -> Result<JsUndefined> {
    let mut this: JsObject = ctx.this_unchecked();
    ctx.env.wrap(&mut this, QMainWindow::new())?;

    ctx.env.get_undefined()
}

#[js_function(1)]
fn qmainwindow_show(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QMainWindow = ctx.env.unwrap(&this)?;
    node.show();
    ctx.env.get_undefined()
}

#[js_function(1)]
fn qapplication_constructor(ctx: CallContext) -> Result<JsUndefined> {
    let mut this: JsObject = ctx.this_unchecked();
    ctx.env.wrap(&mut this, QApplication::new())?;

    ctx.env.get_undefined()
}

#[js_function(1)]
fn qapplication_exec(ctx: CallContext) -> Result<JsUndefined> {
    let this: JsObject = ctx.this_unchecked();
    let node: &mut QApplication = ctx.env.unwrap(&this)?;
    node.exec();
    ctx.env.get_undefined()
}

#[module_exports]
pub fn init(mut exports: JsObject, env: Env) -> Result<()> {
    let qmainwindow = env.define_class(
        "QMainWindow",
        qmainwindow_constructor,
        &[Property::new(&env, "show")?.with_method(qmainwindow_show)],
    )?;
    let qapplication = env.define_class(
        "QApplication",
        qapplication_constructor,
        &[Property::new(&env, "exec")?.with_method(qapplication_exec)],
    )?;
    exports.set_named_property("QMainWindow", qmainwindow)?;
    exports.set_named_property("QApplication", qapplication)?;

    Ok(())
}
