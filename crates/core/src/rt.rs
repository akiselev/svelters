use std::{
    any::{Any, TypeId},
    marker::Unsize,
    mem::{align_of, size_of},
    rc::Weak,
};

use smallbox::{smallbox, SmallBox};

use crate::inner::*;

pub type Rt = Runtime;

#[derive(Clone)]
pub struct Runtime {
    pub(in crate) inner: Rc<RefCell<RT>>,
}

pub struct WeakRt {
    pub(in crate) inner: Weak<RefCell<RT>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(RT {
                objects: Default::default(),
            })),
        }
    }

    pub(in crate) fn weak(&self) -> WeakRt {
        WeakRt {
            inner: Rc::downgrade(&self.inner),
        }
    }

    pub(in crate) fn new_instance<CLS: Type>(&self, cls: CLS) -> Idx {
        self.inner.borrow_mut().objects.insert(Instance::new(cls))
    }
}

impl WeakRt {
    pub(in crate) fn upgrade(&self) -> Option<Runtime> {
        self.inner.upgrade().map(|inner| Runtime { inner })
    }
}

pub(in crate) struct RT {
    pub(in crate) objects: SlotMap<Idx, Instance>,
}

pub(in crate) struct Instance {
    pub(in crate) metadata: &'static Metadata,
    pub(in crate) data: SmallBox<dyn Any, smallbox::space::S2>,
}

impl Instance {
    pub(in crate) fn new<CLS: Type>(data: CLS) -> Self {
        let typeid = TypeId::of::<CLS>();
        Self {
            metadata: unsafe { CLS::metadata() },
            data: smallbox!(data),
        }
    }
}

pub(in crate) type Idx = InstanceKey;
new_key_type! { pub(in crate) struct InstanceKey; }

impl ClassDef for Runtime {
    const CLASS_NAME: &'static str = "Runtime";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = false;

    const HAS_STATIC: bool = false;

    const HAS_REFS: bool = false;

    fn init_proto<'ctx>(_ctx: Ctx<'ctx>, _proto: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn init_static<'ctx>(_ctx: Ctx<'ctx>, _static: &Object<'ctx>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, marker: &RefsMarker) {
        // HasRefs::mark_refs(self, marker)
    }

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
