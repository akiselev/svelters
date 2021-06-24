use frunk::{hlist, Hlist};

use crate::inner::*;

pub trait Member<T> {}

impl<T, U, V> Member<T> for HCons<U, V>
where
    U: Member<T>,
    V: Member<T>,
{
}

impl<T> Member<T> for HNil {}

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

pub trait Instantiate {}

impl<T, D, F> Instantiate for Class<T, D, F>
where
    F: Framework,
    D: Member<T>,
{
}
