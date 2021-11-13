use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Object(GObject);

impl<'js, T> Interface<'js, T> for Object
where
    T: 'static + AsObject,
    &'js T: FromJs<'js>,
{
    fn define(ty: &Type<'js, T>) -> Result<()> {
        ty.custom_prop(
            "asGObject",
            rquickjs::Accessor::new_get(rquickjs::Method(as_object_getter::<'js, T>())),
        )?;
        Ok(())
    }
}

fn as_object_getter<'js, T>() -> impl Fn(&T) -> Object
where
    T: 'static + AsObject,
    &'js T: FromJs<'js>,
{
    return |this: &T| Object(this.as_object().clone());
}

impl rquickjs::ClassDef for Object {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    fn init_proto<'js>(
        ctx: rquickjs::Ctx<'js>,
        proto: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        let ty = Type::<'js, Object>::new(ctx, proto.clone());
        ty.implements::<Self>()?;
        Ok(())
    }
}

impl Object {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GObject> + Cast,
    {
        Object(obj.upcast::<GObject>())
    }
}

impl std::ops::Deref for Object {
    type Target = GObject;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for Object
where
    T: IsA<GObject> + Cast,
{
    fn from(val: T) -> Self {
        Object::new(val)
    }
}

impl<'js> IntoJs<'js> for Object {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_obj(ctx, value)
    }
}

impl AsObject for Object {
    fn as_object(&self) -> &GObject {
        &self.0
    }
}
