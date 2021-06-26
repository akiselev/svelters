use qt_core::cpp_core::StaticDowncast;
use svelters_core::prelude::*;

use crate::inner::*;

pub enum QtWrapper<const CLS: &'static str>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    Owned(QBox<ClsTy<QT, CLS>>),
    Shared(QPtr<ClsTy<QT, CLS>>),
    Ref(Ptr<ClsTy<QT, CLS>>),
}

pub struct DerefWrapper<'a, T>(Ptr<T>, PhantomData<&'a ()>);

impl<'a, T> Deref for DerefWrapper<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<X, const CLS: &'static str> Cast<X> for QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
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

impl<'a, const CLS: &'static str> GetRef<'a, ClsTy<QT, CLS>> for QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    type Pointer = Ptr<ClsTy<QT, CLS>>;

    fn get_ref(&'a self) -> &'a ClsTy<QT, CLS> {
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

impl<const CLS: &'static str> IntoContainer<QT, CLS> for QBox<ClsTy<QT, CLS>>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Owned(self)
    }
}

impl<const CLS: &'static str> IntoContainer<QT, CLS> for QPtr<ClsTy<QT, CLS>>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Shared(self)
    }
}

impl<const CLS: &'static str> IntoContainer<QT, CLS> for Ptr<ClsTy<QT, CLS>>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    type Type = QtWrapper<CLS>;

    fn into_framework_type(self) -> Self::Type {
        QtWrapper::Ref(self)
    }
}
