use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Clone)]
pub struct Object(QPtr<QObject>);

impl Object {
    pub fn new<T>(obj: T) -> Self
    where
        Ptr<QObject>: CastFrom<T>,
    {
        unsafe { Object(QPtr::new(obj)) }
    }

    pub fn inner(self) -> QPtr<QObject> {
        self.0
    }

    pub fn cast<T>(&self) -> Option<QPtr<T>>
    where
        QObject: DynamicCast<T>,
        T: StaticUpcast<QObject>,
    {
        unsafe {
            let ptr = self.dynamic_cast::<T>();
            if ptr.is_null() {
                return None;
            }
            Some(ptr)
        }
    }
}

impl std::ops::Deref for Object {
    type Target = QPtr<QObject>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for Object
where
    Ptr<QObject>: CastFrom<T>,
{
    fn from(val: T) -> Self {
        Object::new(val)
    }
}

impl<'js> IntoJs<'js> for Object {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js_ref(ctx, value)?;
        if unsafe { obj.0.is_null() } {
            return Err(exception!("QObject has been destroyed"));
        }
        Ok(obj)
    }
}

impl<'js> FromJs<'js> for &'js mut Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js_mut(ctx, value)?;
        if unsafe { obj.0.is_null() } {
            return Err(exception!("QObject has been destroyed"));
        }
        Ok(obj)
    }
}

impl<'js> FromJs<'js> for Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js_obj(ctx, value)?;
        if unsafe { obj.0.is_null() } {
            return Err(exception!("QObject has been destroyed"));
        }
        Ok(obj)
    }
}

impl rquickjs::ClassDef for Object {
    const CLASS_NAME: &'static str = "QObject";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }
    const HAS_PROTO: bool = true;
}
