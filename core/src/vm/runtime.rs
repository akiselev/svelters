use std::{
    any::{Any, TypeId},
    borrow::BorrowMut,
    cell::Ref,
    marker::Unsize,
    mem::{align_of, size_of},
    path::Path,
    rc::Weak,
};

use rquickjs::{Function as JsFunction, IntoAtom};
use smallbox::{smallbox, SmallBox};

use crate::{init::cleanup_view_model_context, inner::*};

#[derive(Clone)]
pub struct ViewModelRuntime {
    context: Context,
    pub(in crate) inner: Inner<VMRuntime>,
}

pub struct VMRuntime {
    pub(in crate) interfaces: HashMap<Cow<'static, str>, InterfaceDefFn>,
    pub(in crate) element_fns: HashMap<Cow<'static, str>, ElementDefFn>,
    pub(in crate) elements: HashMap<Cow<'static, str>, Rc<ElementDef>>,
}

impl ViewModelRuntime {
    pub fn new(context: Context) -> Self {
        Self {
            context,
            inner: Inner::new(VMRuntime {
                interfaces: Default::default(),
                element_fns: Default::default(),
                elements: Default::default(),
            }),
        }
    }

    pub(in crate) fn with_ctx<F: FnOnce(&Context) -> R, R>(&self, func: F) -> R {
        func(&self.context)
    }

    pub(in crate) fn with_js<F: for<'js> FnOnce(Ctx<'js>) -> R, R>(&self, func: F) -> R {
        self.context.with(func)
    }

    pub(in crate) fn with_types<F: for<'js> FnOnce(TypeBuilder<'js>)>(
        &self,
        func: F,
    ) -> Result<()> {
        self.context
            .with(|ctx| Ok(func(TypeBuilder::from_ctx(ctx)?)))
    }

    pub fn set_global<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: for<'js> IntoAtom<'js>,
        V: for<'js> IntoJs<'js>,
    {
        self.context.with(move |ctx| ctx.globals().set(key, value))
    }

    pub fn set_viewmodel<N, S>(&self, path: N, src: S) -> Result<()>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        self.with_js(move |ctx| {
            let module = ctx.compile(path, src)?;
            let component = module.get::<_, JsFunction>("default")?;
            let globals = ctx.globals();

            let doc: &mut ViewModelRuntime = globals.get("document")?;
            let root = doc.create_element(ctx, "root".to_string())?;
            globals.get::<_, Object>("document")?.set("root", root)?;
            globals.set("RootViewModel", component.clone())?;

            let opts = Object::new(ctx)?;
            globals.set("rootViewModelOpts", opts.clone())?;
            opts.set("target", globals.get::<_, Value>("document.root")?);
            let instance = ctx.eval::<Object, _>("new RootViewModel(rootViewModelOpts)")?;
            globals.set("rootViewModel", instance)?;
            Result::Ok(())
        })
    }

    pub(in crate) fn create_event<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _interface: String,
    ) -> Result<Value<'js>> {
        Event::custom(ctx)
    }

    pub(in crate) fn create_element_ns<'js>(
        &mut self,
        ctx: Ctx<'js>,
        _ns: Value<'js>,
        name: String,
        _options: Opt<Object<'js>>,
    ) -> Result<Value<'js>> {
        self.create_element(ctx, name)
    }

    pub(in crate) fn create_element<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
    ) -> Result<Value<'js>> {
        if name == "slot" {
            return SlotNode::new().into_js(ctx);
        }
        let mut inner = self.inner.deref().borrow_mut();
        if let Some(def) = inner.elements.get(name.as_str()).cloned() {
            let container = (def.constructor)(ctx)?;
            ElementNode::new(name.into(), container, def).into_js(ctx)
        } else if let Some(def_fn) = inner.element_fns.get(name.as_str()).cloned() {
            let new_def = Rc::new(def_fn(TypeBuilder::new(inner)));
            self.inner
                .deref()
                .borrow_mut()
                .elements
                .insert(name.clone().into(), new_def.clone());
            let container = (new_def.constructor)(ctx)?;
            ElementNode::new(name.into(), container, new_def).into_js(ctx)
        } else {
            Err(exception!("Element \"{}\" is not defined", name))
        }
    }

    pub(in crate) fn dispatch_event<'js>(
        &mut self,
        _ctx: Ctx<'js>,
        _event: Object<'js>,
        _options: Opt<Object<'js>>,
    ) -> Result<()> {
        Ok(())
    }

    pub(in crate) fn weak(&self) -> WeakVMRuntime {
        WeakVMRuntime {
            ctx: self.context.clone(),
            inner: self.inner.downgrade(),
        }
    }
}

impl Drop for ViewModelRuntime {
    fn drop(&mut self) {
        tracing::warn!("Droping viewmodel runtime");
        // cleanup_view_model_context(&self.context).ok();
        // .expect("Failed to clean up viewmodel runtime");
    }
}

#[derive(Clone)]
pub struct WeakVMRuntime {
    ctx: Context,
    pub(in crate) inner: WeakInner<VMRuntime>,
}

impl WeakVMRuntime {
    pub(in crate) fn upgrade(&self) -> Option<ViewModelRuntime> {
        Some(ViewModelRuntime {
            context: self.ctx.clone(),
            inner: self.inner.upgrade()?,
        })
    }
}

mod js_impl {
    use super::*;

    impl ClassDef for ViewModelRuntime {
        const CLASS_NAME: &'static str = "Runtime";

        unsafe fn class_id() -> &'static mut ClassId {
            static mut CLASS_ID: ClassId = ClassId::new();
            &mut CLASS_ID
        }

        const HAS_PROTO: bool = true;

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

        fn init_proto<'js>(_ctx: Ctx<'js>, proto: &Object<'js>) -> rquickjs::Result<()> {
            proto.set(
                "createEvent",
                rquickjs::Func::new(
                    "createEvent",
                    rquickjs::Method(ViewModelRuntime::create_event),
                ),
            )?;
            proto.set(
                "createElement",
                rquickjs::Func::new(
                    "createElement",
                    rquickjs::Method(ViewModelRuntime::create_element),
                ),
            )?;
            proto.set(
                "createElementNS",
                rquickjs::Func::new(
                    "createElementNS",
                    rquickjs::Method(ViewModelRuntime::create_element_ns),
                ),
            )?;
            proto.set(
                "dispatchEvent",
                rquickjs::Func::new(
                    "dispatchEvent",
                    rquickjs::Method(ViewModelRuntime::dispatch_event),
                ),
            )?;
            Ok(())
        }

        fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> rquickjs::Result<()> {
            Ok(())
        }

        fn mark_refs(&self, _marker: &RefsMarker) {}
    }

    impl<'js> rquickjs::IntoJs<'js> for ViewModelRuntime {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for ViewModelRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ViewModelRuntime as rquickjs::ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js ViewModelRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ViewModelRuntime as rquickjs::ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js mut ViewModelRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ViewModelRuntime as rquickjs::ClassDef>::from_js_mut(ctx, value)
        }
    }

    impl ClassDef for WeakVMRuntime {
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

    impl<'js> rquickjs::IntoJs<'js> for WeakVMRuntime {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for WeakVMRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakVMRuntime as rquickjs::ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js WeakVMRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakVMRuntime as rquickjs::ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'js> rquickjs::FromJs<'js> for &'js mut WeakVMRuntime {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <WeakVMRuntime as rquickjs::ClassDef>::from_js_mut(ctx, value)
        }
    }
}
