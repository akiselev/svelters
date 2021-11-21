#![feature(associated_type_bounds)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(unboxed_closures)]
#![feature(const_fn_fn_ptr_basics)]
#![allow(unused)]

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use phf::phf_map;

use cpp_core::{CastInto, DynamicCast};
use qt_core::{QMetaMethod, QMetaObject, QMetaProperty, QObject};

use qttypes::*;

use rquickjs::{Ctx, Value};

pub(crate) mod common;
pub(crate) mod macros;
pub(crate) mod meta;
pub(crate) mod newtype;
pub(crate) mod object;
mod registry;
pub(crate) mod variant;

pub use crate::object::Object;
pub use crate::registry::*;
