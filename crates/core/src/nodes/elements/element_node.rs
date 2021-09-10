use std::{cell::RefCell, rc::Rc};

use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct ElementNode<'js> {
    pub(in crate) rt: Rt,
    pub(in crate) idx: Idx,
    pub(in crate) typekey: TypeKey,
    pub(in crate) tag_name: &'static str,
    pub(in crate) class_name: &'static str,
    pub(in crate) inner: Inner<ElData<'js>>,
}

impl<'js> std::ops::Deref for ElementNode<'js> {
    type Target = Inner<ElData<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js> PartialEq for ElementNode<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<'js> PartialEq<Node<'js>> for ElementNode<'js> {
    fn eq(&self, other: &Node<'js>) -> bool {
        match &other.this {
            UINode::Slot(_) => false,
            UINode::Text(_) => false,
            UINode::Element(node) => self.idx == node.idx,
        }
    }
}

impl<'js> PartialEq<SlotNode<'js>> for ElementNode<'js> {
    fn eq(&self, _other: &SlotNode<'js>) -> bool {
        false
    }
}

impl<'js, CLS> PartialEq<Virt<'js, CLS>> for ElementNode<'js> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        self.idx == other.idx
    }
}

impl<'js> Eq for ElementNode<'js> {}

pub struct ElData<'js> {
    pub(in crate) node: NodeData<'js>,
    pub(in crate) dispatcher: Dispatcher<'js>,
}

impl<'js> HasRefs for ElementNode<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        let this = self.inner.borrow();
        this.node.mark_refs(marker)
    }
}

impl<'js> ElementNode<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(
        rt: Rt,
        ctx: Ctx<'js>,
        tag_name: &'static str,
        cls: CLS,
    ) -> Result<Self> {
        let idx = rt.new_instance(cls);
        let dispatcher = Dispatcher::new();
        Ok(Self {
            rt,
            idx,
            typekey: Class::<CLS>::typekey(),
            tag_name,
            class_name: CLS::CLASS_NAME,
            inner: Inner::new(ElData {
                dispatcher,
                node: NodeData::new(),
            }),
        })
    }
}
