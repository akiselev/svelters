use qt_core::cpp_core::{CastInto, Ref, StaticDowncast};
use svelters_core::prelude::*;

use crate::inner::*;

pub enum Qt<CLS>
where
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    Owned(QBox<CLS>),
    Shared(QPtr<CLS>),
    Ref(Ptr<CLS>),
}

impl<CLS> Clone for Qt<CLS>
where
    Qt<CLS>: Type,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    fn clone(&self) -> Self {
        Qt::Shared(unsafe { QPtr::<CLS>::new(self.as_ptr()) })
    }
}

impl<CLS> Qt<CLS>
where
    Qt<CLS>: Type,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    pub fn new(boxed: QBox<CLS>) -> Self {
        Qt::Owned(boxed)
    }

    pub fn as_ptr(&self) -> Ptr<CLS> {
        unsafe {
            match self {
                Qt::Owned(ref val) => val.as_ptr(),
                Qt::Shared(ref val) => val.as_ptr(),
                Qt::Ref(ref val) => *val,
            }
        }
    }
}

impl<'a, T> Deref for Qt<T>
where
    Qt<T>: Type,
    T: StaticUpcast<QObject> + CppDeletable,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Qt::Owned(ref val) => val.deref(),
            Qt::Shared(ref val) => val.deref(),
            Qt::Ref(ref val) => val.deref(),
        }
    }
}

pub struct DerefWrapper<'a, T>(Qt<T>, PhantomData<&'a ()>)
where
    T: StaticUpcast<QObject> + CppDeletable;

impl<'a, T> AsRef<Qt<T>> for DerefWrapper<'a, T>
where
    T: StaticUpcast<QObject> + CppDeletable,
{
    fn as_ref(&self) -> &Qt<T> {
        &self.0
    }
}

impl<T, CLS> CastFrom<Qt<T>> for Qt<CLS>
where
    T: CastFrom<CLS>,
    Qt<CLS>: Type,
    T: StaticUpcast<QObject> + CppDeletable,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    unsafe fn cast_from(value: Qt<T>) -> Self {
        todo!()
    }
}

impl<X, CLS> Cast<Qt<X>> for Qt<CLS>
where
    Qt<CLS>: Type,
    CLS: StaticUpcast<QObject> + CppDeletable,
    X: StaticUpcast<QObject> + CppDeletable,
    Ref<X>: CastFrom<Ref<CLS>>,
{
    type Output<'a> = DerefWrapper<'a, X>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        unsafe {
            // let obj: Ref<QObject> = CastFrom::cast_from(self.get_ptr());
            // let ptr = CastFrom::cast_from(obj);
            let ptr = unsafe {
                match self {
                    Qt::Owned(ref val) => val.as_ptr(),
                    Qt::Shared(ref val) => val.as_ptr(),
                    Qt::Ref(ref val) => val.clone(),
                }
            };
            let ptr: Ref<X> = CastFrom::cast_from(Ref::new(ptr).unwrap());
            DerefWrapper(Qt::Ref(ptr.as_ptr()), PhantomData)
        }
    }
}
