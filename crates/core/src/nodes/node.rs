use rquickjs::{Function, HasRefs};
use smallvec::SmallVec;

use crate::inner::*;

use crate::inner::*;

#[derive(Clone)]
pub struct Node<'js> {
    pub(in crate) _ctx: Ctx<'js>,
    pub(in crate) this: UINode<'js>,
    pub(in crate) doc_cell: OnceCell<Document>,
}

impl<'js> PartialEq for Node<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.this == other.this
    }
}

impl<'js> PartialEq<ElementNode<'js>> for Node<'js> {
    fn eq(&self, other: &ElementNode<'js>) -> bool {
        match &self.this {
            UINode::Element(this) => this.idx == other.idx,
            _ => false,
        }
    }
}

impl<'js, CLS> PartialEq<Virt<'js, CLS>> for Node<'js> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        match &self.this {
            UINode::Element(this) => this.idx == other.idx,
            _ => false,
        }
    }
}

impl<'js> Eq for Node<'js> {}

impl<'js> Node<'js> {
    pub fn append_node(&self, node: Node<'js>) {
        let parent = self.this.clone();
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
                    data.next = Some(node.this.clone());
                });
                Some(child.clone())
            } else {
                None
            };
            let prev = prev.and_then(|prev| {
                prev.with_node(|data| data.next = Some(node.this.clone()));
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
                    .position(|this| *this == node.this)
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
        let parent = self.this.clone();
        self.with_node(move |this| {
            if this.is_mounted() {
                panic!("Tried to insert node that is already mounted")
            }
            let children = this.children.get_or_insert_with(|| vec![]);
            let index = {
                children
                    .iter_mut()
                    .position(|this| *this == anchor.this)
                    .expect("Can't find anchor for insert")
            };
            let target = children[index].clone();
            let prev = target.with_node(|data| {
                let prev = data.prev.take();
                data.prev = Some(node.this.clone());
                prev
            });
            if let Some(prev) = &prev {
                prev.with_node(|data| data.next = Some(node.this.clone()));
            }
            node.with_node(move |data| {
                data.parent = Some(parent);
                data.prev = prev;
                data.next = Some(target);
            });
            children.insert(index, node.this);
        })
    }

    pub fn index_of(&self, node: Node<'js>) -> Option<usize> {
        self.with_node(move |this| {
            if let Some(children) = &this.children {
                children.iter().position(|this| *this == node.this)
            } else {
                None
            }
        })
    }

    fn with_node<F: FnOnce(&mut NodeData<'js>) -> R, R>(&self, func: F) -> R {
        self.this.with_node(func)
    }

    pub fn document(&self) -> Result<&Document> {
        self.doc_cell.get_or_try_init(|| {
            let doc: &Document = self.ctx().globals().get("document")?;
            Result::Ok((*doc).clone())
        })
    }

    pub fn cast_as<CLS: Clone + Type>(&self) -> Option<ObjRef<CLS>> {
        match &self.this {
            UINode::Element(this) => this.cast_as::<CLS>(),
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
pub(in crate) enum UINode<'js> {
    Slot(SlotNode<'js>),
    Text(TextNode<'js>),
    Element(ElementNode<'js>),
}

impl<'js> UINode<'js> {
    fn with_node<F: FnOnce(&mut NodeData<'js>) -> R, R>(&self, func: F) -> R {
        match self {
            UINode::Slot(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
            UINode::Text(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
            UINode::Element(this) => {
                let mut data = this.borrow_mut();
                func(&mut data.node)
            }
        }
    }
}

impl<'js> From<ElementNode<'js>> for UINode<'js> {
    fn from(v: ElementNode<'js>) -> Self {
        Self::Element(v)
    }
}

impl<'js> From<SlotNode<'js>> for UINode<'js> {
    fn from(v: SlotNode<'js>) -> Self {
        Self::Slot(v)
    }
}

impl<'js> From<TextNode<'js>> for UINode<'js> {
    fn from(v: TextNode<'js>) -> Self {
        Self::Text(v)
    }
}

impl<'js> PartialEq<UINode<'js>> for UINode<'js> {
    fn eq(&self, other: &UINode<'js>) -> bool {
        match (self, other) {
            (UINode::Slot(left), UINode::Slot(right)) => left == right,
            (UINode::Text(left), UINode::Text(right)) => left == right,
            (UINode::Element(left), UINode::Element(right)) => left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(in crate) struct NodeData<'js> {
    pub(in crate) next: Option<UINode<'js>>,
    pub(in crate) prev: Option<UINode<'js>>,
    pub(in crate) parent: Option<UINode<'js>>,
    pub(in crate) children: Option<Vec<UINode<'js>>>,
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

impl<'js> HasRefs for UINode<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        match &self {
            UINode::Slot(node) => node.mark_refs(marker),
            UINode::Text(node) => node.mark_refs(marker),
            UINode::Element(node) => <ElementNode<'js> as HasRefs>::mark_refs(&node, marker),
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
        self.this.into_js(ctx)
    }
}

impl<'js> rquickjs::IntoJs<'js> for UINode<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        Ok(match self {
            UINode::Slot(node) => node.into_js(ctx)?,
            UINode::Text(node) => node.into_js(ctx)?,
            UINode::Element(node) => node.into_js(ctx)?,
        })
    }
}

impl<'js, 'a> rquickjs::FromJs<'js> for Node<'js> {
    fn from_js(_ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let this = if let Some(obj) = value.as_object() {
            match obj {
                obj if obj.instance_of::<ElementNode<'js>>() => {
                    UINode::Element(ElementNode::from_js_obj(_ctx, value)?)
                }
                obj if obj.instance_of::<SlotNode<'js>>() => {
                    UINode::Slot(SlotNode::from_js_obj(_ctx, value)?)
                }
                obj if obj.instance_of::<TextNode<'js>>() => {
                    UINode::Text(TextNode::from_js_obj(_ctx, value)?)
                }
                _ => return Err(Error::new_from_js("object", "Node")),
            }
        } else {
            return Err(Error::new_from_js(value.type_name(), "Node"));
        };
        Ok(Node {
            _ctx,
            this,
            doc_cell: OnceCell::new(),
        })
    }
}
