use std::{cell::RefCell, rc::Rc};

use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct Element<'js>(Inner<ElData<'js>>);

impl<'js> std::ops::Deref for Element<'js> {
    type Target = Inner<ElData<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> PartialEq for Element<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.borrow().obj == other.borrow().obj
    }
}

impl<'js> PartialEq<Node<'js>> for Element<'js> {
    fn eq(&self, other: &Node<'js>) -> bool {
        match &other.this {
            NodeType::Slot(_) => false,
            NodeType::Text(_) => false,
            NodeType::Element(node) => self.borrow().obj == node.borrow().obj,
        }
    }
}

impl<'js> PartialEq<Slot<'js>> for Element<'js> {
    fn eq(&self, other: &Slot<'js>) -> bool {
        false
    }
}

impl<'js, CLS> PartialEq<Virt<'js, CLS>> for Element<'js> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        self.borrow().obj == other.obj
    }
}

impl<'js> PartialEq<Object<'js>> for Element<'js> {
    fn eq(&self, other: &Object<'js>) -> bool {
        self.borrow().obj == *other
    }
}

impl<'js> PartialEq<Value<'js>> for Element<'js> {
    fn eq(&self, other: &Value<'js>) -> bool {
        if let Some(other) = other.as_object() {
            self.borrow().obj == *other
        } else {
            false
        }
    }
}

impl<'js> Eq for Element<'js> {}

pub struct ElData<'js> {
    pub(in crate) obj: Object<'js>,
    pub(in crate) tag: &'static str,
    pub(in crate) node: NodeData<'js>,
    pub dispatcher: Dispatcher,
    pub(in crate) class_name: &'static str,
}

impl<'js> HasRefs for Element<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        let this = self.0.borrow();
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

impl<'js> Element<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(tag: &'static str, obj: Object<'js>) -> Self {
        Self(Inner::new(ElData {
            obj,
            tag,
            dispatcher: Dispatcher::new(),
            node: NodeData::new(),
            class_name: CLS::CLASS_NAME,
        }))
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

#[derive(Copy, Clone)]
pub struct ElementId(pub(in crate) RegisteryKey);

impl ElementId {
    pub fn get_element<'js>(self, ctx: Ctx<'js>) -> Option<Element<'js>> {
        FromJs::from_js(ctx, ctx.get_register(self.0)?).ok()
    }
}
