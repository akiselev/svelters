use std::borrow::BorrowMut;

use rquickjs::{Func, Function, Undefined};

use crate::inner::*;

pub fn initialize_element_node_proto<'js>(obj: &Object<'js>) -> Result<()> {
    obj.prop(
        "nextSibling",
        Accessor::new_get(rquickjs::Method(get_next_sibling)).enumerable(),
    )?;

    obj.prop(
        "prevSibling",
        Accessor::new_get(rquickjs::Method(get_prev_sibling)).enumerable(),
    )?;

    obj.prop(
        "parentNode",
        Accessor::new_get(rquickjs::Method(get_parent)).enumerable(),
    )?;

    obj.prop(
        "children",
        Accessor::new_get(rquickjs::Method(get_children)).enumerable(),
    )?;

    // obj.prop(
    //     "addEventListener",
    //     Func::new(
    //         "addEventListener",
    //         rquickjs::Method(add_event_listener::<CLS>),
    //     ),
    // )?;
    // obj.prop(
    //     "removeEventListener",
    //     Func::new(
    //         "removeEventListener",
    //         rquickjs::Method(remove_event_listener),
    //     ),
    // )?;
    obj.prop(
        "getAttribute",
        Func::new("getAttribute", rquickjs::Method(get_attribute)),
    )?;
    obj.prop(
        "setAttribute",
        Func::new("setAttribute", rquickjs::Method(set_attribute)),
    )?;
    Ok(())
}

fn get_next_sibling<'js>(this: ElementNode<'js>) -> Option<UINode<'js>> {
    this.borrow().node.next.clone()
}

fn get_prev_sibling<'js>(this: ElementNode<'js>) -> Option<UINode<'js>> {
    this.borrow().node.prev.clone()
}

fn get_parent<'js>(this: ElementNode<'js>) -> Option<UINode<'js>> {
    this.borrow().node.parent.clone()
}

fn get_children<'js>(this: ElementNode<'js>) -> Option<Vec<UINode<'js>>> {
    this.borrow().node.children.clone()
}

fn add_event_listener<'js>(
    this: Element<'js>,
    name: String,
    value: Function<'js>,
    _opt: Opt<Object<'js>>,
) -> Result<()> {
    // let node = this.node()?;
    // let rt = node.rt.metadata.deref().borrow();
    // let typedef = rt.types.get(node.typekey).unwrap();
    // let event = typedef.events.get(name.as_str()).ok_or_else(|| {
    //     exception!(
    //         "{} does not have an attribute named {}",
    //         node.tag_name,
    //         name
    //     )
    // })?;
    // (event.subscribe)(this.clone(), value)
    Ok(())
}

fn remove_event_listener<'js>(this: Element<'js>, name: String, func: Function<'js>) -> Result<()> {
    // let disconnect = {
    //     let dispatcher = this.dispatcher()?;
    //     let dispatcher = RefCell::borrow_mut(&dispatcher.inner);
    //     let sub = dispatcher
    //         .subs
    //         .get(name.as_str())
    //         .ok_or_else(|| Error::new(this.ctx(), "Dispathcher not found"))?;
    //     sub.remove_callback(this.ctx(), func)
    // }?;
    // if let Some(disconnect) = disconnect {
    //     tracing::info!("Disconnecting event {}", name.as_str());
    //     (disconnect)(this)?;
    // }
    Ok(())
}

fn get_attribute<'js>(this: ElementNode<'js>, ctx: Ctx<'js>, name: String) -> Result<Value<'js>> {
    let typedef = this.r#type;
    tracing::info!("Getting attr {}::{}", typedef.tag_name, name);
    let prop = typedef.members.get(name.as_str()).ok_or_else(|| {
        exception!(
            "{} does not have an attribute named {}",
            typedef.tag_name,
            name
        )
    })?;
    match prop {
        Member::Property(prop) => prop.get(ctx, &*this.inner.borrow().object),
        _ => Err(exception!(
            "{}::{} is not an attribute",
            typedef.tag_name,
            name
        )),
    }
}

fn set_attribute<'js>(
    this: ElementNode<'js>,
    ctx: Ctx<'js>,
    name: String,
    value: Value<'js>,
) -> Result<()> {
    let typedef = this.r#type;
    tracing::info!("Setting attr {}::{}", typedef.tag_name, name);
    let prop = typedef.members.get(name.as_str()).ok_or_else(|| {
        exception!(
            "{} does not have an attribute named {}",
            typedef.tag_name,
            name
        )
    })?;
    match prop {
        Member::Property(prop) => prop.set(ctx, &*this.inner.borrow().object, value),
        _ => Err(exception!(
            "{}::{} is not an attribute",
            typedef.tag_name,
            name
        )),
    }
}
