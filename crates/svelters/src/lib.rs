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

//!
//! # Reactive exports (E3)
//!
//! - `#[svelters::store(module = "m")]` on a struct: a [`Store<S>`] (or an app-wide [`StoreStatic<S>`],
//!   exported with `#[svelters::export(module = "m")]` on the `static`) that any thread writes and Svelte
//!   reads like `$state`: one dirty bit per field (decision 2026-10-22), folded into the host's change
//!   batch at the next checkpoint. `#[store(writable)]` fields are assignable from JS (and `bind:`-able;
//!   [`StoreHooks::on_write`] validates), `#[store(proportional)]` fields are delivered as invalidations.
//! - `#[svelters::service(module = "m")] fn name(cx: ServiceCx) -> V`: started with the app (the first
//!   `Host`), never restarted by a hot reload, stopped by [`exports::shutdown_services`].
//! - [`Emitter<E>`] with `#[derive(svelters::Events)]`: events from any thread to `obj.on(name, fn)`,
//!   delivered at the checkpoint after the change batch.
//! - [`Sink<T>`] parameters of offloaded exports and [`Stream<T>`] results: bounded async iterables.
//! - JS `persist(key, () => new T(...))` (from `@svelters/types/core`) keeps an app object across hot
//!   reloads; objects that are not persisted die with their runtime.
//!
//! # Serde values and data enums (RA2)
//!
//! - [`Json<T>`] (`T: Serialize + DeserializeOwned + schemars::JsonSchema`) crosses as a plain JS value
//!   (objects, arrays, numbers, strings), never as JSON text, for types that cannot derive `Record`
//!   (a headless crate, or the orphan rule). Its typings come from `T`'s JSON Schema: named schemas
//!   become exported types of the module, and `exports.json` carries the schema itself. [`JsonAny<T>`]
//!   needs only serde and is typed `unknown`.
//! - `#[derive(svelters::Enum)]` with `#[js(tag = "kind")]` takes unit, struct and newtype variants: a
//!   discriminated union of plain objects (`{ kind: 'circle', r: 1 }`, serde's internally tagged form).
//!   Variant tags are kebab-case (`#[js(rename)]`), fields camelCase like records.
//! - A JS value that does not fit throws `NativeError { domain: 'svelters' }` with `code` `type`,
//!   `range` (an integer out of range), `missing-field`, `unknown-variant` (a wrong tag),
//!   `unknown-field` (serde's `deny_unknown_fields`) or `invalid` (a serde `custom` error), and a
//!   message that starts with the path: `at shape.r: invalid type: string "x", expected f64`.
//! - The codegen works on any [`exports::Schema`] value: `schema.typescript()`, `schema.json()`,
//!   `schema.javascript(..)`, `schema.dump(dir)` (SV24).
//!
//! # Native egui widget kinds (RA1)
//!
//! `#[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]` on a
//! struct makes it an element of the egui backend: `#[prop]` fields (typed by the same `JsType`
//! schema as exports; `#[prop(bind)]` for `bind:`), `#[event] x: Event<T>` fields (`onx`), and
//! `#[state]` fields (Rust-only). The struct implements `svelters_egui::provider::Widget`
//! (`measure`, `ui`, `a11y`); the crate depends on `svelters-egui` as well. `svelters_egui::dump(dir)`
//! writes the exports plus `kinds.json` (the compiler's `--spec-fragment`, Vite `specFragments`) and
//! `kinds.d.ts` (the `IntrinsicElements` typings). See `examples/egui-kinds` and
//! `plans/milestones/RA1-widget-kinds.md`.
//!
//! RA3 (`plans/milestones/RA3-widget-api.md`): `#[svelters::widget] impl Kind { pub fn … }` gives the
//! kind methods JS calls on the element (`bind:this`, then `field.commit()`); `#[svelters::widget]` on
//! a `fn(ui: &mut egui::Ui, props…) -> egui::Response` is the fn form; [`wrap_widget!`] wraps an
//! ecosystem egui widget; `caps = CONTAINER` takes children (`cx.children(ui)` draws them).
//!
//! # Native behaviours (EV2b)
//!
//! `#[svelters::behavior(namespace = "cad", name = "pick")]` on a struct makes it a behaviour that
//! Svelte attaches to any element with `use:pick` (imported from `svelters:behaviors/cad`): `#[prop]`
//! fields are the action's params, `#[event] x: Event<T>` fields its `onx` callbacks. The struct
//! implements `svelters_egui::provider::Behavior` (`after_ui` runs after the host's `ui` every frame,
//! in Rust). The dump adds `behaviors/<ns>.js` and `behaviors.d.ts`. See
//! `plans/milestones/EV2b-behaviors.md`.

pub use svelters_host::exports;
/// AS1: an asset ref (`asset://<root>/<path>`), as an export parameter, result or `Json<T>` field.
pub use svelters_host::modules::{AssetRef, Assets};
pub use svelters_host::exports::{
    Buffer, CallCx, Cancelled, Closed, Element, Emitter, EventSet, ExportError, ExportObject, FromJs, IntoJs, JsType, Json, JsonAny, NativeError, Obj, ServiceCx, Shared,
    SharedObject, Sink, StopToken, Store, StoreHooks, StoreState, StoreStatic, Stream, TypeBuilder, TypeRef, UiObject, Value, WorkCx, channel,
};
/// The rayon `#[js(worker)]` exports run on (for [`exports::set_worker_pool`] and `par_iter` in jobs).
pub use svelters_host::exports::offload::rayon;
pub use svelters_macros::{Enum, Error, Events, Record, behavior, export, object, service, store, widget, wrap_widget};

/// `use svelters::prelude::*;`
pub mod prelude {
    pub use super::{
        Buffer, Cancelled, Closed, Emitter, ExportError, Json, JsonAny, NativeError, Obj, ServiceCx, Shared, Sink, StopToken, Store, StoreHooks, StoreStatic, Stream, WorkCx, channel,
    };
    pub use std::sync::Arc;
}

#[doc(hidden)]
pub use svelters_host::exports::__private;
