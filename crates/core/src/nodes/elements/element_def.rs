use rquickjs::HasRefs;

use crate::inner::*;

impl<'a, 'js> rquickjs::FromJs<'js> for Element<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let type_of = value.type_of();
        let node = Object::from_js(ctx, value)?;
        if node.instance_of::<ElementNode>() {
            Ok(Element {
                _ctx: ctx,
                el_node: node,
                doc_cell: OnceCell::new(),
                node_cell: OnceCell::new(),
            })
        } else {
            Err(Error::new(
                ctx,
                format!("{} is not an Element node", type_of),
            ))
        }
    }
}
