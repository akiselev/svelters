#![recursion_limit = "1024"]
#![allow(unused)]
#![allow(non_camel_case_types)]

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_char;
use std::pin::Pin;

use cpp::{cpp, cpp_class};
use phf::phf_map;
use qttypes::*;
use svelters_core::Framework;

// mod document;
// mod elements;
// mod object;

// pub use document::Document;
// pub use object::QObject;

// pub(crate) use elements::ELEMENTS;

#[derive(Clone)]
pub struct Qt {}

impl Qt {
    pub fn new() -> Self {
        Self {}
    }
}

impl Framework for Qt {}
