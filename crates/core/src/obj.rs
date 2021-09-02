use crate::{build_class, inner::*};

pub struct Obj<CLS>
where
    CLS: Type,
{
    pub(in crate) inner: ReprOf<CLS>,
}

impl<'a, CLS> Obj<CLS>
where
    CLS: Type,
{
    pub fn new(inner: ReprOf<CLS>) -> Self {
        Self { inner }
    }

    pub fn as_inner<'b>(&'a self) -> &'b ReprOf<CLS>
    where
        'a: 'b,
    {
        &self.inner
    }
}

impl<'a, CLS> ClassDef for Obj<CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = std::any::type_name::<CLS>();

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type>::metadata().obj_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build_class!(CLS with InitProto::<CLS>::new(ctx, proto));
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        build_class!(CLS with InitStatic::<CLS>::new(ctx, _static));
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

impl<'a, 'js, CLS> rquickjs::IntoJs<'js> for Obj<CLS>
where
    CLS: 'static + Type,
{
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Obj<CLS> as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'a, 'js, CLS> rquickjs::FromJs<'js> for &'js Obj<CLS>
where
    CLS: 'static + Type,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Obj<CLS> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'a, 'js, CLS> rquickjs::FromJs<'js> for &'js mut Obj<CLS>
where
    CLS: 'static + Type,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Obj<CLS> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
