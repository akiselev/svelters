use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::inner::*;

pub struct DocInner {
    // pub(in crate) types: HashMap<&'static str, TypeDef>,
}

#[derive(Clone)]
pub struct Document {
    pub(in crate) rt: Rt,
    pub(in crate) inner: Inner<DocInner>,
}

impl Document {
    pub fn new() -> Document {
        Document {
            rt: Rt::new(),
            inner: Inner::new(DocInner {}),
        }
    }

    pub fn new_instance<'js, CLS: Type>(&self, ctx: Ctx<'js>, cls: CLS) -> Result<Value<'js>> {
        Obj::new(self.rt.clone(), cls).into_js(ctx)
    }

    pub fn new_element<'js, CLS: Type>(
        &self,
        ctx: Ctx<'js>,
        tag_name: &'static str,
        cls: CLS,
    ) -> Result<Value<'js>> {
        let proto_key = {
            let mut inner = self.rt.inner.deref().borrow();
            let r#type = inner.types.get(Class::<CLS>::typekey()).unwrap();
            r#type
                .element_proto
                .ok_or_else(|| exception!("Type {} is not initialized", CLS::CLASS_NAME))?
        };
        let proto: Object = ctx
            .get_register(proto_key)
            .ok_or_else(|| exception!("Could not get proto for type {}", CLS::CLASS_NAME))?
            .as_object()
            .ok_or_else(|| exception!("Proto for type {} is not an object", CLS::CLASS_NAME))?
            .clone();
        let el = ElementNode::new::<CLS>(self.rt.clone(), ctx, tag_name, cls)?;
        WithProto(el, proto).into_js(ctx)
    }

    pub(in crate) fn create_event<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _interface: String,
    ) -> Result<Value<'js>> {
        Event::custom(ctx)
    }

    pub(in crate) fn create_element_ns<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _ns: Value<'js>,
        name: String,
        _options: Opt<Object<'js>>,
    ) -> Result<Value<'js>> {
        self.create_element(ctx, name)
    }

    pub(in crate) fn create_element<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
    ) -> Result<Value<'js>> {
        if name == "slot" {
            return SlotNode::new().into_js(ctx);
        }
        if let Some(element) = self.rt.inner.borrow().elements.get(name.as_str()) {
            (element)(ctx)
        } else {
            Err(exception!("{} is not a valid element", name))
        }
    }

    pub(in crate) fn dispatch_event<'js>(
        &mut self,
        _ctx: Ctx<'js>,
        _event: Object<'js>,
        _options: Opt<Object<'js>>,
    ) -> Result<()> {
        Ok(())
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

    fn init_proto<'js>(_ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        proto.set(
            "createEvent",
            rquickjs::Func::new("createEvent", rquickjs::Method(Document::create_event)),
        )?;
        proto.set(
            "createElement",
            rquickjs::Func::new("createElement", rquickjs::Method(Document::create_element)),
        )?;
        proto.set(
            "createElementNS",
            rquickjs::Func::new(
                "createElementNS",
                rquickjs::Method(Document::create_element_ns),
            ),
        )?;
        proto.set(
            "dispatchEvent",
            rquickjs::Func::new("dispatchEvent", rquickjs::Method(Document::dispatch_event)),
        )?;
        Ok(())
    }

    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
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
