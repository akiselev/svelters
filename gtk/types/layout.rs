use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Box(gtk4::Box);

impl Box {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<gtk4::Box> + Cast,
    {
        Box(obj.upcast::<gtk4::Box>())
    }
}

impl std::ops::Deref for Box {
    type Target = gtk4::Box;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClassDef for Box {
    const CLASS_NAME: &'static str = "Box";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    fn init_proto<'js>(
        _ctx: rquickjs::Ctx<'js>,
        _proto: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        Ok(())
    }
}

impl<T> From<T> for Box
where
    T: IsA<gtk4::Box> + Cast,
{
    fn from(val: T) -> Self {
        Box::new(val)
    }
}

impl<'js> IntoJs<'js> for Box {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Box {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Box::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Box {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Box::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Box {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Box::from_js_obj(ctx, value)
    }
}
