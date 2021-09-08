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

pub(in crate) type ObjMap = SlotMap<Idx, Instance>;

#[derive(Clone)]
pub struct Runtime {
    pub(in crate) objects: Rc<RefCell<ObjMap>>,
    pub(in crate) inner: Rc<RefCell<RT>>,
}

pub struct WeakRt {
    pub(in crate) objects: Weak<RefCell<ObjMap>>,
    pub(in crate) inner: Weak<RefCell<RT>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            objects: Rc::new(RefCell::new(Default::default())),
            inner: Rc::new(RefCell::new(RT {
                types: Default::default(),
                elements: Default::default(),
            })),
        }
    }

    pub(in crate) fn weak(&self) -> WeakRt {
        WeakRt {
            objects: Rc::downgrade(&self.objects),
            inner: Rc::downgrade(&self.inner),
        }
    }

    pub(in crate) fn new_instance<CLS: Type>(&self, cls: CLS) -> Idx {
        self.objects.deref().borrow_mut().insert(Instance::new(cls))
    }

    pub(in crate) fn get_instance<CLS: Type>(&self, idx: Idx) -> Option<ObjRef<CLS>> {
        Obj::<CLS>::from_rt(&self, idx)
    }

    pub(in crate) fn register_type<'js, CLS: Type>(&self, ctx: Ctx<'js>) -> TypeKey {
        let mut typedef = TypeDef::new::<CLS>();
        let mut child = InitType::<CLS>::new(&mut typedef);
        let mut cls: TypeBuilder<CLS, _> = TypeBuilder::new(child);
        <CLS as Type>::define(cls).unwrap();

        let mut child = InitCaster::<CLS>::new(&mut typedef.casters);
        let mut cls: TypeBuilder<CLS, _> = TypeBuilder::new(child);
        <CLS as Type>::define(cls).unwrap();
        self.inner.deref().borrow_mut().types.insert(typedef)
    }

    delegate::delegate! {
        to self.inner {
            fn borrow(&self) -> Ref<RT>;
        }
    }
}

impl WeakRt {
    pub(in crate) fn upgrade(&self) -> Option<Runtime> {
        Some(Runtime {
            objects: self.objects.upgrade()?,
            inner: self.inner.upgrade()?,
        })
    }
}

pub(in crate) struct RT {
    pub(in crate) types: SlotMap<TypeKey, TypeDef>,
    pub(in crate) elements:
        HashMap<&'static str, SmallBox<dyn for<'js> Fn(Ctx<'js>) -> Result<Value<'js>>, space::S8>>,
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
new_key_type! { pub(in crate) struct TypeKey; }

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
