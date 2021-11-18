#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(type_alias_impl_trait)]
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
// #![feature(const_str_from_utf8_unchecked)]
#![feature(const_fn_trait_bound)]
#![feature(const_type_name)]
#![feature(specialization)]
#![allow(incomplete_features)]
#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::error::Error;

pub mod logging;

pub(crate) mod js_modules {
    pub mod console;
    pub mod loader;
    pub mod timing;
}

pub(crate) mod js {
    pub use rquickjs::{
        bind, BuiltinLoader, BuiltinResolver, Context, Ctx, ExecutorSpawner, FileResolver, FromJs,
        Func, Function, IntoAtom, IntoJs, ModuleLoader, NativeLoader, Object, Persistent,
        Result as JsResult, Runtime, ScriptLoader, Value,
    };
}

trait CheckErr {
    fn check(self);
}

impl<T, E: Error> CheckErr for Result<T, E> {
    fn check(self) {
        if let Err(err) = self {
            tracing::error!("{}", err);
        }
    }
}
