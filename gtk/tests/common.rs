#![allow(dead_code)]
use std::fs::read_to_string;

use rquickjs::{AsArguments, Context, Ctx, FromJs, Function, Opt, Runtime, Value};

pub fn eval(src: &str) -> rquickjs::Result<()> {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        setup(ctx)?;

        let res = ctx.eval::<(), _>(src);
        cleanup(ctx)?;
        res
    })
}

pub fn eval_result<T: for<'js> FromJs<'js>>(src: &str) -> rquickjs::Result<T> {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        setup(ctx)?;

        let res = ctx.eval::<T, _>(src);
        cleanup(ctx)?;
        res
    })
}

pub fn module<T>(path: &str, func_name: &str, args: T) -> rquickjs::Result<()>
where
    T: for<'js> AsArguments<'js>,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(move |ctx| {
        setup(ctx)?;

        let src = read_to_string(path).unwrap();
        let module = ctx.compile(path, src).unwrap();
        let func = module.get::<_, Function>(func_name).unwrap();
        let res = func.call::<T, ()>(args);
        cleanup(ctx)?;
        res
    })
}

macro_rules! exception {
    ($($tokens:tt)*) => {
        {
            use std::convert::TryInto;
            rquickjs::Error::Exception {
                message: format!(
                    $($tokens)*
                ),
                file: file!().to_string(),
                line: line!().try_into().unwrap(),
                stack: "".to_string(),
                rust_stack: None,
            }
        }
    };
}

fn setup(ctx: Ctx) -> rquickjs::Result<()> {
    gtk4::init().unwrap();
    svelters_gtk::initialize_types(ctx)?;
    let globals = ctx.globals();
    globals.set("document", rquickjs::Object::new(ctx)?)?;
    globals.set("application", rquickjs::Object::new(ctx)?)?;
    globals.set("assert_eq", rquickjs::Function::new(ctx, assert_eq)?)?;
    globals.set("assert", rquickjs::Function::new(ctx, assert)?)?;
    Ok(())
}

fn assert_eq<'js>(left: Value<'js>, right: Value<'js>, msg: Opt<String>) -> rquickjs::Result<()> {
    if left != right {
        if let Some(msg) = msg.into_inner() {
            return Err(exception!("AssertionError: {}", msg));
        }
        return Err(exception!("AssertionError"));
    }
    Ok(())
}

fn assert(val: Value, msg: Opt<String>) -> rquickjs::Result<()> {
    let mut failed = false;
    if val.is_null() || val.is_undefined() {
        failed = true;
    }
    if let Some(val) = val.as_bool() {
        if !val {
            failed = true;
        }
    }
    if let Some(val) = val.as_string() {
        if val.to_string().unwrap().is_empty() {
            failed = true;
        }
    }
    if let Some(val) = val.as_int() {
        if val <= 0 {
            failed = true;
        }
    }
    if failed {
        if let Some(msg) = msg.into_inner() {
            return Err(exception!("AssertionError: {}", msg));
        }
        return Err(exception!("AssertionError"));
    }
    Ok(())
}

pub fn cleanup(ctx: Ctx) -> rquickjs::Result<()> {
    ctx.clear_registery();
    tracing::info!("Cleaning up Svelters...");
    ctx.globals().keys::<String>().for_each(|key| {
        ctx.globals()
            .set(key.unwrap(), Value::new_undefined(ctx))
            .ok();
    });
    ctx.globals().set("document", Value::new_undefined(ctx))?;
    ctx.globals().set("window", Value::new_undefined(ctx))?;
    ctx.globals().set("setTimeout", Value::new_undefined(ctx))?;
    ctx.globals().set("console", Value::new_undefined(ctx))?;
    tracing::info!("Done cleaning up Svelters...");
    rquickjs::Result::Ok(())
}
