use rquickjs::{Function, HasRefs};
use smallvec::SmallVec;

use crate::inner::*;

pub type Idx = RegisteryKey;

use crate::inner::*;

#[derive(Clone)]
pub struct Node<'js> {
    pub(in crate) _ctx: Ctx<'js>,
    pub(in crate) this: NodeType<'js>,
}

impl<'js> PartialEq for Node<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.this == other.this
    }
}

impl<'js> PartialEq<Element<'js>> for Node<'js> {
    fn eq(&self, other: &Element<'js>) -> bool {
        match &self.this {
            NodeType::Element(this) => this.borrow().obj == other.borrow().obj,
            _ => false,
        }
    }
}

impl<'js, CLS> PartialEq<Virt<'js, CLS>> for Node<'js> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        match &self.this {
            NodeType::Element(this) => this.borrow().obj == other.obj,
            _ => false,
        }
    }
}

impl<'js> PartialEq<Object<'js>> for Node<'js> {
    fn eq(&self, other: &Object<'js>) -> bool {
        match &self.this {
            NodeType::Element(this) => this.borrow().obj == *other,
            _ => false,
        }
    }
}

impl<'js> Eq for Node<'js> {}

impl<'js> Node<'js> {
    pub fn append_node(&self, node: Node<'js>) {
        let parent = self.clone();
        self.with_node(move |this| {
            if this.is_mounted() {
                panic!("Tried to append node that is already mounted")
            }
            let children = this.children.get_or_insert_with(|| vec![]);
            let prev = if let Some(child) = children.last_mut() {
                child.with_node(|data| {
                    if data.is_mounted() {
                        panic!("Tried to append node that is already mounted")
                    }
                    if data.next.is_none() {
                        panic!("Next sibling on append node is already set")
                    }
                    data.next = Some(node.clone());
                });
                Some(child.clone())
            } else {
                None
            };
            let prev = prev.and_then(|prev| {
                prev.with_node(|data| data.next = Some(node.clone()));
                Some(prev)
            });
            node.with_node(move |data| {
                data.parent = Some(parent);
                data.prev = prev;
            })
        })
    }

    pub fn remove_node(&self, node: Node<'js>) {
        self.with_node(move |this| {
            if !this.is_mounted() {
                panic!("Tried to remove node that is not mounted")
            }
            let children = this.children.get_or_insert_with(|| vec![]);
            let index = {
                children
                    .iter_mut()
                    .position(|this| *this == node)
                    .expect("Can't find node to remove")
            };
            node.with_node(|data| {
                let mut next = data.next.take();
                let mut prev = data.prev.take();
                if let Some(prev) = &mut prev {
                    let next = next.clone();
                    prev.with_node(move |data| data.next = next)
                }
                if let Some(next) = &mut next {
                    next.with_node(|data| data.prev = prev)
                }
                let _ = data.parent.take();
            });
            children.remove(index);
        })
    }

    pub fn insert_node(&self, node: Node<'js>, anchor: Node<'js>) {
        let parent = self.clone();
        self.with_node(move |this| {
            if this.is_mounted() {
                panic!("Tried to insert node that is already mounted")
            }
            let children = this.children.get_or_insert_with(|| vec![]);
            let index = {
                children
                    .iter_mut()
                    .position(|this| *this == anchor)
                    .expect("Can't find anchor for insert")
            };
            let target = children[index].clone();
            let prev = target.with_node(|data| {
                let prev = data.prev.take();
                data.prev = Some(node.clone());
                prev
            });
            if let Some(prev) = &prev {
                prev.with_node(|data| data.next = Some(node.clone()));
            }
            node.with_node(move |data| {
                data.parent = Some(parent);
                data.prev = prev;
                data.next = Some(target);
            });
            children.insert(index, node);
        })
    }

    pub fn index_of(&self, node: Node<'js>) -> Option<usize> {
        self.with_node(move |this| {
            if let Some(children) = &this.children {
                children.iter().position(|this| *this == node)
            } else {
                None
            }
        })
    }

