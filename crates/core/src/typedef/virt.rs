use rquickjs::{Function, HasRefs, This};

use crate::{build_class, inner::*};

pub struct Virt<'js, CLS> {
    pub(in crate) inner: CLS,
    pub(in crate) obj: Object<'js>,
}

impl<'js, CLS> Deref for Virt<'js, CLS> {
    type Target = CLS;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js, CLS> HasRefs for Virt<'js, CLS> {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.obj.mark(marker)
    }
}


impl<'js, CLS> Clone for Virt<'js, CLS>
where
    CLS: Type + Clone,
{
    fn clone(&self) -> Self {
        Virt {
            inner: self.inner.clone(),
            obj: self.obj.clone()
        }
    }
}

impl<'js, CLS> Virt<'js, CLS>
where
    CLS: Type,
{
    pub fn new(inner: CLS, obj: Object<'js>) -> Self {
        Self { inner, obj }
    }

    pub fn as_inner<'a, 'b>(&'a self) -> &'b CLS
    where
        'a: 'b,
    {
        &self.inner
    }

    pub fn as_object<'a, 'b>(&'a self) -> &'b Object<'js>
    where
        'a: 'b,
    {
        &self.obj
    }
}

impl<'a, CLS> ClassDef for Virt<'a, CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = std::any::type_name::<CLS>();

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type>::metadata().virt_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build_class!(CLS with InitVirt::<CLS>::new(ctx, proto));
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        build_class!(CLS with InitStatic::<CLS>::new(ctx, _static));
        Ok(())
    }

    fn mark_refs(&self, marker: &RefsMarker) {
        self.obj.mark(marker)
    }

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

impl<'js, CLS> rquickjs::IntoJs<'js> for Virt<'js, CLS>
where
    CLS: 'static + Type,
{
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js, CLS> rquickjs::FromJs<'js> for Virt<'js, CLS>
where
    CLS: 'static + Type,
    Self: Clone,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let mut_ref = <Virt<'js, CLS> as rquickjs::ClassDef>::from_js_mut(ctx, value.clone());
        match mut_ref {
            Ok(virt) => Ok(virt.clone()),
            Err(err) => match Object::from_js(ctx, value) {
                Ok(obj) => match obj.get::<_, Function<'_>>("castAs") {
                    Ok(func) => func.call((This(obj), CLS::name())),
                    _ => Err(err),
                },
                _ => Err(err),
            },
        }
    }
}
