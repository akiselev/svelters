impl<T, Q> Cast<Ptr<T>> for QPtr<Q>
where
    QObject: DynamicCast<T>,
    Q: StaticUpcast<QObject>,
{
    type Output<'a, U: 'a>
    where
        Self: 'a,
        T: 'a,
    = PtrRef<'a, U>;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, Ptr<T>>>
    where
        T: 'a,
    {
        let ptr: Ptr<T> = unsafe { DynamicCast::dynamic_cast(self.static_upcast().as_ptr()) };
        if !ptr.is_null() {
            Some(PtrRef(ptr, PhantomData))
        } else {
            None
        }
    }
}

impl<T, Q> Cast<Ptr<T>> for QBox<Q>
where
    QObject: DynamicCast<T>,
    Q: StaticUpcast<QObject>,
    Q: CppDeletable,
{
    type Output<'a, U: 'a>
    where
        Self: 'a,
        T: 'a,
    = PtrRef<'a, U>;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, Ptr<T>>>
    where
        T: 'a,
    {
        let ptr: Ptr<T> = unsafe { DynamicCast::dynamic_cast(self.static_upcast().as_ptr()) };
        if !ptr.is_null() {
            Some(PtrRef(ptr, PhantomData))
        } else {
            None
        }
    }
}
