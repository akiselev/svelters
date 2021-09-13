use std::{
    any::{Any, TypeId},
    borrow::BorrowMut,
    cell::Ref,
    marker::Unsize,
    mem::{align_of, size_of},
    rc::Weak,
};

use smallbox::{smallbox, SmallBox};

use crate::inner::*;

pub type Rt = Runtime;

pub struct SvRuntime {
    ctx: Context,
    pub(in crate) elements: HashMap<&'static str, ()>,
}

#[derive(Clone)]
pub struct Runtime {
    pub(in crate) inner: Inner<SvRuntime>,
}

impl Runtime {
    pub fn new(ctx: Context) -> Self {
        Self {
            inner: Inner::new(SvRuntime {
                ctx,
                elements: Default::default(),
            }),
        }
    }

    pub(in crate) fn weak(&self) -> WeakRuntime {
        WeakRuntime {
            inner: self.inner.downgrade(),
        }
    }
}

#[derive(Clone)]
pub struct WeakRuntime {
    pub(in crate) inner: WeakInner<SvRuntime>,
}

impl WeakRuntime {
    pub(in crate) fn upgrade(&self) -> Option<Runtime> {
        Some(Runtime {
            inner: self.inner.upgrade()?,
        })
    }
}

mod js_impl {
    use super::*;

    impl ClassDef for Runtime {
        const CLASS_NAME: &'static str = "Runtime";

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
            JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
        }

        fn from_js_ref<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_ref(ctx, &value)
        }

        fn from_js_mut<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx mut Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_mut(ctx, &value)
        }

        fn from_js_obj<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<Self>
        where
            Self: Clone + Sized,
        {
            let value = Object::from_js(ctx, value)?;
            let instance = JsClass::<Self>::try_ref(ctx, &value)?;
            Ok(instance.clone())
        }
    }

    impl<'js> rquickjs::IntoJs<'js> for Runtime {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for Runtime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <Runtime as rquickjs::ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js Runtime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <Runtime as rquickjs::ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js mut Runtime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <Runtime as rquickjs::ClassDef>::from_js_mut(ctx, value)
        }
    }

    impl ClassDef for WeakRuntime {
        const CLASS_NAME: &'static str = "RuntimeWeakRef";

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
            JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
        }

        fn from_js_ref<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_ref(ctx, &value)
        }

        fn from_js_mut<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx mut Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_mut(ctx, &value)
        }

        fn from_js_obj<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<Self>
        where
            Self: Clone + Sized,
        {
            let value = Object::from_js(ctx, value)?;
            let instance = JsClass::<Self>::try_ref(ctx, &value)?;
            Ok(instance.clone())
        }
    }

    impl<'js> rquickjs::IntoJs<'js> for WeakRuntime {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for WeakRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakRuntime as rquickjs::ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js WeakRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakRuntime as rquickjs::ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js mut WeakRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakRuntime as rquickjs::ClassDef>::from_js_mut(ctx, value)
        }
    }
}
