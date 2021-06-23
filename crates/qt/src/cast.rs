use crate::inner::*;

pub trait Cast<T> {
    type Output<'a, U: 'a>: AsRef<U>
    where
        Self: 'a,
        T: 'a;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
    where
        T: 'a;
}

default impl<T, V> Cast<T> for V {
    type Output<'a, U: 'a>
    where
        V: 'a,
        T: 'a,
    = DefaultCast<'a, U>;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
    where
        T: 'a,
    {
        None
    }
}

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
