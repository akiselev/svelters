use std::{cell::RefCell, rc::Rc};

use rquickjs::{HasRefs, WithProto};
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


pub struct ElData<'js> {
    pub(in crate) obj: Object<'js>,
    pub(in crate) tag: &'static str,
    pub(in crate) node: NodeData<'js>,
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
    pub(in crate) fn new<CLS: Type + Clone>(
        tag: &'static str,
        obj: Object<'js>
    ) -> Self {
        Self(Inner::new( ElData{
            obj,
            tag,
            node: NodeData::new(),
            class_name: CLS::CLASS_NAME,
        }))
    }
}

#[derive(Copy, Clone)]
pub struct ElementId(pub(in crate) RegisteryKey);

impl ElementId {
    pub fn get_element<'js>(self, ctx: Ctx<'js>) -> Option<Element<'js>> {
        FromJs::from_js(ctx, ctx.get_register(self.0)?).ok()
    }
}
