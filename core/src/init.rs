use std::borrow::BorrowMut;

use crate::inner::*;

pub fn initialize_gui_context(ctx: &rquickjs::Context) -> Result<()> {
    let context = ctx.clone();
    ctx.with(move |ctx| {
        console::init(ctx)?;
        timing::init(ctx)?;

        Result::<_>::Ok(())
    })
}

pub fn initialize_view_model_context(ctx: &rquickjs::Context) -> Result<ViewModelRuntime> {
    let context = ctx.clone();
    let runtime = ViewModelRuntime::new(context);
    let runtime = ctx.with(move |ctx| {
        console::init(ctx)?;
        timing::init(ctx)?;

        let globals = ctx.globals();
        rquickjs::Class::<ViewModelRuntime>::register(ctx)?;
        globals.set("document", runtime.clone())?;

        rquickjs::Class::<WeakVMRuntime>::register(ctx)?;
        rquickjs::Class::<SlotNode>::register(ctx)?;
        rquickjs::Class::<TextNode>::register(ctx)?;
        rquickjs::Class::<ElementNode>::register(ctx)?;
        rquickjs::Class::<Event>::register(ctx)?;
        rquickjs::Class::<Dispatcher>::register(ctx)?;
        Result::<_>::Ok(runtime)
    })?;
    runtime.with_types(|mut ty| {
        ty.register_element("root", Rc::new(root_element));
    });
    Ok(runtime)
}

#[tracing::instrument(skip(ctx))]
pub fn cleanup_view_model_context(ctx: &rquickjs::Context) -> Result<()> {
    ctx.with(move |ctx| {
        ctx.clear_registery();
        tracing::info!("Cleaning up Svelters...");
        let doc: &mut ViewModelRuntime = ctx.globals().get("document")?;

        ctx.globals().keys::<String>().for_each(|key| {
            ctx.globals().set(key.unwrap(), Value::new_undefined(ctx));
        });
        ctx.globals().set("document", Value::new_undefined(ctx))?;
        ctx.globals().set("window", Value::new_undefined(ctx))?;
        ctx.globals().set("setTimeout", Value::new_undefined(ctx))?;
        ctx.globals().set("console", Value::new_undefined(ctx))?;
        tracing::info!("Done cleaning up Svelters...");
        Ok(())
    })
}
