use rquickjs::HasRefs;

use crate::inner::*;

impl<'js> ClassDef for Element<'js> {
    const CLASS_NAME: &'static str = "Element";

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
        HasRefs::mark_refs(self, marker)
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
