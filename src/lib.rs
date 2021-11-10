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
use std::{
    cell::UnsafeCell,
    error::Error,
    ffi::{c_void, CString},
    os::raw::c_char,
    path::{Path, PathBuf},
    time::Duration,
};

use gobject_sys::GObject;

pub mod logging;
pub mod prelude {
    pub use crate::svbuilder::SveltersBuilder;
    pub use crate::svelters::Svelters;
}

pub use handle::SvHandle;
pub use svbuilder::SveltersBuilder;
pub use svelters::Svelters;

pub(crate) mod handle;
pub(crate) mod svbuilder;
pub(crate) mod svelters;

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

pub enum SvEvent {
    Gc,
    Shutdown,
    Updated(hotwatch::Event),
    ScheduleCallback(Box<dyn Send + FnOnce(&Svelters) -> rquickjs::Result<()>>),
}

#[no_mangle]
pub extern "C" fn svelters_shutdown() {
    sv().shutdown().check()
}

#[no_mangle]
pub extern "C" fn svelters_gc() {
    sv().gc().check()
}

#[no_mangle]
pub extern "C" fn svelters_schedule_callback(
    func: unsafe extern "C" fn(*mut c_void),
    data: *mut c_void,
) {
    sv().schedule_ffi_callback(func, data).check();
}

fn sv() -> SvHandle {
    Svelters::handle()
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
