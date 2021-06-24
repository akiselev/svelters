use crate::inner::*;

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
