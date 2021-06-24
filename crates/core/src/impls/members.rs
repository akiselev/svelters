use frunk::{hlist, Hlist};

use crate::inner::*;

impl<T, U, V> Member<T> for HCons<U, V>
where
    U: Member<T>,
    V: Member<T>,
{
}

impl<T> Member<T> for HNil {}
