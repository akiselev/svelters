use rquickjs::{Func, Function};

use crate::inner::*;

impl<'a, 'js, 'c, CLS, T> Implements<InitElementProto<'a, 'js, 'c, CLS>>
    for members::Element<CLS, T>
where
    CLS: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut InitElementProto<'a, 'js, 'c, CLS>) -> Result<()> {
        phase.obj.prop(
            "nextSibling",
            Accessor::new_get(rquickjs::Method(get_next_sibling)).enumerable(),
        )?;

        phase.obj.prop(
            "prevSibling",
            Accessor::new_get(rquickjs::Method(get_prev_sibling)).enumerable(),
        )?;

        phase.obj.prop(
            "parentNode",
            Accessor::new_get(rquickjs::Method(get_parent)).enumerable(),
        )?;

        phase.obj.prop(
            "children",
            Accessor::new_get(rquickjs::Method(get_children)).enumerable(),
        )?;

        phase.obj.prop(
            "addEventListener",
            Func::new(
                "addEventListener",
                rquickjs::Method(add_event_listener::<CLS>),
            ),
        )?;
        phase.obj.prop(
            "removeEventListener",
            Func::new(
                "removeEventListener",
                rquickjs::Method(remove_event_listener),
            ),
        )?;
        phase.obj.prop(
            "getAttribute",
            Func::new("getAttribute", rquickjs::Method(get_attribute)),
        )?;
        phase.obj.prop(
            "setAttribute",
            Func::new("setAttribute", rquickjs::Method(set_attribute)),
        )?;

        Ok(())
    }
}

#[inline(always)]
fn get_next_sibling<'js>(this: ElementNode<'js>) -> Option<Node<'js>> {
    this.borrow().node.next.clone()
}

#[inline(always)]
fn get_prev_sibling<'js>(this: ElementNode<'js>) -> Option<Node<'js>> {
    this.borrow().node.prev.clone()
}

#[inline(always)]
fn get_parent<'js>(this: ElementNode<'js>) -> Option<Node<'js>> {
    this.borrow().node.parent.clone()
}

#[inline(always)]
fn get_children<'js>(this: ElementNode<'js>) -> Option<Vec<Node<'js>>> {
    this.borrow().node.children.clone()
}

#[inline(always)]
fn add_event_listener<'js, CLS: Type>(
    this: ElementNode<'js>,
    ctx: Ctx<'js>,
    name: String,
    value: Function<'js>,
    _opt: Opt<Object<'js>>,
) -> Result<()> {
    let doc: &mut Document = ctx.globals().get("document")?;
    let inner = doc.inner.borrow();
    let tag = { this.tag_name };
    let _type = inner
        .types
        .get(tag)
        .ok_or_else(|| Error::new(ctx, format!("Could not find type {}", CLS::CLASS_NAME)))?;
    let subscribe = _type.events.get(name.as_str()).ok_or_else(|| {
        Error::new(
            ctx,
            format!("Could not find event {} for {}", name, CLS::CLASS_NAME),
        )
    })?;
    (subscribe.subscribe)(this, ctx, value)
}

#[inline(always)]
fn remove_event_listener<'js>(
    this: ElementNode<'js>,
    _name: String,
    _value: Function<'js>,
) -> Option<Vec<Node<'js>>> {
    this.borrow().node.children.clone()
}

#[inline(always)]
fn get_attribute<'js>(this: ElementNode<'js>, name: String) -> Result<Value<'js>> {
    this.borrow().obj.get(name)
}

#[inline(always)]
fn set_attribute<'js>(this: ElementNode<'js>, name: String, value: Value<'js>) -> Result<()> {
    this.borrow().obj.set(name, value)
}
