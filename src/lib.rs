#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char};

use anyhow::Result;

pub mod logging;
pub mod runtime {
    pub mod console;
}
