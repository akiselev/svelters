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
// #![feature(const_type_name)]
#![feature(specialization)]
#![feature(min_specialization)]
#![feature(impl_trait_in_bindings)]
#![recursion_limit = "4096"]
#![allow(incomplete_features)]

pub mod prelude {
    pub use crate::class::constructor::*;
    pub use crate::class::JsClass;
    pub use crate::module::{JsModule, JsModuleLoader};
    pub use crate::obj::Obj;
    pub use crate::traits::*;

    pub use rquickjs::{
        Ctx, Error, FromJs, HasRefs, IntoJs, IntoJsByRef, Outlive, Persistent, Result, Value,
    };
}

pub(in crate) mod class;
pub(in crate) mod module;
pub(in crate) mod obj;
pub(in crate) mod phases;
pub(in crate) mod traits;

// ----------------- INTERNAL -----------------

#[allow(unused_imports)]
pub(in crate) mod inner {
    pub(in crate) use std::borrow::Cow;
    pub(in crate) use std::cell::Cell;
    pub(in crate) use std::marker::PhantomData;
    pub(in crate) use std::ops::{Add, Deref, DerefMut};

    pub(in crate) use frunk::{
        hlist::{HList, Selector},
        HCons, HNil,
    };
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use rquickjs::{
        AsFunction, ClassDef, ClassId, Context, Created, Ctx, Error, FromJs, IntoJs, Loaded,
        Module, ModuleDef, Native, Object, RefsMarker, Result, Value,
    };

    pub(in crate) use crate::class::constructor::*;
    pub(in crate) use crate::class::impls::constructor::*;
    pub(in crate) use crate::class::impls::method::*;
    pub(in crate) use crate::class::impls::static_fn::*;
    pub(in crate) use crate::class::method::*;
    pub(in crate) use crate::class::static_fn::*;
    pub(in crate) use crate::class::*;
    pub(in crate) use crate::module::*;
    pub(in crate) use crate::obj::*;
    pub(in crate) use crate::phases::*;
    pub(in crate) use crate::traits::*;
}

// ------------------ LEGACY ------------------
mod legacy;

pub use crate::legacy::console;
// pub use crate::legacy::def::*;
pub use crate::legacy::document;
pub use crate::legacy::timing;
// pub use crate::legacy::traits::*;
// ------------------ LEGACY ------------------
