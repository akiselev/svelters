use qt_core::cpp_core::{CastInto, Ref, StaticDowncast};
use svelters_core::prelude::*;

use crate::inner::*;

pub enum QtWrapper<CLS>
where
    CLS: TypeDef<QT>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    Owned(QBox<CLS>),
    Shared(QPtr<CLS>),
    Ref(Ptr<CLS>),
}

impl<'a, T> Deref for QtWrapper<T>
where
    T: TypeDef<QT>,
    T: StaticUpcast<QObject> + CppDeletable,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            QtWrapper::Owned(ref val) => val.deref(),
            QtWrapper::Shared(ref val) => val.deref(),
            QtWrapper::Ref(ref val) => val.deref(),
        }
    }
}

pub struct DerefWrapper<'a, T>(Ptr<T>, PhantomData<&'a ()>);

impl<'a, T> Deref for DerefWrapper<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T, CLS> CastFrom<T> for QtWrapper<CLS>
where
    T: CastFrom<CLS>,
    CLS: TypeDef<QT>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    unsafe fn cast_from(value: T) -> Self {
        todo!()
    }
}

impl<X, CLS> Cast<X> for QtWrapper<CLS>
where
    CLS: TypeDef<QT>,
    CLS: StaticUpcast<QObject> + CppDeletable,
    Ref<X>: CastFrom<Ref<CLS>>,
    // Ref<CLS>: CastFrom<Ref<X>> + StaticUpcast<Ref<X>> + CppDeletable,
    // Ref<QObject>: CastFrom<Ptr<X>>,
    // X: CastFrom<CLS>,
    // QObject: qt_core::cpp_core::StaticUpcast<X>,
{
    type Output<'a> = DerefWrapper<'a, X>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        unsafe {
            // let obj: Ref<QObject> = CastFrom::cast_from(self.get_ptr());
            // let ptr = CastFrom::cast_from(obj);
            let ptr = unsafe {
                match self {
                    QtWrapper::Owned(ref val) => val.as_ptr(),
                    QtWrapper::Shared(ref val) => val.as_ptr(),
                    QtWrapper::Ref(ref val) => val.clone(),
                }
            };
            let ptr: Ref<X> = CastFrom::cast_from(Ref::new(ptr).unwrap());
            DerefWrapper(ptr.as_ptr(), PhantomData)
        }
    }
}

impl<CLS> IntoRepr<QT, CLS> for QBox<CLS>
where
    CLS: TypeDef<QT> + Repr<QT, Type = QtWrapper<CLS>>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    fn into_framework_type(self) -> QtWrapper<CLS> {
        QtWrapper::Owned(self)
    }
}

impl<CLS> IntoRepr<QT, CLS> for QPtr<CLS>
where
    CLS: TypeDef<QT> + Repr<QT, Type = QtWrapper<CLS>>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    fn into_framework_type(self) -> QtWrapper<CLS> {
        QtWrapper::Shared(self)
    }
}

impl<CLS> IntoRepr<QT, CLS> for Ptr<CLS>
where
    CLS: TypeDef<QT> + Repr<QT, Type = QtWrapper<CLS>>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    fn into_framework_type(self) -> QtWrapper<CLS> {
        QtWrapper::Ref(unsafe { self })
    }
}
