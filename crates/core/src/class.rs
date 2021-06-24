use crate::inner::*;

pub struct Class<T, D, F>
where
    D: Member<T>,
    F: Framework,
{
    pub(in crate) name: Cow<'static, str>,
    pub(in crate) _type: PhantomData<T>,
    pub(in crate) list: D,
    pub(in crate) fw: F,
}

unsafe impl<T, D, F> Sync for Class<T, D, F>
where
    F: Framework,
    D: Member<T>,
{
}

impl<T, D, F> Instantiate for Class<T, D, F>
where
    F: Framework,
    D: Member<T>,
{
}

impl<A> Instantiate for RwLock<A> where A: Instantiate {}
