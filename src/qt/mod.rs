pub struct QRef<'a, T> {
    ptr: T,
    _marker: std::marker::PhantomData<&'a T>,
}

impl<'a, T> QRef<'a, T> {
    pub unsafe fn new<'inner>(ptr: T) -> QRef<'inner, T> {
        QRef {
            ptr,
            _marker: std::marker::PhantomData,
        }
    }
}
