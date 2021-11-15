use std::{borrow::Cow, cell::UnsafeCell, collections::HashMap, marker::PhantomData};

use glib::{value::FromValue, ObjectType, SignalHandlerId};
use rquickjs::{qjs, ClassDef, Context, Ctx, FromJs, Function, IntoJs, Method, MutFn, Opt, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Object(GObject);

impl<U> CastRef<U> for Object
where
    U: ObjectType,
{
    fn cast_ref(&self) -> Option<&U> {
        self.0.dynamic_cast_ref::<U>()
    }
}

impl rquickjs::ClassDef for Object {
    const CLASS_NAME: &'static str = "GObject";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }
    const HAS_PROTO: bool = true;
}

impl Object {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GObject> + Cast,
    {
        Object(obj.upcast::<GObject>())
    }

    pub fn inner(self) -> GObject {
        self.0
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
