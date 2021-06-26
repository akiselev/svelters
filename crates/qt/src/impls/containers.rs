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
