use std::{borrow::Borrow, cell::Ref};

use ouroboros::self_referencing;

use crate::{build_class, inner::*};

pub struct Obj<CLS>
where
    CLS: Type,
{
    pub(in crate) rt: Rt,
    pub(in crate) idx: Idx,
    pub(in crate) cls: Class<CLS>,
}

impl<CLS> Obj<CLS>
where
    CLS: Type,
{
    pub fn new(rt: Rt, inner: CLS) -> Self {
        let idx = rt.new_instance(inner);
        Self {
            rt,
            idx,
            cls: Class::new(),
        }
    }

    pub fn borrow(&self) -> Result<ObjRef<CLS>> {
        Obj::<CLS>::from_rt(&self.rt, self.idx)
    }

    pub(in crate) fn from_rt(rt: &Rt, idx: Idx) -> Result<ObjRef<CLS>> {
        let rt = rt.objects.deref().borrow();
        ObjRefTryBuilder {
            rt,
            obj_builder: |this: &Ref<ObjMap>| {
                let inst = this.get(idx).ok_or_else(|| {
                    exception!("Could not find object for Obj<{}>", CLS::CLASS_NAME)
                })?;
                inst.data
                    .downcast_ref::<CLS>()
                    .ok_or_else(|| exception!("Could not cast to {}", CLS::CLASS_NAME))
            },
            cls: Class::new(),
        }
        .try_build()
    }
}

#[self_referencing]
pub struct ObjRef<'a, CLS>
where
    CLS: 'a,
{
    pub(in crate) rt: Ref<'a, ObjMap>,
    #[borrows(rt)]
    pub(in crate) obj: &'this CLS,
    pub(in crate) cls: Class<CLS>,
}

impl<'a, CLS> std::ops::Deref for ObjRef<'a, CLS>
where
    CLS: 'a,
{
    type Target = CLS;

    fn deref(&self) -> &Self::Target {
        self.borrow_obj()
    }
}

impl<'a, CLS> ClassDef for Obj<CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = CLS::CLASS_NAME;

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type>::metadata().obj_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build!(CLS::InitProto::<CLS>[ctx, proto, 0]);
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        build!(CLS::InitStatic::<CLS>[ctx, _static]);
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
