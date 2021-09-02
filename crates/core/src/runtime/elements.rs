use crate::inner::*;

pub(in crate) fn create_element_ns<'js>(
    this: &mut Document,
    ctx: Ctx<'js>,
    _ns: Value<'js>,
    name: String,
    options: Opt<Object<'js>>,
) -> Result<Value<'js>> {
    create_element(this, ctx, name, options)
}

pub(in crate) fn create_element<'js>(
    this: &mut Document,
    ctx: Ctx<'js>,
    name: String,
    _options: Opt<Object<'js>>,
) -> Result<Value<'js>> {
    if let Some(element) = this.elements.borrow().get(name.as_str()) {
        (element.constructor)(ctx)
    } else {
        Err(Error::new(ctx, format!("{} is not a valid element", name)))
    }
}

pub(in crate) fn dispatch_event<'js>(
    this: &mut Document,
    ctx: Ctx<'js>,
    event: Object<'js>,
    _options: Opt<Object<'js>>,
) -> Result<()> {
    Ok(())
}
