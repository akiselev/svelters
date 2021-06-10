#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char};

use anyhow::Result;
// use qmetaobject::{connections::connect, QString};

// mod application;
// mod button;
// mod comp;
mod js;
mod logging;
// mod main_window;
pub mod types;

use js::compile_svelte;
// pub mod widgets;

// pub use comp::*;

// use application::QApplication;
// use main_window::QMainWindow;

pub struct QRef<'a, T> {
    ptr: T,
    _marker: std::marker::PhantomData<&'a T>,
}

impl<'a, T> QRef<'a, T> {
    pub unsafe fn new<'inner>(ptr: T) -> QRef<'inner, T> {
        QRef {
            ptr,
            _marker: std::marker::PhantomData,
        }
    }
}

pub fn compile() -> Result<String> {
    let res = compile_svelte("src/ts/index.ts")?;
    println!("{}", res);
    Ok(res)
}
