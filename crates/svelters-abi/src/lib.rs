//! Rust mirror of `include/svelters_backend.h`. Source of truth: plans/architecture/backend-abi.md.
//! P0 provides the raw `#[repr(C)]` types; safe wrappers and the backend-trait helper are P2's.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

pub const SVL_ABI_VERSION: u32 = 3;

pub type svl_handle = u64;
pub type svl_id = u32;

// ---- tags ----
pub const SVL_NULL: u16 = 0;
pub const SVL_BOOL: u16 = 1;
pub const SVL_I64: u16 = 2;
pub const SVL_F64: u16 = 3;
pub const SVL_STR: u16 = 4;
pub const SVL_HANDLE: u16 = 5;
pub const SVL_LIST: u16 = 6;
pub const SVL_ENUM: u16 = 7;
pub const SVL_FLAGS: u16 = 8;
pub const SVL_RECORD: u16 = 9;
pub const SVL_BYTES: u16 = 10;
pub const SVL_BOXED: u16 = 11;
pub const SVL_CALLBACK: u16 = 12;
pub const SVL_MAP: u16 = 13;
pub const SVL_ERROR: u16 = 14;

// ---- svl_value.flags ----
pub const SVL_F_UTF16: u16 = 1 << 0;
pub const SVL_F_NUL_TERM: u16 = 1 << 1;
pub const SVL_F_UNSIGNED: u16 = 1 << 2;
pub const SVL_F_OWNED_BY_CALLEE: u16 = 1 << 3;

// ---- well-known value types ----
pub const SVL_T_RECT: u32 = 1;
pub const SVL_T_SIZE: u32 = 2;
pub const SVL_T_POINT: u32 = 3;
pub const SVL_T_INSETS: u32 = 4;
pub const SVL_T_COLOR: u32 = 5;
pub const SVL_T_DATE: u32 = 6;
pub const SVL_T_TIME: u32 = 7;
pub const SVL_T_DATETIME: u32 = 8;
pub const SVL_T_VARIANT: u32 = 9;
pub const SVL_T_TYPED_VARIANT: u32 = 10;
/// ABI v2.5: `STR` carrying a native type name (a GType, QMetaType or Class).
pub const SVL_T_TYPE: u32 = 11;
pub const SVL_T_FIRST_BACKEND: u32 = 0x10000;

// ---- descriptor flags ----
pub const SVL_D_NULLABLE: u16 = 1;
pub const SVL_D_OPTIONAL: u16 = 2;
pub const SVL_D_OUT: u16 = 4;
pub const SVL_D_INOUT: u16 = 8;
pub const SVL_D_UNSIGNED: u16 = 16;
pub const SVL_D_INTEGRAL: u16 = 32;
pub const SVL_D_BIGINT: u16 = 64;
pub const SVL_D_BORROWED_ARG: u16 = 128;
pub const SVL_D_STRING_FORM: u16 = 256;
pub const SVL_D_READONLY: u16 = 512;
pub const SVL_M_THROWS: u32 = 1;
pub const SVL_M_ASYNC: u32 = 2;
pub const SVL_M_SYNC_SAFE: u32 = 4;
pub const SVL_M_BLOCKING: u32 = 8;
/// ABI v2.5: reached through `call_static` only.
pub const SVL_M_STATIC: u32 = 16;
/// ABI v2.5: with `SVL_M_STATIC`, returns a new reference.
pub const SVL_M_CTOR: u32 = 32;
/// ABI v2.6: on `SVL_PROP` descriptors, `observe` can watch this property.
pub const SVL_M_OBSERVABLE: u32 = 64;
/// ABI v2.6 (R1): on `SVL_PROP` descriptors, reading the value costs time proportional to its size
/// (whole-text props): the host does not read or compare it after a change, it delivers an
/// invalidation only, and the backend suppresses the echo of the host's own `set_prop`.
pub const SVL_M_PROPORTIONAL: u32 = 128;

