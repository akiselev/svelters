#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char};

use anyhow::Result;

mod js;
mod logging;
pub mod runtime {
    pub mod loader;
    pub mod resolver;
}

use js::compile_svelte;

pub fn compile() -> Result<String> {
    let res = compile_svelte("src/ts/index.ts")?;
    println!("{}", res);
    Ok(res)
}
