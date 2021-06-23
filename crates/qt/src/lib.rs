#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(min_type_alias_impl_trait)]
//
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
#![feature(const_str_from_utf8_unchecked)]
#![feature(const_fn_trait_bound)]
#![feature(specialization)]
#![allow(incomplete_features)]
#![recursion_limit = "1024"]
#![allow(unused)]

use std::borrow::Cow;
use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::{Add, Deref};

use frunk::hlist::Selector;
use frunk::{HCons, HNil};
use qt_core::cpp_core::{DynamicCast, Ptr, StaticUpcast};
use qt_core::{QObject, QPtr};
use rquickjs::{Ctx, Error, FromJs, IntoJs, Result, Value};

#[macro_export]
mod macros;

mod builder_types;
mod class;
mod containers;
mod core;
mod func;
mod node;
pub use crate::builder_types::*;
pub use crate::class::*;
pub use crate::containers::*;
pub use crate::core::*;
pub use crate::func::*;
pub use crate::node::*;

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

impl<T, Q> Cast<T> for QPtr<Q>
where
    QObject: DynamicCast<T>,
    Q: StaticUpcast<QObject>,
{
    type Output<'a, U: 'a>
    where
        Self: 'a,
        T: 'a,
    = PtrRef<'a, U>;

    fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
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

// impl<T> Cast<T> for QPtr<QObject>
// where
//     QObject: DynamicCast<T>,
// {
//     type Output<'a, U: 'a>
//     where
//         Self: 'a,
//         T: 'a,
//     = PtrRef<'a, U>;

//     fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
//     where
//         T: 'a,
//     {
//         let ptr: Ptr<T> = unsafe { DynamicCast::dynamic_cast(self.as_ptr()) };
//         if !ptr.is_null() {
//             Some(PtrRef(ptr, PhantomData))
//         } else {
//             None
//         }
//     }
// }

// pub trait Upcaster {
//     type Output<'a, U>: AsRef<T>;

//     fn upcast<'a, V>(&'a self) -> Option<Self::Output<'a, V>>;
// }

pub trait Framework: Copy + Clone + 'static {
    type Container: Container;
}
