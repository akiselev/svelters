use std::{cell::RefCell, rc::Rc};

use rquickjs::{HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

pub(in crate) struct ElInfo {
    id: ElId,
    slot_name: Cow<'static, str>,
}

pub(in crate) struct ElInner {
    pub(in crate) next: Option<ElId>,
    pub(in crate) prev: Option<ElId>,
    pub(in crate) parent: Option<ElId>,
    pub(in crate) children: SmallVec<[ElInfo; 10]>,
}

impl ElInner {
    fn new() -> Self { Self {  
        children: SmallVec::new(),
        next: None,
        prev: None,
        parent: None,
    } }
}

#[derive(Clone)]
pub struct Element<'js> {
    pub(in crate) id: ElId,
    pub(in crate) class_name: &'static str,
    pub(in crate) inner: Rc<RefCell<ElInner>>,
    pub(in crate) obj: Value<'js>,
}

impl<'js> HasRefs for Element<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.obj.as_object().unwrap().mark(marker)
    }
}

impl<'js> std::ops::Deref for Element<'js> {
    type Target = Value<'js>;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<'js> Element<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(
        id: ElId,
        obj: Value<'js>
    ) -> Self {
        Self {
            id,
            obj,
            class_name: CLS::CLASS_NAME,
            inner: Rc::new(RefCell::new(ElInner::new())),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ElementId(pub(in crate) RegisteryKey);

impl ElementId {
    pub fn get_element<'js>(self, ctx: Ctx<'js>) -> Option<Element<'js>> {
        FromJs::from_js(ctx, ctx.get_register(self.0)?).ok()
    }
}
