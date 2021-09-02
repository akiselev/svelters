use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::inner::*;

pub struct DocInner {
    pub(in crate) events: EventHandlers,
    pub(in crate) elements: HashMap<&'static str, ElementType>,
}

pub struct Document {
    pub(in crate) inner: Rc<RefCell<DocInner>>,
}

impl Clone for Document {
    fn clone(&self) -> Self {
        panic!("Can't clone Document struct")
    }
}

impl Document {
    pub fn new() -> Document {
        Document {
            inner: Rc::new(RefCell::new(DocInner {
                events: Default::default(),
                elements: Default::default(),
            })),
        }
    }
}

impl ClassDef for Document {
    const CLASS_NAME: &'static str = "Document";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        proto.set(
            "createEvent",
            rquickjs::Func::new("createEvent", rquickjs::Method(create_event)),
        )?;
        proto.set(
            "createElement",
            rquickjs::Func::new("createElement", rquickjs::Method(create_element)),
        )?;
        proto.set(
            "createElementNS",
            rquickjs::Func::new("createElementNS", rquickjs::Method(create_element_ns)),
        )?;
        proto.set(
            "dispatchEvent",
            rquickjs::Func::new("dispatchEvent", rquickjs::Method(dispatch_event)),
        )?;
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, _marker: &RefsMarker) {}

    fn into_js_obj<'js>(self, ctx: Ctx<'js>) -> Result<Value<'js>>
    where
        Self: Sized,
    {
        JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
    }

    fn from_js_ref<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_ref(ctx, &value)
    }

    fn from_js_mut<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_mut(ctx, &value)
    }

    fn from_js_obj<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = JsClass::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}

impl<'js> rquickjs::IntoJs<'js> for Document {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Document as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js Document {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Document as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js mut Document {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Document as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
