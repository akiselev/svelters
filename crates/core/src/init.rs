use crate::inner::*;

pub fn initialize_svelters<'js>(ctx: Ctx<'js>) -> Result<()> {
    let obj = Object::new(ctx)?;
    ctx.globals().set("window", obj)?;

    let doc = Document::new();
    rquickjs::Class::<Document>::register(ctx)?;
    rquickjs::Class::<Event<Value<'js>>>::register(ctx)?;
    rquickjs::Class::<Slot>::register(ctx)?;

    ctx.globals().set("document", doc)?;
    Ok(())
}
