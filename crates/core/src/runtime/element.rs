use std::{cell::RefCell, rc::Rc};

use rquickjs::WithProto;
use smallvec::SmallVec;

use crate::inner::*;

pub(in crate) struct ElInner {
    pub(in crate) next: Option<Idx>,
    pub(in crate) prev: Option<Idx>,
    pub(in crate) parent: Option<Idx>,
    pub(in crate) children: SmallVec<[Idx; 10]>,
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
    pub(in crate) id: Idx,
    pub(in crate) class_name: &'static str,
    pub(in crate) inner: Rc<RefCell<ElInner>>,
    pub(in crate) obj: Value<'js>,
}

impl<'js> std::ops::Deref for Element<'js> {
    type Target = Value<'js>;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<'js> Element<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(
        id: Idx,
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

impl<'js> ClassDef for Element<'js> {
    const CLASS_NAME: &'static str = "Element<'js>";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'ctx>(_ctx: Ctx<'ctx>, _proto: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn init_static<'ctx>(_ctx: Ctx<'ctx>, _static: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, marker: &RefsMarker) {
        self.obj.as_object().unwrap().mark(marker)
    }

    fn into_js_obj<'ctx>(self, ctx: Ctx<'ctx>) -> Result<Value<'ctx>>
    where
        Self: Sized,
    {
        JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
    }

    fn from_js_ref<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_ref(ctx, &value)
    }

    fn from_js_mut<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_mut(ctx, &value)
    }

    fn from_js_obj<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = JsClass::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}

impl<'js> rquickjs::IntoJs<'js> for Element<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'a, 'js> rquickjs::FromJs<'js> for Element<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Element<'js> as rquickjs::ClassDef>::from_js_obj(ctx, value)
    }
}

impl<'a, 'js> rquickjs::FromJs<'js> for &'js Element<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Element<'js> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'a, 'js> rquickjs::FromJs<'js> for &'js mut Element<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Element<'js> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}

#[derive(Copy, Clone)]
pub struct ElementId(pub(in crate) RegisteryKey);

impl ElementId {
    pub fn get_element<'js>(self, ctx: Ctx<'js>) -> Option<Element<'js>> {
        FromJs::from_js(ctx, ctx.get_register(self.0)?).ok()
    }
}
