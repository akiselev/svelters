use std::borrow::BorrowMut;

use crate::inner::*;

pub fn initialize_svelters<'js>(ctx: Ctx<'js>) -> Result<()> {
    console::init(ctx)?;
    timing::init(ctx)?;

    let doc = Document::new();
    rquickjs::Class::<Document>::register(ctx)?;
    rquickjs::Class::<Event<'js>>::register(ctx)?;
    rquickjs::Class::<SlotNode>::register(ctx)?;
    rquickjs::Class::<TextNode>::register(ctx)?;
    rquickjs::Class::<ElementNode>::register(ctx)?;
    rquickjs::Class::<Dispatcher>::register(ctx)?;

    ctx.globals().set("document", doc)?;
    Ok(())
}

#[tracing::instrument(skip(ctx))]
pub fn cleanup_svelters<'js>(ctx: Ctx<'js>) -> Result<()> {
    ctx.clear_registery();
    tracing::info!("Cleaning up Svelters...");
    let doc: &mut Document = ctx.globals().get("document")?;
    let mut rt = doc.rt.inner.deref().borrow_mut();
    doc.rt.objects.deref().borrow_mut().clear();
    rt.types.clear();
    ctx.globals().keys::<String>().for_each(|key| {
        ctx.globals().set(key.unwrap(), Value::new_undefined(ctx));
    });
    ctx.globals().set("document", Value::new_undefined(ctx))?;
    ctx.globals().set("window", Value::new_undefined(ctx))?;
    ctx.globals().set("setTimeout", Value::new_undefined(ctx))?;
    ctx.globals().set("console", Value::new_undefined(ctx))?;
    tracing::info!("Done cleaning up Svelters...");
    // for (_key, _val) in doc.inner.borrow_mut().types.drain() {}
    Ok(())
}
