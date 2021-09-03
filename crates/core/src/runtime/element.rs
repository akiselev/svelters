use std::{cell::RefCell, collections::HashMap, lazy::OnceCell, rc::Rc};

use rquickjs::{Function, This, WithProto};

use crate::{build_class, inner::*};

#[derive(Clone)]
pub struct Data<'js> {
    pub(in crate) mounted: bool,
    pub(in crate) parent: Option<Object<'js>>,
    pub(in crate) class_name: &'static str,
}

#[derive(Clone)]
pub struct Element<'js> {
    pub(in crate) data: Rc<RefCell<Data<'js>>>,
    pub(in crate) inner: Object<'js>,
}

impl<'js> Deref for Element<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js> Element<'js> {
    pub fn new<CLS: Type + Clone>(
        ctx: Ctx<'js>,
        class_name: &'static str,
        inner: CLS,
        proto: Object<'js>,
    ) -> Result<Value<'js>> {
        let inner = Obj::new(inner).into_js_obj(ctx)?.into_object().unwrap();
        let data = Element {
            data: Rc::new(RefCell::new(Data {
                parent: None,
                mounted: false,
                class_name: class_name,
            })),
            inner: inner,
        };
        WithProto(data, proto).into_js(ctx)
    }
}

impl<'x> ClassDef for Element<'x> {
    const CLASS_NAME: &'static str = "Element";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
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

impl<'js> rquickjs::IntoJs<'js> for Element<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
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
