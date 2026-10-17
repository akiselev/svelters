//! Rust mirror of `include/svelters_backend.h`. Source of truth: plans/architecture/backend-abi.md.
//! P0 provides the raw `#[repr(C)]` types; safe wrappers and the backend-trait helper are P2's.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

pub const SVL_ABI_VERSION: u32 = 1;

pub type svl_handle = u64;
pub type svl_id = u32;

pub const SVL_NULL: u32 = 0;
pub const SVL_BOOL: u32 = 1;
pub const SVL_I64: u32 = 2;
pub const SVL_F64: u32 = 3;
pub const SVL_STR: u32 = 4;
pub const SVL_HANDLE: u32 = 5;
pub const SVL_LIST: u32 = 6;

pub const SVL_KIND: u32 = 0;
pub const SVL_PROP: u32 = 1;
pub const SVL_SIGNAL: u32 = 2;
pub const SVL_METHOD: u32 = 3;
pub const SVL_SLOT: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_str {
    pub ptr: *const c_char,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_list {
    pub ptr: *const svl_value,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union svl_value_u {
    pub b: bool,
    pub i: i64,
    pub f: f64,
    pub h: svl_handle,
    pub s: svl_str,
    pub l: svl_list,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_value {
    pub tag: u32,
    pub u: svl_value_u,
}

#[repr(C)]
pub struct svl_host {
    pub self_: *mut c_void,
    pub post_event: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize),
    pub checkpoint: unsafe extern "C" fn(*mut c_void),
    pub call_sync: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize, *mut svl_value) -> c_int,
    pub on_destroyed: unsafe extern "C" fn(*mut c_void, svl_handle),
}

#[repr(C)]
pub struct svl_backend {
    pub abi_version: u32,
    pub self_: *mut c_void,
    pub resolve: unsafe extern "C" fn(*mut c_void, u32, svl_id, *const c_char, usize) -> svl_id,
    pub create: unsafe extern "C" fn(*mut c_void, svl_id) -> svl_handle,
    pub insert: unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle, svl_handle, svl_id) -> c_int,
    pub remove: unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle) -> c_int,
    pub destroy: unsafe extern "C" fn(*mut c_void, svl_handle),
    pub set_prop: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value) -> c_int,
    pub get_prop: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *mut svl_value) -> c_int,
    pub call: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize, *mut svl_value) -> c_int,
    pub listen: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, bool) -> c_int,
    pub run: unsafe extern "C" fn(*mut c_void, *const svl_host) -> c_int,
    pub wake: unsafe extern "C" fn(*mut c_void),
    pub quit: unsafe extern "C" fn(*mut c_void),
    pub last_error: unsafe extern "C" fn(*mut c_void) -> *const c_char,
}

// Backends are driven from the UI thread; `wake` is the only thread-safe entry point.
unsafe impl Sync for svl_backend {}

/// Path to the C header, for C/C++ backends' build scripts.
pub const HEADER_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include");

mod safe;
pub use safe::*;
