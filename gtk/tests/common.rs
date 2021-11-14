use std::fs::read_to_string;

use rquickjs::{AsArguments, Context, Ctx, FromJs, Function, Module, Opt, Runtime, Value};

pub fn eval(src: &str) -> rquickjs::Result<()> {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        setup(ctx)?;

        ctx.eval::<(), _>(src)
    })
}

pub fn eval_result<T: for<'js> FromJs<'js>>(src: &str) -> rquickjs::Result<T> {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        setup(ctx)?;

        ctx.eval::<T, _>(src)
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
        func.call::<T, ()>(args)
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

fn setup<'js>(ctx: Ctx<'js>) -> rquickjs::Result<()> {
    gtk4::init().unwrap();
    let globals = ctx.globals();
    globals.init_def::<svelters_gtk::Types>()?;
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

fn assert<'js>(val: Value<'js>, msg: Opt<String>) -> rquickjs::Result<()> {
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
        if val.to_string().unwrap().len() == 0 {
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
