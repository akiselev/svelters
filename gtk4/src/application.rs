use glib::Object as GObject;
use gtk4::{Application, ApplicationWindow, Window};
use query_interface::{interfaces, vtable_for, ObjectClone, ObjectPartialEq};
use rquickjs::{Class, ClassDef, ClassId, Ctx, FromJs, IntoJs, Object, Value};
use smallbox::smallbox;

use svelters_core::prelude::*;

use crate::Gtk;

pub fn application_window(builder: TypeBuilder) -> ElementDef {
    ElementDef::new(builder, "applicationWindow", |ctx| {
        let app: &Gtk<Application> = ctx.globals().get("application")?;
        Ok(smallbox!(Gtk(ApplicationWindow::new(&app.0))))
    })
    .build()
}

interfaces!(Gtk<GObject>: Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ApplicationWindow>: Upcast<Window>, Upcast<GObject>, ObjectClone, ObjectPartialEq);

mod js_impl {
    use super::*;
    impl ClassDef for Gtk<Application> {
        const CLASS_NAME: &'static str = "Gtk<Application>";

        unsafe fn class_id() -> &'static mut ClassId {
            static mut CLASS_ID: ClassId = ClassId::new();
            &mut CLASS_ID
        }

        const HAS_PROTO: bool = false;

        const HAS_STATIC: bool = false;

        const HAS_REFS: bool = false;

        fn into_js_obj<'ctx>(self, ctx: Ctx<'ctx>) -> Result<Value<'ctx>>
        where
            Self: Sized,
        {
            Class::<Self>::instance(ctx, self).map(|cls| cls.into_value())
        }

        fn from_js_ref<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            Class::<Self>::try_ref(ctx, &value)
        }

        fn from_js_mut<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx mut Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            Class::<Self>::try_mut(ctx, &value)
        }

        fn from_js_obj<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<Self>
        where
            Self: Clone + Sized,
        {
            let value = Object::from_js(ctx, value)?;
            let instance = Class::<Self>::try_ref(ctx, &value)?;
            Ok(instance.clone())
        }
    }

    impl<'js> IntoJs<'js> for Gtk<Application> {
        fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
            <Self as ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'js> FromJs<'js> for Gtk<Application> {
        fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
            <Gtk<Application> as ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'js> FromJs<'js> for &'js Gtk<Application> {
        fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
            <Gtk<Application> as ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'js> FromJs<'js> for &'js mut Gtk<Application> {
        fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
            <Gtk<Application> as ClassDef>::from_js_mut(ctx, value)
        }
    }
}
