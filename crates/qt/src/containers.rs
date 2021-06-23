use std::borrow::Cow;
use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::{Add, Deref};

use frunk::hlist::Selector;
use frunk::{HCons, HNil};
use qt_core::cpp_core::{CastFrom, DynamicCast, Ptr, StaticUpcast};
use qt_core::{QObject, QPtr};
use rquickjs::{Ctx, Error, FromJs, IntoJs, Result, Value};

pub trait Container {
    type Output<T>: Deref<Target = T>;
}

pub struct DefaultCast<'a, T: 'a>(pub(in crate) PhantomData<&'a T>);
impl<'a, T: 'a> AsRef<T> for DefaultCast<'a, T> {
    fn as_ref(&self) -> &T {
        panic!("This shouldn't have happened")
    }
}

pub struct Stack<'a, T: 'a>(pub(in crate) T, pub(in crate) PhantomData<&'a ()>);
impl<'a, T: 'a> AsRef<T> for Stack<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

pub struct PtrRef<'a, T: 'a>(pub(in crate) T, pub(in crate) PhantomData<Cell<&'a ()>>);
impl<'a, T: 'a> AsRef<T> for PtrRef<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<'a, T, U> CastFrom<PtrRef<'a, Ptr<U>>> for Ptr<T>
where
    U: StaticUpcast<T>,
{
    unsafe fn cast_from(value: PtrRef<'a, Ptr<U>>) -> Self {
        StaticUpcast::static_upcast(value.0)
    }
}
