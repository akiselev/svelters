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

impl<const CLS: &'static str> AsRef<ClsTy<QT, CLS>> for QtWrapper<CLS>
where
    QT: FrameworkClass<CLS>,
    ClsTy<QT, CLS>: StaticUpcast<QObject> + CppDeletable,
{
    fn as_ref(&self) -> &ClsTy<QT, CLS> {
        match self {
            QtWrapper::Owned(val) => &*val,
            QtWrapper::Shared(val) => &*val,
            QtWrapper::Ref(val) => &*val,
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
