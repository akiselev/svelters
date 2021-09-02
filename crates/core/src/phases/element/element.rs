use std::{collections::HashMap, lazy::OnceCell};

use rquickjs::{Function, This};

use crate::{build_class, inner::*};

pub struct Element<CLS> {
    pub(in crate) inner: CLS,
}

impl<'js, CLS> Deref for Element<CLS> {
    type Target = CLS;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js, CLS> Clone for Element<CLS>
where
    CLS: Type + Clone,
{
    fn clone(&self) -> Self {
        Element {
            inner: self.inner.clone(),
        }
    }
}

impl<'js, CLS> Element<CLS>
where
    CLS: Type,
{
    pub fn new(inner: CLS) -> Self {
        Self { inner }
    }

    pub fn as_inner<'a, 'b>(&'a self) -> &'b CLS
    where
        'a: 'b,
    {
        &self.inner
    }
}

impl<'a, CLS> ClassDef for Element<CLS>
where
    CLS: 'static + Type,
{
    const CLASS_NAME: &'static str = std::any::type_name::<CLS>();

    unsafe fn class_id() -> &'static mut ClassId {
        &mut <CLS as Type>::metadata().el_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        let cell = OnceCell::new();
        proto.set(
            "castAs",
            rquickjs::Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: JsClass<'js, Element<CLS>>, ctx: Ctx<'js>, target: String| {
                        let map = cell.get_or_init(|| {
                            let mut map: ElementCasterMap<CLS> = HashMap::new();
                            map.insert(CLS::CLASS_NAME, Box::new(|ctx, value| value.into_js(ctx)));
                            {
                                build_class!(CLS with InitElementCaster::<'_, CLS>::new(&mut map))
                            };
                            Result::Ok(map)
                        });
                        let map = match map {
                            Ok(map) => Ok(map),
                            Err(err) => Err(err.clone()),
                        }?;
                        match map.get(target.as_str()) {
                            Some(func) => (*func)(ctx, this),
                            None => Err(Error::new(
                                ctx,
                                format!("No cast function from {} to {}", CLS::CLASS_NAME, target),
                            )),
                        }
                    },
                ),
            ),
        )?;
        build_class!(CLS with InitElementProto::<CLS>::new(ctx, proto));
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

impl<'js, CLS> rquickjs::IntoJs<'js> for Element<CLS>
where
    CLS: 'static + Type,
{
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js, CLS> rquickjs::FromJs<'js> for Element<CLS>
where
    CLS: 'static + Type,
    Self: Clone,
{
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let mut_ref = <Element<CLS> as rquickjs::ClassDef>::from_js_mut(ctx, value.clone());
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
