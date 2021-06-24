use crate::inner::*;

#[derive(Debug, Clone, Copy)]
pub struct AsRefWrapper<T>(T);

impl<T> std::ops::Deref for AsRefWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: CppDeletable> AsRef<T> for AsRefWrapper<CppBox<T>> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T: CppDeletable + StaticUpcast<QObject>> AsRef<T> for AsRefWrapper<QBox<T>> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T> Container<QT, T> for CppBox<T>
where
    T: CppDeletable,
    AsRefWrapper<CppBox<T>>: AsRef<T>,
{
    type Output<X> = AsRefWrapper<CppBox<T>>;

    fn wrap(self) -> Self::Output<Self> {
        AsRefWrapper(self)
    }
}

impl<T> Container<QT, T> for QBox<T>
where
    T: CppDeletable + StaticUpcast<QObject>,
{
    type Output<X> = AsRefWrapper<QBox<T>>;

    fn wrap(self) -> Self::Output<Self> {
        AsRefWrapper(self)
    }
}

pub struct Stack<'a, T: 'a>(pub(in crate) T, pub(in crate) PhantomData<&'a ()>);
impl<'a, T: 'a> AsRef<T> for Stack<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

pub struct PtrRef<'a, T: 'a>(pub(in crate) T, pub(in crate) PhantomData<Cell<&'a ()>>);
impl<'a, T: 'a> AsRef<T> for PtrRef<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<'a, T, U> CastFrom<PtrRef<'a, Ptr<U>>> for Ptr<T>
where
    U: StaticUpcast<T>,
{
    unsafe fn cast_from(value: PtrRef<'a, Ptr<U>>) -> Self {
        StaticUpcast::static_upcast(value.0)
    }
}