pub const SVL_CB_CALL: u16 = 1;
pub const SVL_CB_ASYNC: u16 = 2;
pub const SVL_CB_NOTIFIED: u16 = 3;
pub const SVL_CB_FOREVER: u16 = 4;

pub const SVL_CAP_UTF16_IN: u32 = 1;
pub const SVL_CAP_UTF16_OUT: u32 = 2;
pub const SVL_CAP_BYTES_RETAIN: u32 = 4;
/// ABI v2.9: `set_prop` compares a scalar write with the value the backend stores and drops it
/// when equal, cheaply (typed columns), so the JS runtime may skip its own last-value cache for
/// typed scalar writes (`setp`).
pub const SVL_CAP_DEDUPE: u32 = 8;

pub const SVL_KIND: u32 = 0;
pub const SVL_PROP: u32 = 1;
pub const SVL_SIGNAL: u32 = 2;
pub const SVL_METHOD: u32 = 3;
pub const SVL_SLOT: u32 = 4;
/// ABI v2.2: child attribute, scoped by the parent's kind.
pub const SVL_ATTR: u32 = 5;

/// ABI v2.2 `listen_ex` flags.
pub const SVL_LISTEN_AFTER: u32 = 1;
pub const SVL_LISTEN_CAPTURE: u32 = 2;
/// Listener flags ride in these bits of the signal id of `post_event` / `call_sync`.
pub const SVL_SIGNAL_FLAGS_SHIFT: u32 = 28;
pub const SVL_SIGNAL_ID_MASK: u32 = (1 << SVL_SIGNAL_FLAGS_SHIFT) - 1;

/// ABI v2.3 `style` ops (X6 styling). Payloads are UTF-8 strings; see plans/architecture/backend-abi.md.
pub const SVL_STYLE_TARGET: u32 = 0;
pub const SVL_STYLE_CLASSES: u32 = 1;
pub const SVL_STYLE_NAME: u32 = 2;
pub const SVL_STYLE_INLINE: u32 = 3;
pub const SVL_STYLE_OPACITY: u32 = 4;
pub const SVL_STYLE_SHEET: u32 = 5;
pub const SVL_STYLE_SCHEME: u32 = 6;
/// ST2: an element's resolved declarations per state, for backends of the decl dialect (AppKit):
/// blocks separated by `\x02`, each the state name (empty for the base) then `\x01prop:value` fields.
pub const SVL_STYLE_DECLS: u32 = 7;
/// ABI v2.8 (TR1): `"tx ty sx sy ox oy"` translate (px), scale, origin (fractions); `None` = identity.
pub const SVL_STYLE_TRANSFORM: u32 = 8;
/// ABI v2.8 (TR1): `"w h"` px the element occupies and is cut at (`-1`: that axis unclipped); `None` = none.
pub const SVL_STYLE_CLIP: u32 = 9;
/// ABI v2.8 (TR1): `"1"`: no pointer input or focus (an outroing element); `None` restores.
pub const SVL_STYLE_INERT: u32 = 10;
/// ABI v2.9 (TR2): an animation's keyframes and timing, sent once: JSON `{"id":n,"duration":ms,
/// "delay":ms,"frames":[{"opacity":o,"transform":[6],"clip":[2]},..]}`, or `{"id":n,"cancel":true}`.
/// The backend interpolates and reports the end with the `animationend` signal (argument: the id).
/// A backend without it refuses the op (an error) and the runtime plays the animation itself.
/// skelter (`native_transitions`): an optional `"native":{"kind","params","computed","t1","t2"[,"flip"]}`
/// names the svelte/transition built-in or `flip` the frames came from (crates/skelter-motion `op`);
/// other backends ignore it.
pub const SVL_STYLE_ANIMATE: u32 = 11;
/// skelter (`css_precompiled`): a precompiled sheet, `id SEP base64` of `svelters_style::binary`
/// bytes (the sheet, rule hints per element, utility rules); replaces sheet `id` like `SVL_STYLE_SHEET`.
pub const SVL_STYLE_SHEET_BIN: u32 = 12;
/// `SVL_STYLE_TARGET` answers: the dialect the backend's sheets use.
pub const SVL_STYLE_TARGET_NONE: i32 = 0;
pub const SVL_STYLE_TARGET_CSS: i32 = 1;
pub const SVL_STYLE_TARGET_QSS: i32 = 2;
/// The decl dialect with the AppKit style table (`metadata/style/appkit.yaml`).
pub const SVL_STYLE_TARGET_APPKIT: i32 = 3;
/// Separator between the fields of a `style` payload string.
pub const SVL_STYLE_SEP: char = '\u{1}';

