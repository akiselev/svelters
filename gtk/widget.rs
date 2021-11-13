use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Widget(GtkWidget);

impl Widget {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GtkWidget> + Cast,
    {
        Widget(obj.upcast::<GtkWidget>())
    }
}

impl std::ops::Deref for Widget {
    type Target = GtkWidget;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClassDef for Widget {
    const CLASS_NAME: &'static str = "Widget";

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

impl<T> From<T> for Widget
where
    T: IsA<GtkWidget> + Cast,
{
    fn from(val: T) -> Self {
        Widget::new(val)
    }
}

impl<'js> IntoJs<'js> for Widget {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Widget {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Widget::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Widget {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Widget::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Widget {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Widget::from_js_obj(ctx, value)
    }
}

impl AsObject for Widget {
    fn as_object(&self) -> &GObject {
        &self.0.upcast_ref::<GObject>()
    }
}
