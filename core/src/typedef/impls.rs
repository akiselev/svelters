use std::cell::Ref;

use rquickjs::{Function, HasRefs, This};

use crate::{build_class, inner::*};

#[derive(Clone)]
pub struct Impl<CLS> {
    pub(in crate) cls: CLS,
}

impl<CLS> std::ops::Deref for Impl<CLS> {
    type Target = CLS;

    fn deref(&self) -> &Self::Target {
        &self.cls
    }
}

impl<CLS> Impl<CLS>
where
    CLS: Type,
{
    pub fn new(cls: CLS) -> Self {
        Self { cls }
    }
}

impl<CLS> ClassDef for Impl<CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = CLS::CLASS_NAME;

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type>::metadata().impl_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = false;

    const HAS_REFS: bool = false;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build!(CLS::InitImpl::<CLS>[ctx, proto]);
        Ok(())
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

impl<'js, CLS> rquickjs::IntoJs<'js> for Impl<CLS>
where
    CLS: 'static + Type,
{
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js, CLS> rquickjs::FromJs<'js> for Impl<CLS>
where
    CLS: 'static + Type,
    Self: Clone,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let obj = value.as_object().ok_or_else(|| {
            exception!(
                "Expected object implementing {}, found {:?}",
                CLS::CLASS_NAME,
                value
            )
        })?;
        if obj.instance_of::<Self>() {
            return Self::from_js_obj(ctx, value);
        }
        let func: Function = obj.get::<_, Function<'_>>("asImpl")?;
        let res: Option<Value> = func.call((This(obj.clone()), CLS::name()))?;
        let res =
            res.ok_or_else(|| exception!("Could not get implementation of {}", CLS::CLASS_NAME))?;
        Impl::<CLS>::from_js_obj(ctx, res)
    }
}

// impl<'js, BASE, CLS> PartialEq<Impl<CLS>> for Impl<BASE> {
//     fn eq(&self, other: &Impl<CLS>) -> bool {
//         self.obj == other.obj
//     }
// }

// impl<'js, CLS> PartialEq<Object<'js>> for Impl<CLS> {
//     fn eq(&self, other: &Object<'js>) -> bool {
//         self.obj == *other
//     }
// }

// impl<'js, CLS> PartialEq<Value<'js>> for Impl<CLS> {
//     fn eq(&self, other: &Value<'js>) -> bool {
//         if let Some(other) = other.as_object() {
//             self.obj == *other
//         } else {
//             false
//         }
//     }
// }

// impl<'js, CLS> Eq for Impl<CLS> {}
