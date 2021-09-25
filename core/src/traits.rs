use std::{backtrace::Backtrace, future::Future};

use query_interface::{mopo, HasInterface, Object as DynObject, ObjectClone, ObjectPartialEq};
use rquickjs::{Loader, Resolver};

use crate::inner::*;

/// Cast Self to type &T
pub trait Upcast<T> {
    fn upcast(&self) -> T;
}

pub trait UpcastRef<T> {
    fn upcast_ref(&self) -> &T;
}

pub trait IElement:
    DynObject
    + ObjectClone
    + HasInterface<dyn ObjectClone>
    + ObjectPartialEq
    + HasInterface<dyn ObjectPartialEq>
{
}
mopo!(dyn IElement);

impl<T> IElement for T where
    T: DynObject
        + ObjectClone
        + HasInterface<dyn ObjectClone>
        + ObjectPartialEq
        + HasInterface<dyn ObjectPartialEq>
{
}