    fn instance_of<CLS: Type>(&self) -> bool {
        match &self.this {
            NodeType::Element(this) => this.borrow().obj.instance_of::<Obj<CLS>>(),
            _ => false,
        }
    }

    fn with_instance<CLS: Type, F: for<'a> FnOnce(&'a CLS) -> R, R>(&self, func: F) -> Option<R> {
        match &self.this {
            NodeType::Element(this) => {
                let this = this.borrow();
                if this.obj.instance_of::<Obj<CLS>>() {
                    this.obj
                        .clone()
                        .into_instance::<Obj<CLS>>()
                        .and_then(|obj| Some(func(&AsRef::<Obj<CLS>>::as_ref(&obj).inner)))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn with_node<F: FnOnce(&mut NodeData<'js>) -> R, R>(&self, func: F) -> R {
        match &self.this {
            NodeType::Slot(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
            NodeType::Text(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
            NodeType::Element(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
        }
    }

    pub fn with_dispatcher<F: FnOnce(&mut Dispatcher) -> R, R>(&self, func: F) -> R {
        match &self.this {
            NodeType::Slot(this) => {
                // let mut data = this.borrow_mut();
                // func(&mut data.node)
                todo!()
            }
            NodeType::Text(this) => {
                // let mut data = this.borrow_mut();
                // func(&mut data.node)
                todo!()
            }
            NodeType::Element(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.dispatcher)
            }
        }
    }

    pub fn cast_as<CLS: Clone + Type>(&self) -> Option<Virt<'js, CLS>> {
        match &self.this {
            NodeType::Element(this) => {
                let this = this.borrow();
                let func: Function = this
                    .obj
                    .get("castAs")
                    .expect("No castAs function found on Node");
                let res = func
                    .call((This(this.obj.clone()), CLS::CLASS_NAME))
                    .expect("Could not convert");
                Some(res)
            }
            _ => None,
        }
    }

    pub fn num_children(&self) -> usize {
        self.with_node(|node| {
            if let Some(children) = &node.children {
                children.len()
            } else {
                0
            }
        })
    }

    pub fn ctx(&self) -> Ctx<'js> {
        self._ctx
    }
}

#[derive(Clone, Eq)]
pub(in crate) enum NodeType<'js> {
    Slot(Slot<'js>),
    Text(TextNode<'js>),
    Element(Element<'js>),
}

impl<'js> From<Element<'js>> for NodeType<'js> {
    fn from(v: Element<'js>) -> Self {
        Self::Element(v)
    }
}

impl<'js> PartialEq<NodeType<'js>> for NodeType<'js> {
    fn eq(&self, other: &NodeType<'js>) -> bool {
        match (self, other) {
            (NodeType::Slot(left), NodeType::Slot(right)) => left == right,
            (NodeType::Text(left), NodeType::Text(right)) => left == right,
            (NodeType::Element(left), NodeType::Element(right)) => left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
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

impl<'js> NodeData<'js> {
    fn is_mounted(&self) -> bool {
        self.parent.is_some()
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
        if let Some(next) = &self.next {
            next.mark_refs(marker)
        }
        if let Some(prev) = &self.prev {
            prev.mark_refs(marker)
        }
        if let Some(parent) = &self.parent {
            parent.mark_refs(marker)
        }
        if let Some(children) = &self.children {
            children.iter().for_each(|child| child.mark_refs(marker))
        }
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
    fn from_js(_ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let this = if let Some(obj) = value.as_object() {
            match obj {
                obj if obj.instance_of::<Element<'js>>() => {
                    NodeType::Element(Element::from_js_obj(_ctx, value)?)
                }
                obj if obj.instance_of::<Slot<'js>>() => {
                    NodeType::Slot(Slot::from_js_obj(_ctx, value)?)
                }
                obj if obj.instance_of::<TextNode<'js>>() => {
                    NodeType::Text(TextNode::from_js_obj(_ctx, value)?)
                }
                _ => return Err(Error::new_from_js("object", "Node")),
            }
        } else {
            return Err(Error::new_from_js(value.type_name(), "Node"));
        };
        Ok(Node { _ctx, this })
    }
}
