use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::Application as GtkApplication;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Application(GtkApplication);

impl Application {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GtkApplication> + Cast,
    {
        Application(obj.upcast::<GtkApplication>())
    }
}

impl std::ops::Deref for Application {
    type Target = GtkApplication;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClassDef for Application {
    const CLASS_NAME: &'static str = "GtkApplication";

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

impl<T> From<T> for Application
where
    T: IsA<GtkApplication> + Cast,
{
    fn from(val: T) -> Self {
        Application::new(val)
    }
}

impl<'js> IntoJs<'js> for Application {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Application {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Application::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Application {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Application::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Application {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Application::from_js_obj(ctx, value)
    }
}
