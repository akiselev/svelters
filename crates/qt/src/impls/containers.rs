use qt_core::cpp_core::StaticDowncast;
use svelters_core::prelude::*;

use crate::inner::*;

pub enum QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    Owned(QBox<CLS>),
    Shared(QPtr<CLS>),
    Ref(Ptr<CLS>),
}

pub struct DerefWrapper<'a, T>(Ptr<T>, PhantomData<&'a ()>);

impl<'a, T> Deref for DerefWrapper<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<X, CLS> Cast<X> for QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
    QObject: StaticDowncast<X>,
{
    type Output<'a> = DerefWrapper<'a, X>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        unsafe {
            let obj: Ptr<QObject> = StaticUpcast::static_upcast(self.get_ptr());
            let ptr = StaticDowncast::static_downcast(obj);
            DerefWrapper(ptr, PhantomData)
        }
    }
}

impl<'a, CLS> GetRef<'a, CLS> for QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    type Pointer = Ptr<CLS>;

    fn get_ref(&'a self) -> &'a CLS {
        match self {
            QtWrapper::Owned(ref val) => val.deref(),
            QtWrapper::Shared(ref val) => val.deref(),
            QtWrapper::Ref(ref val) => val.deref(),
        }
    }

    fn get_ptr(&'a self) -> Self::Pointer {
        match self {
            QtWrapper::Owned(ref val) => unsafe { val.as_ptr() },
            QtWrapper::Shared(ref val) => unsafe { val.as_ptr() },
            QtWrapper::Ref(ref val) => val.clone(),
        }
    }
}

impl<CLS> IntoContainer<QT, CLS> for QBox<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Owned(self)
    }
}

impl<CLS> IntoContainer<QT, CLS> for QPtr<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Shared(self)
    }
}

impl<CLS> IntoContainer<QT, CLS> for Ptr<CLS>
where
    QT: FrameworkClass<CLS>,
    CLS: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Ref(self)
    }
}
