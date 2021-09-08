use std::borrow::BorrowMut;

use crate::inner::*;

pub fn initialize_svelters<'js>(ctx: Ctx<'js>) -> Result<()> {
    console::init(ctx)?;
    timing::init(ctx)?;

    let obj = Object::new(ctx)?;
    ctx.globals().set("window", obj)?;

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

pub fn cleanup_svelters<'js>(ctx: Ctx<'js>) -> Result<()> {
    ctx.clear_registery();
    let doc: &mut Document = ctx.globals().get("document")?;
    let mut rt = doc.rt.inner.deref().borrow_mut();
    doc.rt.objects.deref().borrow_mut().clear();
    rt.types.clear();
    // for (_key, _val) in doc.inner.borrow_mut().types.drain() {}
    Ok(())
}
