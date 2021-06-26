use std::collections::HashMap;

use crate::inner::*;

pub struct Obj<FW, const CLS: &'static str>
where
    FW: FrameworkClass<CLS> + Container<CLS> + Framework,
{
    fw: PhantomData<FW>,
    inner: ClsBox<FW, CLS>,
}

impl<FW, const CLS: &'static str> Obj<FW, CLS>
where
    FW: FrameworkClass<CLS> + Container<CLS> + Framework,
{
    pub fn new(inner: ClsBox<FW, CLS>) -> Self {
        Self {
            fw: PhantomData,
            inner,
        }
    }
}

impl<'a, FW, const CLS: &'static str> ClassDef for Obj<FW, CLS>
where
    FW: Framework + Container<CLS> + FrameworkClass<CLS>,
    <FW as FrameworkClass<CLS>>::Proto: ClassProto,
    <FW as FrameworkClass<CLS>>::Static: ClassStatic,
    <FW as FrameworkClass<CLS>>::Constructor: JsModuleDef,
{
    const CLASS_NAME: &'static str = CLS;

    unsafe fn class_id() -> &'static mut ClassId {
        <FW as FrameworkClass<CLS>>::get_id()
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        let def = <FW as FrameworkClass<CLS>>::get_proto();
        def.init_proto(ctx, proto)?;
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        let def = <FW as FrameworkClass<CLS>>::get_static();
        def.init_static(ctx, _static)?;
        Ok(())
    }

    fn mark_refs(&self, _marker: &RefsMarker) {}

    fn into_js_obj<'js>(self, ctx: Ctx<'js>) -> Result<Value<'js>>
    where
        Self: Sized,
    {
        rquickjs::Class::<Self>::instance(ctx, self).map(|cls| cls.into_value())
    }

    fn from_js_ref<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        rquickjs::Class::<Self>::try_ref(ctx, &value)
    }

    fn from_js_mut<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        rquickjs::Class::<Self>::try_mut(ctx, &value)
    }

    fn from_js_obj<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = rquickjs::Class::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}
