use rquickjs::{Ctx, FromJs, IntoJs, Value};

use crate::common::*;

#[derive(Debug, Clone)]
pub struct Qt<T: StaticUpcast<QObject>>(pub QPtr<T>);

impl<T> Qt<T>
where
    T: StaticUpcast<QObject>,
{
    pub fn new(val: T) -> Self
    where
        Ptr<T>: CastFrom<T>,
    {
        let ptr = unsafe { QPtr::new(val) };
        if unsafe { ptr.is_null() } {
            panic!("QPtr<QObject> is null");
        }
        Qt(ptr)
    }

    pub fn inner(self) -> QPtr<T> {
        self.0
    }
}

impl<'js, T> IntoJs<'js> for Qt<T>
where
    Ptr<QObject>: CastFrom<T>,
    T: StaticUpcast<QObject>,
{
    fn into_js(self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        ctx.marshall(unsafe { self.0.as_ptr() })
    }
}

impl<'js, T> FromJs<'js> for Qt<T>
where
    QObject: DynamicCast<T>,
    // Ptr<QObject>: CastFrom<T>,
    T: StaticUpcast<QObject>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let obj = crate::Object::from_js(ctx, value)?;
        let ptr = obj
            .cast::<T>()
            .ok_or_else(|| exception!("Could not cast"))?;
        Ok(Qt(ptr))
    }
}

impl<T> std::ops::Deref for Qt<T>
where
    T: StaticUpcast<QObject>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for Qt<T>
where
    T: StaticUpcast<QObject>,
{
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for Qt<T>
where
    T: StaticUpcast<QObject>,
    Ptr<T>: CastFrom<T>,
{
    fn from(val: T) -> Self {
        let ptr = unsafe { QPtr::new(val) };
        if unsafe { ptr.is_null() } {
            panic!("QPtr<QObject> is null");
        }
        Qt(ptr)
    }
}

pub struct QBox<T>(pub CppBox<T>)
where
    T: cpp_core::CppDeletable;

impl<T> std::ops::Deref for QBox<T>
where
    T: cpp_core::CppDeletable,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0.deref()
    }
}