pub const SVL_LIFE_OWNED: u32 = 1;
pub const SVL_LIFE_TOPLEVEL: u32 = 2;
pub const SVL_LIFE_BORROWED: u32 = 3;
pub const SVL_LIFE_VALUE: u32 = 4;
/// ABI v2.5: a strong reference to a refcounted object outside the widget tree.
pub const SVL_LIFE_SHARED: u32 = 5;

pub const SVL_GONE_UNKNOWN: u32 = 0;
pub const SVL_GONE_CLOSED: u32 = 1;
pub const SVL_GONE_PARENT: u32 = 2;
pub const SVL_GONE_OWNER: u32 = 3;
pub const SVL_GONE_REPLACED: u32 = 4;
pub const SVL_GONE_SHUTDOWN: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct svl_handle_info {
    pub kind: svl_id,
    pub lifetime: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_str {
    pub ptr: *const c_void,
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
pub struct svl_bytes {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union svl_value_u {
    pub b: bool,
    pub i: i64,
    pub u: u64,
    pub f: f64,
    pub h: svl_handle,
    pub s: svl_str,
    pub l: svl_list,
    pub bytes: svl_bytes,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_value {
    pub tag: u16,
    pub flags: u16,
    pub type_: svl_id,
    pub u: svl_value_u,
}

impl svl_value {
    pub const fn new(tag: u16, u: svl_value_u) -> svl_value {
        svl_value { tag, flags: 0, type_: 0, u }
    }
    pub const fn null() -> svl_value {
        svl_value::new(SVL_NULL, svl_value_u { i: 0 })
    }
    pub const fn typed(tag: u16, type_: svl_id, u: svl_value_u) -> svl_value {
        svl_value { tag, flags: 0, type_, u }
    }
}

const _: () = assert!(std::mem::size_of::<svl_value>() == 24);

// ---- descriptors ----

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_slot_desc {
    pub name: *const c_char,
    pub tag: u16,
    pub flags: u16,
    pub type_: svl_id,
    pub min: f64,
    pub max: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_member_desc {
    pub name: *const c_char,
    pub params: *const svl_slot_desc,
    pub n_params: u32,
    pub ret: svl_slot_desc,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_enum_member {
    pub name: *const c_char,
    pub value: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_type_desc {
    pub name: *const c_char,
    pub kind: u16,
    pub scope: u16,
    pub members: *const svl_enum_member,
    pub n_members: u32,
    pub fields: *const svl_slot_desc,
    pub n_fields: u32,
    pub ret: svl_slot_desc,
}

// Descriptors are immutable static data owned by the backend.
unsafe impl Sync for svl_slot_desc {}
unsafe impl Sync for svl_member_desc {}
unsafe impl Sync for svl_enum_member {}
unsafe impl Sync for svl_type_desc {}

#[repr(C)]
pub struct svl_host {
    pub self_: *mut c_void,
    pub post_event: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize),
    pub checkpoint: unsafe extern "C" fn(*mut c_void),
    pub call_sync: unsafe extern "C" fn(
        *mut c_void,
        svl_handle,
        svl_id,
        *const svl_value,
        usize,
        *mut svl_value,
    ) -> c_int,
    pub on_destroyed: unsafe extern "C" fn(*mut c_void, svl_handle, u32),
    pub invoke_callback:
        unsafe extern "C" fn(*mut c_void, u64, *const svl_value, usize, *mut svl_value) -> c_int,
    pub release_callback: unsafe extern "C" fn(*mut c_void, u64),
    /// ABI v2.1: a requested frame is due.
    pub frame: unsafe extern "C" fn(*mut c_void),
    /// ABI v2.6: an observed property may have changed (no value). Optional.
    pub prop_changed: Option<unsafe extern "C" fn(*mut c_void, svl_handle, svl_id)>,
    /// ABI v2.9 (W-H1): deliver a signal now, at event time, and drain the JS jobs it queued
    /// (Svelte's flush), so its result is in the scene when this returns. `SVL_SYNC_OK`,
    /// `SVL_SYNC_BUSY` (JS was running: queued for the next checkpoint, as `post_event`) or
    /// `SVL_SYNC_FAILED`. Optional (NULL: a host without it; use `post_event` + `checkpoint`).
    pub dispatch_now: Option<unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize) -> c_int>,
}

pub const SVL_SYNC_OK: i32 = 0;
/// `call_sync` ran nothing or failed: the backend uses the toolkit default.
pub const SVL_SYNC_FAILED: i32 = -1;
/// `call_sync` found JS running: the host queued the event for async delivery.
pub const SVL_SYNC_BUSY: i32 = -2;
/// ABI v2.6 `observe`: the property exists but this backend cannot watch it natively.
pub const SVL_OBSERVE_UNSUPPORTED: i32 = -2;

#[repr(C)]
pub struct svl_backend {
    pub abi_version: u32,
    pub self_: *mut c_void,
    pub resolve: unsafe extern "C" fn(*mut c_void, u32, svl_id, *const c_char, usize) -> svl_id,
    pub create: unsafe extern "C" fn(*mut c_void, svl_id) -> svl_handle,
    pub insert:
        unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle, svl_handle, svl_id) -> c_int,
    pub remove: unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle) -> c_int,
    pub destroy: unsafe extern "C" fn(*mut c_void, svl_handle),
    pub set_prop: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value) -> c_int,
    pub get_prop: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *mut svl_value) -> c_int,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        svl_handle,
        svl_id,
        *const svl_value,
        usize,
        *mut svl_value,
    ) -> c_int,
    pub listen: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, bool) -> c_int,
    pub handle_info: unsafe extern "C" fn(*mut c_void, svl_handle, *mut svl_handle_info) -> c_int,
    pub run: unsafe extern "C" fn(*mut c_void, *const svl_host) -> c_int,
    pub wake: unsafe extern "C" fn(*mut c_void),
    pub schedule_wake: unsafe extern "C" fn(*mut c_void, i64),
    pub quit: unsafe extern "C" fn(*mut c_void),
    pub last_error: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub caps: u32,
    pub describe: Option<unsafe extern "C" fn(*mut c_void, u32, svl_id, svl_id) -> *const svl_member_desc>,
    pub describe_type: Option<unsafe extern "C" fn(*mut c_void, svl_id) -> *const svl_type_desc>,
    pub cancel: Option<unsafe extern "C" fn(*mut c_void, u64) -> c_int>,
    /// ABI v2.1: one `host.frame` on the toolkit's next frame.
    pub request_frame: unsafe extern "C" fn(*mut c_void),
    /// ABI v2.2: set a per-child attribute (`SVL_NULL` resets). Optional.
    pub set_child_attr: Option<
        unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle, svl_id, *const svl_value) -> c_int,
    >,
    /// ABI v2.2: read a per-child attribute back. Optional.
    pub get_child_attr: Option<
        unsafe extern "C" fn(*mut c_void, svl_handle, svl_handle, svl_id, *mut svl_value) -> c_int,
    >,
    /// ABI v2.2: `listen` with `SVL_LISTEN_*` flags. Optional.
    pub listen_ex: Option<unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, bool, u32) -> c_int>,
    /// ABI v2.3: styling (classes, widget name, inline declarations, opacity, sheets, scheme). Optional.
    /// `v` is `SVL_STR` or `SVL_NULL`; the result is `>= 0` on success (`SVL_STYLE_TARGET` returns the dialect).
    pub style: Option<unsafe extern "C" fn(*mut c_void, u32, svl_handle, *const svl_value) -> c_int>,
    /// ABI v2.5: create an instance with construct-time properties. Optional.
    pub construct: Option<
        unsafe extern "C" fn(*mut c_void, svl_id, *const svl_id, *const svl_value, usize, *mut svl_value) -> c_int,
    >,
    /// ABI v2.5: call a static function or constructor function of a kind. Optional.
    pub call_static:
        Option<unsafe extern "C" fn(*mut c_void, svl_id, svl_id, *const svl_value, usize, *mut svl_value) -> c_int>,
    /// ABI v2.6: start or stop watching a property natively (`host.prop_changed`). Optional.
    pub observe: Option<unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, bool) -> c_int>,
    /// ABI v2.7: attach the host and bring the toolkit up without running the loop; returns at
    /// once (`0` ok, `< 0` + `last_error` failure). The caller then drives the loop and the backend
    /// calls the host exactly as from `run`. `start(self, NULL)` detaches the host. Optional.
    pub start: Option<unsafe extern "C" fn(*mut c_void, *const svl_host) -> c_int>,
    /// ABI v2.8: the layout rect of a handle in its toplevel (x, y, width, height into `out[4]`,
    /// logical px; natural size at 0, 0 when not laid out yet). Optional.
    pub bounds: Option<unsafe extern "C" fn(*mut c_void, svl_handle, *mut f64) -> c_int>,
    /// ABI v2.9 (W-C2): build a whole static subtree from a template in one call. Node `k` of the
    /// template gets handle `*out_base + k` (contiguous). Optional: the host expands a template
    /// into `create`/`set_prop`/`insert`/`listen_ex` when NULL (`svl_template` has the order).
    pub instantiate: Option<unsafe extern "C" fn(*mut c_void, *const svl_template, *mut svl_handle) -> c_int>,
    /// ABI v2.9 (W-C2): destroy the `n` handles `base..base + n` of one instantiation, as `destroy`
    /// of each, children before parents. Optional (requires `instantiate`).
    pub destroy_range: Option<unsafe extern "C" fn(*mut c_void, svl_handle, u32)>,
}

