use rquickjs::HasRefs;
use smallvec::SmallVec;

use crate::inner::*;

pub type Idx = RegisteryKey;

use crate::inner::*;

#[derive(Clone)]
pub struct Node<'js> {
    ctx: Ctx<'js>,
    this: NodeType<'js>,
}

#[derive(Clone)]
enum NodeType<'js> {
    Slot(Slot<'js>),
    Text(TextNode<'js>),
    Element(Element<'js>),
}




#[derive(Clone)]
pub(in crate) struct NodeData<'js> {
    pub(in crate) next: Option<Node<'js>>,
    pub(in crate) prev: Option<Node<'js>>,
    pub(in crate) parent: Option<Node<'js>>,
    pub(in crate) children: Option<Vec<Node<'js>>>,
}

impl<'js> NodeData<'js> {
    pub(in crate) fn new() -> Self {
        Self {
            next: None,
            prev: None,
            parent: None,
            children: None,
        }
    }
}

impl<'js> HasRefs for Node<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        match &self.this {
            NodeType::Slot(node) => node.mark_refs(marker),
            NodeType::Text(node) => node.mark_refs(marker),
            NodeType::Element(node) => <Element<'js> as HasRefs>::mark_refs(&node, marker),
        }
    }
}


impl<'js> HasRefs for NodeData<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        if let Some(next) = &self.next { next.mark_refs(marker)}
        if let Some(prev) = &self.prev { prev.mark_refs(marker)}
        if let Some(parent) = &self.parent { parent.mark_refs(marker)}
        if let Some(children) = &self.children { children.iter().for_each(|child| child.mark_refs(marker)) }
    }
}

impl<'js> rquickjs::IntoJs<'js> for Node<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        Ok(match self.this {
            NodeType::Slot(node) => node.into_js(ctx)?,
            NodeType::Text(node) => node.into_js(ctx)?,
            NodeType::Element(node) => node.into_js(ctx)?,
        })
    }
}

impl<'js, 'a> rquickjs::FromJs<'js> for Node<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let this = if let Some(obj) = value.as_object() {
            match obj {
                obj if obj.instance_of::<Element<'js>>() => NodeType::Element(Element::from_js_obj(ctx, value)?),
                obj if obj.instance_of::<Slot<'js>>() => NodeType::Slot(Slot::from_js_obj(ctx, value)?),
                obj if obj.instance_of::<TextNode<'js>>() => NodeType::Text(TextNode::from_js_obj(ctx, value)?),
                _ => return Err(Error::new_from_js("object", "Node"))
            }
        } else { return Err(Error::new_from_js(value.type_name(), "Node")) };
        Ok(Node {
            ctx,
            this
        })
    }
}
