//! Expose an app's own Rust types and functions to Svelte (E0, `docs/notes/egui-rust-exports.md`).
//!
//! ```ignore
//! use svelters::prelude::*;
//!
//! #[derive(svelters::Record)]
//! pub struct Params { pub step: u32, #[js(default = 1.0)] pub scale: f64 }
//!
//! #[svelters::object(module = "demo")]          // UI thread, `&mut self`
//! pub struct Counter { value: i64 }
//!
//! #[svelters::export]
//! impl Counter {
//!     #[js(constructor)]
//!     pub fn new(start: i64) -> Self { Counter { value: start } }
//!     #[js(getter)]
//!     pub fn value(&self) -> i64 { self.value }
//!     pub fn add(&mut self, n: i64) -> i64 { self.value += n; self.value }
//! }
//!
//! #[svelters::export(module = "demo")]
//! pub fn greet(name: &str) -> String { format!("hello {name}") }
//! ```
//!
//! ```js
//! import { Counter, greet } from 'svelters:app/demo';
//! const c = new Counter(5);
//! c.add(2);           // 7
//! ```
//!
//! - `#[svelters::object(module = "m")]`: a UI-thread object (`Rc<RefCell<Self>>`; may be `!Send`;
//!   methods take `&self` or `&mut self`). `#[svelters::object(shared, module = "m")]`: `Arc<Self>`,
//!   checked `Send + Sync`; methods take `&self` or `self: Arc<Self>`.
//! - `#[svelters::export]` on an `impl` block exports its `pub fn`s (several blocks per type are fine):
//!   `#[js(constructor)]`, `#[js(getter)]`, `#[js(setter)]` (`set_x` is the property `x`), no receiver =
//!   static, `#[js(rename = "...")]`, `#[js(bigint)]` (return), `#[js(skip)]`. On a free function it
//!   needs `module = "m"`.
//! - Parameters: anything [`FromJs`] (numbers, `bool`, `String`, `Option`, `Vec`, maps, records,
//!   enums, `Arc<T>`/`Obj<T>` of exported objects), `&str`, `&[T]`, `&T`/`&mut T` of exported objects.
//!   `#[js(bigint)]` on a parameter makes it a JS `bigint`; `i64`/`u64` are otherwise checked numbers
//!   (a `RangeError` outside ±2^53).
//! - Results: anything [`IntoJs`], `Result<T, E>` with `E:` [`ExportError`] (`#[derive(svelters::Error)]`
//!   gives one `code` per variant), a new object by value (`Self`), `Arc<T>`/`Obj<T>` (the same JS
//!   object while it lives).
//! - JS imports `svelters:app/<module>`; the host serves the stubs. [`exports::dump`] writes
//!   `exports.json` (`svelters-exports/1`), `app.d.ts` and `<module>.js` for bundlers and svelte-check.
//!
//! # Offload (E1)
//!
//! The thread a call runs on is in its signature; every offloaded member returns a `Promise` in JS
//! and takes a trailing `{ signal, onProgress }`:
//!
//! - `#[js(worker)] fn`: rayon (rayon's global pool, or [`exports::set_worker_pool`]). Methods take
//!   `self: Arc<Self>` or `&self` of a shared object; parameters and results must be `Send + 'static`.
//! - `#[js(blocking)] fn`: a pool of plain threads, for blocking I/O.
//! - `async fn` (or `#[js(local)] async fn`): **the JS thread**, polled at the call and again when its
//!   waker fires (decision 2026-10-21). It may hold `!Send` state and borrow a UI object across
//!   `.await`. It must not block: offload CPU work with [`exports::worker`]`(|| ...).await` or I/O with
//!   [`exports::blocking`]. Debug builds warn when one poll takes more than 20 ms.
//! - `#[js(tokio)] async fn` (feature `tokio`): a tokio runtime ([`exports::set_tokio_handle`], else a
//!   small one of our own). tokio is not a default dependency.
//! - `#[js(supersede)]`: a new call cancels the previous in-flight calls of the member on the same
//!   object (they reject with `superseded`); `#[js(max_in_flight = n)]` queues calls beyond `n`.
//!
//! A [`WorkCx<P>`] parameter is injected (not a JS argument): [`WorkCx::check`] / `is_cancelled` /
//! `cancelled().await` for cancellation, [`WorkCx::progress`] for `onProgress` (latest value, once per
//! checkpoint). An abort rejects the Promise at once with `NativeError { domain: 'svelters', code:
//! 'cancelled' }` (decision 2026-10-21); the job sees the cancel through `WorkCx`, and its late result
//! is dropped on the JS thread. A panic rejects with `code: 'panic'`, disposing the object a call runs
//! on rejects it with `disposed`, and a hot reload drops the calls of the old runtime.
//!
//! # Buffers (E2)
//!
//! Large numeric data crosses without a copy:
//!
//! - [`Buffer<T>`] (`T` = `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`) is an owned
//!   typed array. As a parameter the caller's `Float32Array` is **moved**: it is detached in JS (length
//!   0) for good, and Rust gets its memory; a worker may mutate it in place and return it, and JS then
//!   gets the same memory back as a new typed array. As a result, JS owns the memory.
//! - [`Shared<[T]>`] is an `Arc<[T]>`: JS gets a read-only view (`ReadonlyFloat32Array` in the typings;
//!   the engine ignores writes) and the `Arc` is released when JS collects it. As a parameter, a view
//!   the host handed out gives back its `Arc`; other arrays are copied.
//! - `alloc(Float32Array, n)` from `@svelters/types/core` makes a typed array over a Rust allocation;
//!   passing it as a `Buffer<T>` gives Rust that allocation back (no copy).
//!
//! Memory that came from JS stays in the JS engine's allocator while Rust holds it: dropping the
//! `Buffer<T>` (on a worker, after a panic, or as a late result after an abort) releases it on the JS
//! thread. Buffers work in records, lists, worker arguments and results.

pub use svelters_host::exports;
pub use svelters_host::exports::{
    Buffer, CallCx, Cancelled, Element, ExportError, ExportObject, FromJs, IntoJs, JsType, NativeError, Obj, Shared, SharedObject, TypeBuilder, TypeRef,
    UiObject, Value, WorkCx,
};
/// The rayon `#[js(worker)]` exports run on (for [`exports::set_worker_pool`] and `par_iter` in jobs).
pub use svelters_host::exports::offload::rayon;
pub use svelters_macros::{Enum, Error, Record, export, object};

/// `use svelters::prelude::*;`
pub mod prelude {
    pub use super::{Buffer, Cancelled, ExportError, NativeError, Obj, Shared, WorkCx};
    pub use std::sync::Arc;
}

#[doc(hidden)]
pub use svelters_host::exports::__private;
