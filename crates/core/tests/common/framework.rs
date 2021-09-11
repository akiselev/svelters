use std::future::Future;

use super::*;

impl<T> Default for Fw<T>
where
    T: Default,
{
    fn default() -> Self {
        Fw(Default::default())
    }
}