/// ABI v2.9 (W-C2): one node of an [`svl_template`], in pre-order.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct svl_tpl_node {
    pub kind: svl_id,
    /// Index + 1 of the template parent; 0 for a root (the runtime places roots itself).
    pub parent: u32,
    /// Slot in the parent (`SVL_SLOT` id of the parent's kind), 0 = default.
    pub slot: svl_id,
    /// This node's static props: `props[first_prop..first_prop + n_props]`.
    pub first_prop: u32,
    pub n_props: u32,
}

/// ABI v2.9: a static prop of a template node: an id of the node's kind and a typed constant.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_tpl_prop {
    pub prop: svl_id,
    pub value: svl_value,
}

/// ABI v2.9: a listener of a template node (`listen_ex` with `flags`; 0 = plain `listen`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct svl_tpl_listen {
    pub node: u32,
    pub signal: svl_id,
    pub flags: u32,
}

/// ABI v2.9 (W-C2, plans/research/skelter/compiler-and-js.md §3.2): a static subtree. Instantiating
/// it is, op for op, this sequence (the host writes it to the op log when it expands one): for
/// each node in pre-order, `create` and its static props in table order; when a node's subtree is
/// complete, its `insert` into its template parent (roots are not inserted), then its listeners.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct svl_template {
    /// Content hash of the template table (stable across instantiations and reloads).
    pub id: u64,
    pub nodes: *const svl_tpl_node,
    pub n_nodes: usize,
    pub props: *const svl_tpl_prop,
    pub n_props: usize,
    pub listens: *const svl_tpl_listen,
    pub n_listens: usize,
}

// Backends are driven from the UI thread; `wake` is the only thread-safe entry point.
unsafe impl Sync for svl_backend {}

/// Path to the C header, for C/C++ backends' build scripts.
pub const HEADER_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include");

mod buffer;
mod safe;
pub use buffer::*;
pub use safe::*;
