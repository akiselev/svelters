use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Button(gtk4::Button);

impl Button {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<gtk4::Button> + Cast,
    {
        Button(obj.upcast::<gtk4::Button>())
    }
}

impl std::ops::Deref for Button {
    type Target = gtk4::Button;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClassDef for Button {
    const CLASS_NAME: &'static str = "Button";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = false;

    const HAS_REFS: bool = false;

    fn init_proto<'js>(
        _ctx: rquickjs::Ctx<'js>,
        _proto: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        Ok(())
    }

    fn init_static<'js>(
        _ctx: rquickjs::Ctx<'js>,
        _static: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        Ok(())
    }
}

impl<T> From<T> for Button
where
    T: IsA<gtk4::Button> + Cast,
{
    fn from(val: T) -> Self {
        Button::new(val)
    }
}

impl<'js> IntoJs<'js> for Button {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Button {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Button::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Button {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Button::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Button {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Button::from_js_obj(ctx, value)
    }
}
