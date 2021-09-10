use std::cell::Ref;

use rquickjs::{Function, HasRefs, This};

use crate::{build_class, inner::*};

#[derive(Clone)]
pub struct Virt<'js, CLS> {
    pub(in crate) rt: Rt,
    pub(in crate) idx: Idx,
    pub(in crate) cls: CLS,
    pub(in crate) obj: Object<'js>,
}

impl<'js, CLS> HasRefs for Virt<'js, CLS> {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.obj.mark(marker)
    }
}

impl<'js, CLS> Virt<'js, CLS>
where
    CLS: Type,
{
    pub(in crate) fn new(rt: Rt, idx: Idx, cls: CLS, obj: Object<'js>) -> Self {
        Self { rt, idx, obj, cls }
    }

    pub fn get<T>(&self, name: &str) -> Result<T>
    where
        T: FromJs<'js>,
    {
        self.obj.get(name)
    }

    pub fn as_object<'a, 'b>(&'a self) -> &'b Object<'js>
    where
        'a: 'b,
    {
        &self.obj
    }

    pub fn cast_as<OTHER: Clone + Type>(&self) -> Option<Virt<'js, OTHER>> {
        let func: Function = self.obj.get("castAs").ok()?;
        func.call((This(self.obj.clone()), OTHER::CLASS_NAME)).ok()
    }
}

impl<'a, CLS> ClassDef for Virt<'a, CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = CLS::CLASS_NAME;

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
        self.into_js_obj(ctx)
    }
}

impl<'js, CLS> rquickjs::FromJs<'js> for Virt<'js, CLS>
where
    CLS: 'static + Type,
    Self: Clone,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let obj = value
            .as_object()
            .ok_or_else(|| Error::new(ctx, format!("Could not cast to {}", CLS::CLASS_NAME)))?;
        if obj.instance_of::<Self>() {
            return Self::from_js_obj(ctx, value);
        }
        let func: Function = obj.get::<_, Function<'_>>("castAs")?;
        let res = func.call((This(obj.clone()), CLS::name()));
        res
    }
}

impl<'js, BASE, CLS> PartialEq<Virt<'js, CLS>> for Virt<'js, BASE> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        self.obj == other.obj
    }
}

impl<'js, CLS> PartialEq<Object<'js>> for Virt<'js, CLS> {
    fn eq(&self, other: &Object<'js>) -> bool {
        self.obj == *other
    }
}

impl<'js, CLS> PartialEq<Value<'js>> for Virt<'js, CLS> {
    fn eq(&self, other: &Value<'js>) -> bool {
        if let Some(other) = other.as_object() {
            self.obj == *other
        } else {
            false
        }
    }
}

impl<'js, CLS> Eq for Virt<'js, CLS> {}
