use rquickjs::{Function, This};

use crate::{build_class, inner::*};

pub struct Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
{
    pub(in crate) value: Value<'js>,
    pub(in crate) fw: PhantomData<FW>,
    pub(in crate) inner: ReprOf<FW, CLS>,
}

impl<'js, FW, CLS> Clone for Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
    for<'x> &'x ReprOf<FW, CLS>: IntoRepr<FW, CLS>,
{
    fn clone(&self) -> Self {
        Virt {
            value: self.value.clone(),
            fw: PhantomData,
            inner: self.inner.into_repr(),
        }
    }
}

impl<'js, FW, CLS> Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
{
    pub fn new(value: Value<'js>, inner: ReprOf<FW, CLS>) -> Self {
        Self {
            value,
            inner,
            fw: PhantomData,
        }
    }

    pub fn as_inner<'a, 'b>(&'a self) -> &'b ReprOf<FW, CLS>
    where
        'a: 'b,
    {
        &self.inner
    }
}

impl<'a, FW, CLS> ClassDef for Virt<'a, FW, CLS>
where
    FW: Framework,
    CLS: 'static + Type<FW>,
{
    const CLASS_NAME: &'static str = std::any::type_name::<CLS>();

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type<FW>>::metadata().virt_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build_class!(FW::CLS with InitProto::<FW, CLS>::new(ctx, proto));
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        build_class!(FW::CLS with InitStatic::<FW, CLS>::new(ctx, _static));
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

impl<'js, FW, CLS> rquickjs::IntoJs<'js> for Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: 'static + Type<FW>,
{
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'a, 'js, FW, CLS> rquickjs::FromJs<'js> for Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: 'static + Type<FW>,
    Self: Clone,
    for<'x> &'x ReprOf<FW, CLS>: IntoRepr<FW, CLS>,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let mut_ref = <Virt<'js, FW, CLS> as rquickjs::ClassDef>::from_js_mut(ctx, value.clone());
        match mut_ref {
            Ok(virt) => Ok(virt.clone()),
            Err(err) => match Object::from_js(ctx, value) {
                Ok(obj) => match obj.get::<_, Function<'_>>("asFrameworkClass") {
                    Ok(func) => func.call((This(obj), CLS::name())),
                    _ => Err(err),
                },
                _ => Err(err),
            },
        }
    }
}

impl<'a, 'js, FW, CLS> rquickjs::FromJs<'js> for &'js Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: 'static + Type<FW>,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Virt<'js, FW, CLS> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js, FW, CLS> rquickjs::FromJs<'js> for &'js mut Virt<'js, FW, CLS>
where
    FW: Framework,
    CLS: 'static + Type<FW>,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Virt<'js, FW, CLS> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
