use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::ClassDef;

use super::common::*;

#[derive(Clone)]
pub struct Object(GObject);

impl Object {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GObject> + Cast,
    {
        Object(obj.upcast::<GObject>())
    }
}

impl std::ops::Deref for Object {
    type Target = GObject;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClassDef for Object {
    const CLASS_NAME: &'static str = "GObject";

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

trait Property {
    type Ref;

    fn name(&self) -> &str;

    fn spec(&self, obj: &Self::Ref) -> &ParamSpec;

    // fn set(&self, obj: &Self::Ref) -> &ParamSpec;
}

pub struct GParam<T> {
    param_name: Cow<'static, str>,
    spec: UnsafeCell<Option<ParamSpec>>,
    marker: PhantomData<T>,
}

impl<T> GParam<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            param_name: name.into(),
            spec: UnsafeCell::new(None),
            marker: PhantomData,
        }
    }
}

impl<T> Property for GParam<T>
where
    T: ObjectExt,
{
    type Ref = T;

    fn name(&self) -> &str {
        self.param_name.as_ref()
    }

    fn spec(&self, obj: &T) -> &ParamSpec {
        unsafe {
            let spec: &mut Option<ParamSpec> = &mut *self.spec.get();
            if let Some(spec) = spec {
                &*spec
            } else {
                *spec = obj.find_property(self.param_name.as_ref());
                &*spec.as_ref().expect("Param not found")
            }
        }
    }
}
