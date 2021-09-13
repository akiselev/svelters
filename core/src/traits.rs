use std::{backtrace::Backtrace, future::Future};

use rquickjs::{Loader, Resolver};

use crate::inner::*;

/// Cast Self to type &T
pub trait Upcast<T> {
    fn upcast(&self) -> T;
}

pub trait UpcastRef<T> {
    fn upcast_ref(&self) -> &T;
}
