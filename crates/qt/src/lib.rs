#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(min_type_alias_impl_trait)]
//
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
// #![feature(const_str_from_utf8_unchecked)]
#![feature(const_generics)]
#![feature(const_fn_trait_bound)]
#![feature(const_type_name)]
#![feature(specialization)]
#![feature(impl_trait_in_bindings)]
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
pub(in crate) mod obj;

pub mod js;
pub mod prelude;

use crate::inner::*;

pub trait Container<F: Framework, T>: Sized {
    type Output<X>: AsRef<T>;

    fn wrap(self) -> Self::Output<Self>;
}

pub trait Framework: Copy + Clone + 'static {}

pub trait InFramework<const T: &'static str> {
    type Definition: TypeDef;

    fn def() -> Self::Definition;
}
