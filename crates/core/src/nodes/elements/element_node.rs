use std::{cell::RefCell, rc::Rc};

use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct ElementNode<'js> {
    pub(in crate) tag_name: &'static str,
    pub(in crate) class_name: &'static str,
    pub(in crate) obj_id: RegisteryKey,
    pub(in crate) dispatcher_id: RegisteryKey,
    inner: Inner<ElData<'js>>,
}

impl<'js> std::ops::Deref for ElementNode<'js> {
    type Target = Inner<ElData<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js> PartialEq for ElementNode<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.borrow().obj == other.borrow().obj
    }
}

impl<'js> PartialEq<Node<'js>> for ElementNode<'js> {
    fn eq(&self, other: &Node<'js>) -> bool {
        match &other.this {
            NodeType::Slot(_) => false,
            NodeType::Text(_) => false,
            NodeType::Element(node) => self.borrow().obj == node.borrow().obj,
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
        self.borrow().obj == other.obj
    }
}

impl<'js> PartialEq<Object<'js>> for ElementNode<'js> {
    fn eq(&self, other: &Object<'js>) -> bool {
        self.borrow().obj == *other
    }
}

impl<'js> PartialEq<Value<'js>> for ElementNode<'js> {
    fn eq(&self, other: &Value<'js>) -> bool {
        if let Some(other) = other.as_object() {
            self.borrow().obj == *other
        } else {
            false
        }
    }
}

impl<'js> Eq for ElementNode<'js> {}

pub struct ElData<'js> {
    pub(in crate) obj: Object<'js>,
    pub(in crate) node: NodeData<'js>,
    pub(in crate) dispatcher: Dispatcher,
}

impl<'js> HasRefs for ElementNode<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        let this = self.inner.borrow();
        this.obj.mark(marker);
        this.node.mark_refs(marker)
    }
}

impl<'js> std::ops::Deref for ElData<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<'js> ElementNode<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(
        ctx: Ctx<'js>,
        tag_name: &'static str,
        obj: Obj<CLS>,
    ) -> Result<Self> {
        let obj = obj
            .into_js_obj(ctx)?
            .into_object()
            .expect("Sould be object");
        let obj_id = ctx.register(obj.clone().into_value());
        let dispatcher = Dispatcher::new();
        let dispatcher_id = ctx.register(dispatcher.clone().into_js_obj(ctx)?);
        Ok(Self {
            tag_name,
            class_name: CLS::CLASS_NAME,

            obj_id,
            dispatcher_id,
            inner: Inner::new(ElData {
                obj,
                dispatcher,
                node: NodeData::new(),
            }),
        })
    }

    pub fn cast_as<CLS: Clone + Type>(&self) -> Option<Virt<'js, CLS>> {
        let this = self.borrow();
        let func: Function = this
            .obj
            .get("castAs")
            .expect("No castAs function found on Node");
        let res = func
            .call((This(this.obj.clone()), CLS::CLASS_NAME))
            .expect("Could not convert");
        Some(res)
    }
}
