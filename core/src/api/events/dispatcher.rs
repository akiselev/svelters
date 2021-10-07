use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

use rquickjs::{Function, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub struct Dispatcher<'js> {
    pub(in crate) inner: Inner<DispInner<'js>>,
}

pub(in crate) struct DispInner<'js> {
    pub(in crate) subs: HashMap<&'static str, Callbacks<'js>>,
}

impl<'js> Dispatcher<'js> {
    pub fn new() -> Self {
        tracing::info!("Dispatcher created...");
        Self {
            inner: Inner::new(DispInner {
                subs: Default::default(),
            }),
        }
    }

    pub fn dispatch(&self, ctx: Ctx<'js>, event: Event<'js>) -> Result<()> {
        tracing::info!("Dispatch inner2");
        let inner = self.inner.borrow();
        let name = event
            ._type
            .as_ref()
            .ok_or_else(|| exception!("Event has no type"))?
            .as_str();
        if let Some(cb) = inner.subs.get(name) {
            let inner = cb.inner.borrow();
            tracing::info!(
                "Found {} subscriptions for event {}",
                inner.callbacks.len(),
                name
            );
            let value = event.into_js(ctx)?;
            for func in inner.callbacks.iter() {
                match func.call::<_, Value>((value.clone(),)) {
                    Ok(ret) => tracing::info!("Called event handler: {}", ret.type_of()),
                    Err(err) => tracing::error!("Error in event handler: {}", err),
                }
            }
            Ok(())
        } else {
            tracing::error!("Event {} not found", name);
            Err(exception!("Event named {} not found", name))
        }
    }

    delegate::delegate! {
        to self.inner {
            pub(in crate) fn borrow(&self) -> Ref<DispInner<'js>>;
            pub(in crate) fn borrow_mut(&self) -> RefMut<DispInner<'js>>;
        }
    }
}

impl<'x> ClassDef for Dispatcher<'x> {
    const CLASS_NAME: &'static str = "Dispatcher";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = false;

    fn init_proto<'ctx>(_ctx: Ctx<'ctx>, _proto: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn init_static<'ctx>(_ctx: Ctx<'ctx>, _static: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, marker: &RefsMarker) {
        // HasRefs::mark_refs(self, marker)
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

impl<'js> rquickjs::IntoJs<'js> for Dispatcher<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js> rquickjs::FromJs<'js> for Dispatcher<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Dispatcher<'js> as rquickjs::ClassDef>::from_js_obj(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js Dispatcher<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Dispatcher<'js> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js mut Dispatcher<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Dispatcher<'js> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
