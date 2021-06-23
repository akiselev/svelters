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

#[macro_export]
mod macros;

pub(in crate) mod builder_types;
pub(in crate) mod cast;
pub(in crate) mod class;
pub(in crate) mod containers;
pub(in crate) mod core;
pub(in crate) mod func;
pub(in crate) mod inner;
pub(in crate) mod node;
pub mod prelude;

use crate::inner::*;

pub struct Type<T, F: Framework> {
    name: Cow<'static, str>,
    framework: F,
    marker: PhantomData<T>,
}

pub trait Framework: Copy + Clone + 'static {
    type Container: Container;
}
