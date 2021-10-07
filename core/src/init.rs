use std::borrow::BorrowMut;

use crate::inner::*;

pub fn initialize_svelters(ctx: &rquickjs::Context) -> Result<Runtime> {
    let context = ctx.clone();
    ctx.with(move |ctx| {
        console::init(ctx)?;
        timing::init(ctx)?;

        let runtime = Runtime::new(context);
        rquickjs::Class::<Runtime>::register(ctx)?;
        rquickjs::Class::<WeakRuntime>::register(ctx)?;
        rquickjs::Class::<SlotNode>::register(ctx)?;
        rquickjs::Class::<TextNode>::register(ctx)?;
        rquickjs::Class::<ElementNode>::register(ctx)?;
        rquickjs::Class::<Event>::register(ctx)?;
        rquickjs::Class::<Dispatcher>::register(ctx)?;

        ctx.globals().set("document", runtime.clone())?;
        Result::<_>::Ok(runtime)
    })
}

#[tracing::instrument(skip(ctx))]
pub fn cleanup_svelters(ctx: &rquickjs::Context) -> Result<()> {
    ctx.with(move |ctx| {
        ctx.clear_registery();
        tracing::info!("Cleaning up Svelters...");
        let doc: &mut Runtime = ctx.globals().get("document")?;

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
