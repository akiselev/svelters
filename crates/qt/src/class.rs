use frunk::{hlist, Hlist};

use crate::inner::*;

pub struct Class<T, D, F: Framework> {
    pub(in crate) name: Cow<'static, str>,
    pub(in crate) _type: PhantomData<T>,
    pub(in crate) list: D,
    pub(in crate) fw: F,
}

unsafe impl<T, D, F: Framework> Sync for Class<T, D, F> {}

pub trait Instantiate {}

impl<T, D, F: Framework> Instantiate for Class<T, D, F> {}
