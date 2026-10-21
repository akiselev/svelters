# egui: exposing the app's own Rust types, functions and widgets to Svelte

Status: research proposal, 2026-10-21, for M7 (egui). **E0, E1 and E3 are implemented** (`plans/milestones/E0-rust-exports.md`, `plans/milestones/E1-exports-offload.md`, `plans/milestones/E3-exports-reactive.md`: as built, deviations, measurements); E2, E4 and E5 are not.

- **Scope:** how the developer of an all-Rust svelters-egui app exposes their own types, functions, background services and egui widgets to Svelte/TS, with compute offloaded to Rust threads.
- **Not in scope:** the retained scene graph that the egui backend renders. Another lane is designing it; §7.4 lists only the interface this design needs from it.
- **Probe:** `docs/notes/scripts/egui-exports-probe/` is a standalone crate with its own empty `[workspace]`. It depends on `svelters-host`/`svelters-abi` by path. Run it with `CARGO_TARGET_DIR=… cargo run --release`. Numbers are in §4.
- **Builds on:**
  - `plans/research/NON-GUI-TYPES.md`: handles, SHARED, `construct`/`call_static`, async → Promise, `{ signal }`, `NativeError`, the `@svelters/types` core;
  - `plans/research/REACTIVITY.md`: observe, the dirty set, one `__svelters_changed` batch per checkpoint, `observed()` getters, `PlatformSemantics::native_type`;
  - `plans/research/CUSTOM-WIDGETS.md`: packs, id ranges. Its open question 9 asks about `#[svelters::export]` for Rust GTK widget methods, and §3 below answers it;
  - `plans/research/QUICKJS-RUNTIME.md`: built-in `svelters:*` modules, async host ops on our own completion queue, item 13 ("don't bind widgets with `#[rquickjs::class]`").

## Summary

1. **Where exports live: in the host, behind an engine-neutral Rust registry.** They don't go through the backend C ABI, and they don't use rquickjs classes.
   - User types are not toolkit code, so they have no business in `svl_backend`. That boundary exists for GTK/Qt/C++ and adds measurable marshalling (§4).
   - Binding straight to rquickjs is about 4x cheaper per call, but it gives up `JsEngine` neutrality (JSC, Node/N-API), handle identity, `dispose`, the op log, the reactivity batch and reload survival.
   - The registry is a Rust trait-object table inside `svelters-host`. It speaks the owned `Value` model plus a new zero-copy `Buffer` variant. The host routes handles and kinds in a reserved `app:` range to it instead of the backend.
   - Because it lives in the host, **the same exports work on GTK, Qt, AppKit and WinUI**, not only egui.
2. **Macro surface.**
   - `#[derive(svelters::Record)]` / `#[derive(svelters::Enum)]`: by-value data.
   - `#[svelters::object]` (UI-thread, `&mut self`) and `#[svelters::object(shared)]` (`Arc<Self>`, `Send + Sync`): handle types.
   - `#[svelters::export]` goes on `impl` blocks (several per type are allowed), free functions, `async fn`, and traits that JS implements.
   - `#[svelters::store]`: reactive state that any thread may write.
   - `Emitter<E>`: events.
   - `#[svelters::widget]`: egui widgets as Svelte elements.
   - JS imports everything from `svelters:app/<module>`.
3. **Threading is opt-in per call and checked at compile time.**
   - `#[js(worker)]` runs on rayon, `#[js(blocking)]` on a blocking pool, a bare `async fn` (or `#[js(local)] async fn`) on the JS thread, `#[js(tokio)] async fn` on tokio (decided 2026-10-21, §11).
   - Arguments become owned `Send` data on the JS thread. Results come back through a **host completion queue drained at the start of every checkpoint**. egui's `request_repaint` is the wake.
   - Workers never see JS values.
   - Cancellation is the existing `{ signal }` → `cancelAsync` path.
   - Panics become `NativeError { domain: 'svelters', code: 'panic' }`.
4. **Zero-copy is real on QuickJS-NG (measured):**
   - a 64 MiB `Arc<[f32]>` becomes a read-only `Float32Array` in **0.02–0.045 ms**, against 80–94 ms for a copy;
   - JS buffers move to a worker through `ArrayBuffer.prototype.transfer()` in **0.02–0.03 ms**, and come back as the same memory;
   - a Rust `Vec` that JS filled is reclaimed without a copy.
   - Today's ABI `BYTES` path takes 315–652 ms for the same 64 MiB.
5. **Stores and events plug into R1's batch.**
   - A store write from any thread sets an atomic dirty bit and wakes once. The checkpoint folds dirty fields into the same dirty set, compare and `__svelters_changed` batch as native props.
   - Svelte sees them as reactive getters, and `bind:` lowers onto them because a store field is `exact` by construction.
6. **State survives hot reload when Rust owns it.**
   - Services, stores and `persist`ed objects outlive the JS runtime.
   - Handles are tagged with the runtime generation and released when that runtime goes.
   - JS-created objects die with their runtime unless they are persisted.
7. **Types:** the macros register a schema (`inventory`).
   - A debug host writes `svelters-exports/1` JSON, `app.d.ts` and the JS stubs on start, rewriting them only when they change. `svelters exports` does the same headless for CI.
   - The host serves the same JS as built-in modules, so stubs never go stale relative to the binary.
   - `svelters-compile` reads the JSON through `PlatformSemantics::native_type` (REACTIVITY §9.2) to type-check imports, observability and widget props.

## 1. What is different about an egui app

Every other backend takes app code as Svelte and native code as toolkit metadata: GIR, moc/qdoc, winmd, objc2. An egui app is a Rust crate. It embeds `svelters-host` and the egui backend, and its domain logic, kernels and custom widgets are Rust from the start.

- **No metadata to ingest:** the Rust source is the metadata. Proc macros see it with full types, which is the PyO3/napi-rs/UniFFI situation rather than the GJS/PySide one.
- **Cargo is already in the dev loop for Rust edits.** PLAN rule 5 still holds for `.svelte`/`.ts` edits: those must never rebuild Rust, so the JS stubs and typings for Rust exports must be byproducts of the Rust build, never inputs to it.
- **egui is immediate mode.** A custom widget is a `fn ui(&mut self, ui: &mut egui::Ui) -> Response`, not a native object with properties. The scene graph keeps the props, and the widget mutates them during the pass (§7).
- **egui's `Context` is `Send + Sync`,** and `request_repaint()` may be called from any thread (eframe installs the repaint callback). That makes it the natural `wake`.

## 2. Prior art

The versions below are current on crates.io as of 2026-10-21. They come from a survey that fetched the docs and read crate sources. Entries marked "unverified" weren't confirmed.

| Project | Surface | Threading / async | Binary data | TS / types | Best at | Links |
|---|---|---|---|---|---|---|
| **napi-rs** 3.14 | `#[napi]` on fn/struct/impl; `#[napi(object)]` records; `constructor`, `factory`, `getter`/`setter` | `Task` (`compute` on libuv pool, `resolve` on JS thread), `async fn` on tokio, `ThreadsafeFunction` (rewritten in v3), `AsyncTask::with_signal` (cancels only before compute starts) | owned (`Float32Array`, can cross await) vs borrowed (`&[u8]`, scope-bound); external `Vec`; docs warn `Send` "does not synchronize the shared bytes" | `.d.ts` from macro side effect (env-gated JSON lines) + CLI assembly, `--dts-cache` | ergonomics + typing | [class](https://napi.rs/docs/concepts/class), [async-task](https://napi.rs/docs/concepts/async-task), [tsfn](https://napi.rs/docs/concepts/threadsafe-function), [typed-array](https://napi.rs/docs/concepts/typed-array) |
| **wasm-bindgen** 0.2.129 + **tsify** 0.5.8 | `#[wasm_bindgen]` on struct/impl/fn/enum; `#[derive(Tsify)]` honours serde attrs | futures on the JS microtask queue (no `Send`); real threads only via wasm-bindgen-rayon + COOP/COEP | `Float32Array::view` is `unsafe` (any allocation can invalidate it) | `.d.ts` from wasm custom sections; tsify interfaces | TS from serde types | [weak refs](https://wasm-bindgen.github.io/wasm-bindgen/reference/weak-references.html), [futures](https://wasm-bindgen.github.io/wasm-bindgen/reference/js-promises-and-rust-futures.html), [tsify](https://github.com/madonoharu/tsify) |
| **Tauri** 2.12 + **specta/tauri-specta** 2.0-rc.25 | `#[tauri::command]` free fns only (RPC, no objects); `State<T>` | sync commands on main thread, async on tokio; **`Channel`** = ordered fast streaming (events are slow, they eval JS) | always ≥ 1 copy (out-of-process webview); raw `Response` | runtime export in debug builds writes `bindings.ts` | streaming channels; debug-time type export | [calling Rust](https://v2.tauri.app/develop/calling-rust/), [calling frontend](https://v2.tauri.app/develop/calling-frontend/), [tauri-specta](https://docs.rs/tauri-specta/2.0.0-rc.21/tauri_specta/) |
| **deno_core** 0.412 `#[op2]` | ops with `#[string]`/`#[buffer]`/`#[serde]`; `#[cppgc]` objects with ctor/getter/setter | async ops are **eagerly polled** `!Send` futures on the isolate thread; CPU work pushed out explicitly | `&[u8]` zero-copy (JS may mutate on re-entry); `JsBuffer` refcounted + `Send`; **`#[buffer(detach)]`** = transfer | hand-written `.d.ts` | eager polling, detach-on-transfer | [op2 README](https://github.com/denoland/deno_core/blob/main/ops/op2/README.md), [docs](https://docs.rs/deno_core/latest/deno_core/) |
| **rquickjs** 0.14 | `#[class]` + **one** `#[methods]` block per type; `#[qjs(get, set, constructor, static, rename)]`; `FromJs`/`IntoJs` derives for structs only; `#[module]` | `futures` feature: `!Send` `'js` futures driven only when the host calls `idle()`/`execute_pending_job`; `parallel` = locked runtime | `ArrayBuffer::from_source` / `_immutable` / `_shared` | none | external buffers | [repo](https://github.com/DelSkayn/rquickjs) (read from crate source) |
| **Neon** 1.1 (1.2-alpha) | `#[neon::export]` (+`json`, `task`); class macro in 1.2-alpha | `Channel::send` back to JS; `Deferred::settle_with`; async class methods **clone `self`** (lost writes) | `cx.lock()` borrow guard | none | post-back channel | [export](https://docs.rs/neon/latest/neon/attr.export.html), [Channel](https://docs.rs/neon/latest/neon/event/struct.Channel.html) |
| **UniFFI** 0.32 | `#[uniffi::export]` on fn/impl/trait; `Record`/`Enum`/`Object`/`Error` derives; `with_foreign` traits implemented by the foreign side | objects are `Arc<Self>: Send + Sync`, no `&mut self`; the foreign executor polls futures; no cancellation | copies | separate bindgen (JS via uniffi-bindgen-react-native) | foreign traits; one sound sharing model | [proc macros](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html), [foreign traits](https://mozilla.github.io/uniffi-rs/latest/foreign_traits.html), [ubrn](https://github.com/jhugman/uniffi-bindgen-react-native) |
| **PyO3** 0.30 | `#[pyclass(frozen, unsendable, …)]`, `#[pymethods]`, `#[getter]`/`#[setter]`, `#[staticmethod]` | `Python::detach` (was `allow_threads`) releases the GIL around Rust work; free-threaded builds turn borrows into runtime errors; `cancel_handle` | numpy borrow-checked views | pyo3-stub-gen (`inventory` + a stub binary) or built-in introspection | most complete attribute vocabulary | [class](https://pyo3.rs/v0.30.1/class.html), [free-threading](https://pyo3.rs/v0.30.1/free-threading.html) |
| **Rustler** 0.39 | `#[rustler::nif(schedule = "DirtyCpu")]`, `ResourceArc` | dirty schedulers; `OwnedEnv::send_and_clear` from own threads (panics on a BEAM thread) | `make_binary` borrows resource memory | none | "message the result back" | [nif](https://docs.rs/rustler/latest/rustler/attr.nif.html), [OwnedEnv](https://docs.rs/rustler/latest/rustler/env/struct.OwnedEnv.html) |
| **Boa** 0.22 | `Class` trait; `#[boa_class]` on impl; `TryFromJs`/`TryIntoJs` derives | `JobExecutor` for custom loops | `JsArrayBuffer::from_byte_block` | none | — | [class](https://docs.rs/boa_engine/latest/boa_engine/class/index.html) |
| **ts-rs** 12 / **specta** 2.0-rc | `#[derive(TS)]` / `#[derive(Type)]`, serde-compatible attrs | — | — | ts-rs: test-time export; specta: runtime registry + exporters; 64-bit ints → `bigint` by default | type emission | [ts-rs](https://docs.rs/ts-rs/latest/ts_rs/), [specta](https://github.com/specta-rs/specta) |
| **cxx-qt** 0.10 / **Slint** 1.18 | `#[qobject]`, `#[qproperty]`, `#[qsignal]` / `.slint` properties | `CxxQtThread::queue(FnOnce + Send)`; Slint `invoke_from_event_loop`, `Weak::upgrade_in_event_loop` (skips silently if gone) | — | — | Rust state pushed into a declarative UI from workers | [cxx-qt](https://kdab.github.io/cxx-qt/book/bridge/extern_rustqt.html), [Slint](https://docs.slint.dev/latest/docs/rust/slint/fn.invoke_from_event_loop) |

**Lessons this design takes:**
1. **Split compute from resolve** (napi `Task`). The worker part is `Send` and has no JS access; the JS part runs on the UI thread. The macro generates the split.
2. **The post-back handle is typed, `Send`, weak, and fails softly** (Slint `upgrade_in_event_loop`, cxx-qt `queue().ok()`). A completion for a dead object or runtime is dropped, never panics.
3. **Drop JS-bound things on the JS thread.** Neon routes unsettled `Deferred`s to a drop queue; rquickjs has `pending_free` under `parallel`, and a `Persistent` that outlives its runtime aborts the process. User code never holds a `Persistent`.
4. **Three explicit buffer modes** (Deno `detach`, rquickjs `_immutable`): borrow (sync only), move (transfer), shared read-only. Never "`Send` but JS may still write".
5. **One sharing model enforced by types:**
   - UniFFI's `Arc<Self>: Send + Sync` for anything a worker touches;
   - UI-only objects may be `!Send`;
   - no implicit `self` clones for async methods (Neon).
6. **Eager-poll local async** (Deno), and batch completions per checkpoint.
7. **AbortSignal plus a Rust-side token** (napi, PyO3). Document that only cooperative code can be interrupted.
8. **Emit types from the build, robustly.** napi's proc-macro file side effects break under incremental builds, and tauri-specta stalled outside the core. We use runtime registration (specta/pyo3-stub-gen style) dumped by the dev host, and keep it in-tree.

## 3. Layer decision: host registry, not the C ABI, not rquickjs classes

### 3.1 Options

| | A. Backend C ABI (a Rust code pack in the egui backend, `svl_value`) | B. rquickjs `#[class]` directly | **C. Host registry (recommended)** |
|---|---|---|---|
| Engine-agnostic (`JsEngine`, JSC, N-API TestHost) | yes | **no**: rquickjs types in user code | yes: Rust values, engine converts |
| Works on GTK/Qt/AppKit/WinUI | only on the backend that hosts the pack | yes (same engine) | **yes**: host-level |
| Handle identity, `dispose`, `StaleHandleError`, SHARED, op log, TestHost replay | yes | no (JS objects own Rust data) | yes (host handle table) |
| Reactivity (`observe`, batch) | yes | hand-rolled | yes (stores feed the dirty set) |
| Hot reload | handles in backend; OK | **instances die with the runtime**; `Trace` + `Persistent` aborts if one outlives it | Rust state in host survives; handles re-minted |
| Per-call cost (measured, §4) | 2.9–4.0 µs | 0.72–1.04 µs | between: no C shim/raw `svl_value` build; not measured |
| Zero-copy buffers | needs an ABI change (`BYTES` copies out) | yes | yes (new `Value::Buffer`, engine-implemented) |
| TS typing | from descriptors | none (would need our own) | from the registry schema |
| Multi-impl blocks, enums, traits | ours | one `#[methods]` block; struct-only derives | ours |

- **Why not A.**
  - The C ABI exists so a C++ or C backend can serve toolkit objects; a user's `Simulation` is neither.
  - Going through `svl_backend` costs a raw `svl_value` tree per call, a vtable shim, and `describe` descriptors built from strings.
  - It would make an exported type a property of one backend, while a `Simulation` should work under GTK too.
- **Why not B.**
  - It is the fastest path. But it puts rquickjs lifetimes (`'js`, `Trace`, `Persistent`) into app code.
  - Instances die with every hot reload.
  - It bypasses the handle table, so identity, `dispose`, the op log and the R1 batch would all need a second implementation.
  - QUICKJS-RUNTIME item 13 already rejected it for widgets for the same reason.

### 3.2 The recommended shape

```text
 Svelte/JS ── import { Simulation } from 'svelters:app/sim'   (built-in module, served by the host)
     │  @svelters/types core: NativeObject, call, callAsync, observed, on   (unchanged)
     ▼
 __svelters_host.{construct, call, callStatic, getProp, setProp, listen, observe, cancelAsync}
     │  host routes by handle range / kind prefix:
     ├── backend handles / kinds ───────────▶ svl_backend (C ABI)      GTK, Qt, egui widgets, …
     └── `app:` kinds, app handle range ────▶ svelters-host::exports::Registry (Rust)
                                                 ├ thunks generated by #[svelters::export]
                                                 ├ objects: Rc<RefCell<T>> (ui) | Arc<T> (shared)
                                                 ├ completion queue  ◀── worker threads (Send)
                                                 └ stores (dirty bits) ◀── any thread
```

- **Handles.** The registry mints handles in a range the backend never uses (bit 63 set). `handle_info` answers `SVL_LIFE_SHARED`. Releases arrive through the existing FinalizationRegistry → `queueRelease` path at the checkpoint.
- **Kinds** are named `app:<module>.<Type>`. Ids sit in a reserved pack index (CUSTOM-WIDGETS §3.2: pack 63), so `resolve`, the name caches and the op log work unchanged.
- **Values.**
  - Owned `svelters_abi::Value`, plus one new variant `Value::Buffer(Buffer)` (§5.4). C backends never see it; on the raw path it degrades to a `BYTES` copy.
  - Descriptors are the owned `MemberDesc`/`TypeDesc`, so `convert.rs` validates arguments exactly as for backends, including async + `{ signal }`.
- **Engine.** `convert.rs`'s `JsReader`/`JsWriter`/`JsPromises` gain buffer operations: `external_buffer(src, immutable)`, `transfer(v) -> TransferredBuffer`, `reclaim`. QuickJS implements them with `ArrayBuffer::from_source*`, `transfer()` and `detach` (probe Part A). A JSC or N-API engine implements the same three.
- **Fast path, later and only if measured.** Under a `quickjs-fast` feature, the macro could also emit direct rquickjs functions for scalar-only `#[js(fast)]` methods. Nothing in the API would change. Don't build it until an app shows a chatty hot path. The guidance in §5.1 (coarse calls, Rust owns the data) makes it rarely matter.

## 4. Probe results (measured)

`docs/notes/scripts/egui-exports-probe`, release build, rquickjs 0.14.0 (QuickJS-NG 0.16.2), rayon 1.12, on WSL2 with 16 cores.

**Caveat: the machine was heavily loaded by other agents' builds** (load average 11–14 during the runs), so absolute times are pessimistic and noisy. The ranges are three to four runs. Ratios between rows within one run are the reliable part.

| Measurement | Result |
|---|---|
| plain JS function call (QuickJS) | 234–325 ns |
| rquickjs `#[class]` method `sim.add(1)` (`&mut self`, f64) | 721–1041 ns |
| rquickjs `#[class]` field getter | 676–874 ns |
| `host.call(h, 'add', [1])` through `svelters-host` + C ABI to a Rust `Backend` (descriptor F64 → F64) | 2.9–4.0 µs |
| zero-copy `Float32Array` over a 64 MiB `Arc<[f32]>` (`from_source_immutable`) | **0.019–0.045 ms** |
| `TypedArray::new_copy` of the same 64 MiB | 80–94 ms |
| 64 MiB `SVL_BYTES` result through the ABI (Rust `Vec` + JS copy) | 315–652 ms |
| writing `view[0] = 42` on the immutable view (strict code) | **silently ignored, no throw**; value unchanged |
| `Arc` strong count with a JS view alive / after the view is dropped + GC | 2 / 1 (the `Arc` is released on GC) |
| `ArrayBuffer.prototype.transfer()` of a JS-allocated 64 MiB buffer | **0.024–0.031 ms** (pointer reused; the source is detached) |
| `scaleAsync(a, k)`: transfer + raw pointer + `rayon::spawn`, returning a Promise (2nd call; the 1st includes rayon pool start-up, 5–15 ms) | 0.14–0.16 ms in three runs, 5.3 ms in one |
| completion on the JS thread: `resolve(new Float32Array(sameMemory))` | 0.018–0.027 ms |
| the job drain after that resolve | 16–28 ms, of which the reaction itself took 0.03 ms. **Not attributed;** most likely a QuickJS GC cycle triggered by the many 64 MiB allocations in the script, not the completion path |
| result after the worker kernel: `out[0]`, input length | `6` (2 × 3) and `0`: the same memory went JS → worker → JS, and the input is detached |
| Rust `Vec` filled by JS, reclaimed by Rust (detach → our drop hook) + Rust sum of 16 Mi f32 | 45–58 ms total, no copy; JS length afterwards 0 |
| JS loop summing 16 Mi f32 (view or copy, same) | 4.5–5.3 s (the interpreter: never do this in JS) |
| async noop on rayon through the host (`SVL_M_ASYNC` + CALLBACK), sequential `await` | 94–296 µs per round trip (two thread hops on a loaded box) |
| 10,000 async noops in flight (`Promise.all`) | 523–605 ms, i.e. 52–61 µs per op |
| host `invoke_callback` (settling one completion), Rust side | 19.5–23.8 µs average |
| `await step(200)`: 200 ms of CPU on a worker, with 1 ms tick events posted to JS meanwhile | resolves in 200.5 ms; **156–166 ticks delivered** (the ticker sleeps ≈1.2 ms), so the JS thread stayed free |
| 4 × `step(200)` in parallel | 200.9–207.7 ms |

**What follows:**
1. **Per-call cost.** The ABI path costs 3.5–5.5x an rquickjs method, which costs about 3x a JS call. It is in the same range as REACTIVITY §6's `getProp` (≈4 µs). A UI that makes hundreds of calls per frame is fine; one making 100k is not. Steer users to coarse-grained calls, and leave the direct fast path (§3.2) as an option.
2. **Large data must never go through `Value::Bytes`.** The external-buffer and transfer paths make 64 MiB effectively free on QuickJS-NG; the ABI's copy-out costs 3–8x a single copy. That justifies the new `Value::Buffer` variant.
3. **Settling a completion costs ≈20 µs in today's host** (`invoke_callback` → decode → a JS completion function), and that dominates the 10k-in-flight number.
   - The registry's completion queue should settle Promises directly from the engine (`resolve` was 0.02 ms in Part A, including the typed-array wrap). It should skip the CALLBACK round trip and settle a whole batch in one engine entry.
   - Projection, not measured: about 1–3 µs per completion.
4. **Immutable views are read-only in effect but not loudly.** Generated typings must mark them read-only (§6.3), because a write fails silently.
5. **The worker pattern works with today's ABI.** `SVL_M_ASYNC` + a one-shot CALLBACK + a thread-safe `wake` gave Promises resolved on the JS thread, with the UI thread free during compute. The registry is an optimisation and a home for the semantics, not a requirement for correctness.

## 5. The macro surface

One facade crate, `svelters`, holds the macros (`svelters-macros`) and the runtime types (`svelters-host::exports`). Everything below is a proposal; type and attribute names are open to bikeshedding.

### 5.1 Design rules

- **JS orchestrates, Rust owns the data.** Large state stays in Rust: worlds, buffers, models. JS holds handles, gets snapshots and zero-copy views, and calls coarse operations. Rust widgets read Rust state directly (§7.3).
- **The thread a call runs on is visible in the signature:**
  - plain `fn`: sync, JS thread;
  - `#[js(worker)]`: rayon;
  - `async fn` (= `#[js(local)] async fn`): the JS thread, eagerly polled (decided 2026-10-21, §11);
  - `#[js(tokio)] async fn`: tokio (feature `tokio`);
  - `#[js(blocking)]`: blocking pool;
  - `#[js(local)] async fn`: JS thread, eagerly polled.

  Every non-sync form returns `Promise<T>` and takes a trailing `{ signal }`, the NT1 convention.
- **The compiler checks the thread rules.**
  - A worker or tokio entry point requires `Self: Send + Sync` (a `shared` object) and `Send + 'static` arguments.
  - UI objects, `JsFn` and borrowed slices are rejected there with a targeted error message.
- **Names follow the generated toolkit modules.** Rust `snake_case` becomes JS `camelCase` for members and `PascalCase` for types. Unit enum variants become kebab-case strings, as GTK enums are today. `#[js(rename = "…")]` overrides.

### 5.2 Rust side

```rust
// my-app/src/sim.rs
use svelters::prelude::*;                       // Record, Enum, Error, object, export, store, Buffer, Shared, WorkCx, Emitter…
use std::sync::{Arc, RwLock};

/// Plain data, copied across. Doc comments become TS doc comments and `svelters docs` cards.
#[derive(svelters::Record, Clone, Debug)]
pub struct SimParams {
    pub gravity: f32,
    pub damping: f32,
    #[js(default = 4096)]
    pub particles: u32,
    pub integrator: Integrator,
}

#[derive(svelters::Enum, Clone, Copy, Debug)]
pub enum Integrator { Euler, Verlet, Rk4 }       // 'euler' | 'verlet' | 'rk4'

#[derive(svelters::Record, Clone)]
#[js(tag = "kind")]                              // data enum → discriminated union
pub enum Source { Point { x: f32, y: f32 }, Disc { x: f32, y: f32, r: f32 } }

#[derive(svelters::Record)]
pub struct StepReport { pub steps: u64, pub energy: f64, pub wall_ms: f64 }

/// Errors reject/throw as NativeError { domain: 'app:sim', code: 'too-many' | 'diverged', message }.
#[derive(svelters::Error, thiserror::Error, Debug)]
pub enum SimError {
    #[error("too many particles: {0}")] TooMany(u32),
    #[error("diverged at step {step}")] Diverged { step: u64 },
}

/// Events: `sim.on('diverged', e => …)`. Any thread may emit.
#[derive(svelters::Events, Clone)]
pub enum SimEvent { Diverged { step: u64 }, Checkpoint { step: u64 } }

/// A handle type workers may use: stored as Arc<Self>; the macro asserts Send + Sync.
#[svelters::object(shared, module = "sim")]
pub struct Simulation {
    world: RwLock<World>,
    stats: Store<SimStats>,                      // reactive mirror, see below
    events: Emitter<SimEvent>,
}

#[svelters::export]
impl Simulation {
    #[js(constructor)]
    pub fn new(params: SimParams) -> Result<Self, SimError> {
        if params.particles > 1_000_000 { return Err(SimError::TooMany(params.particles)); }
        Ok(Self { world: RwLock::new(World::new(&params)), stats: SIM_STATS.clone(), events: Emitter::new() })
    }

    /// Sync, JS thread: cheap reads only.
    #[js(getter)]
    pub fn particle_count(&self) -> u32 { self.world.read().unwrap().len() as u32 }

    pub fn reset(&self, params: SimParams) { *self.world.write().unwrap() = World::new(&params); }

    /// Runs on the rayon pool. JS: `step(n, { signal, onProgress }) → Promise<StepReport>`.
    /// `cx` is injected (not a JS argument): cancellation, progress, events.
    #[js(worker)]
    pub fn step(self: Arc<Self>, n: u32, cx: WorkCx<f32>) -> Result<StepReport, SimError> {
        let t0 = std::time::Instant::now();
        let mut w = self.world.write().unwrap();
        for i in 0..n {
            cx.check()?;                         // Err(Cancelled) → rejects 'cancelled'
            w.integrate();
            if !w.energy().is_finite() {
                self.events.emit(SimEvent::Diverged { step: w.step });
                return Err(SimError::Diverged { step: w.step });
            }
            if i % 64 == 0 { cx.progress(i as f32 / n as f32); }   // latest-value, coalesced per checkpoint
        }
        self.stats.update(|s| { s.step = w.step; s.energy = w.energy(); });
        Ok(StepReport { steps: w.step, energy: w.energy(), wall_ms: t0.elapsed().as_secs_f64() * 1e3 })
    }

    /// Zero-copy, read-only snapshot. Shared<[f32]> is Arc<[f32]>; JS gets an immutable Float32Array.
    pub fn positions(&self) -> Shared<[f32]> { self.world.read().unwrap().positions_snapshot() }

    /// Streaming: JS `for await (const f of sim.run(600, { signal }))`. Bounded (backpressure).
    #[js(worker, buffer = 4)]
    pub fn run(self: Arc<Self>, frames: u32, out: Sink<Frame>, cx: WorkCx) -> Result<(), SimError> {
        for _ in 0..frames {
            let f = { let mut w = self.world.write().unwrap(); w.integrate(); w.frame() };
            out.send(f)?;                        // blocks while 4 frames are unread; Err(Closed) on break/abort/reload
            cx.check()?;
        }
        Ok(())
    }
}

/// Free function on the pool; `samples` is *moved* from JS (its ArrayBuffer is transferred, zero-copy).
#[svelters::export(module = "sim", worker)]
pub fn spectrum(samples: Buffer<f32>, window: Window) -> Buffer<f32> {
    let mut s = samples;                         // exclusive: JS's array is detached until we return it
    fft_in_place(&mut s, window);
    s                                            // handed back as the same memory
}

/// I/O on tokio. Futures must be Send + 'static (checked).
#[svelters::export(module = "sim")]
pub async fn load_scene(path: String) -> Result<SimParams, SceneError> {
    let text = tokio::fs::read_to_string(&path).await?;
    Ok(toml::from_str(&text)?)
}

/// UI-thread object: may hold !Send state, gets &mut self. No worker/async methods allowed.
#[svelters::object(module = "sim")]
pub struct Recorder { frames: Vec<Frame>, encoder: Option<Box<dyn Encoder>> /* !Send ok */ }

#[svelters::export]
impl Recorder {
    #[js(constructor)] pub fn new() -> Self { Self { frames: vec![], encoder: None } }
    pub fn push(&mut self, f: Frame) { self.frames.push(f); }
    #[js(getter)] pub fn len(&self) -> u32 { self.frames.len() as u32 }
}

/// A second impl block on the same type is fine (rquickjs allows one).
#[svelters::export]
impl Recorder {
    #[js(static)] pub fn supported_codecs() -> Vec<String> { vec!["png".into(), "webm".into()] }
}

/// Reactive state, written from any thread, read reactively in Svelte (§6.4).
#[svelters::store(module = "sim")]
#[derive(Default, Clone)]
pub struct SimStats {
    pub step: u64,
    pub energy: f64,
    pub running: bool,
    #[store(writable)]                          // JS may write it (bind:value); on_write validates
    pub speed: f32,
}
pub static SIM_STATS: StoreStatic<SimStats> = StoreStatic::new();   // one app-wide instance, survives reload

/// JS implements a Rust trait (UniFFI "foreign trait"): calls from the UI thread are synchronous;
/// from a worker they go through the UI queue and return a future.
#[svelters::export(module = "sim", foreign)]
pub trait ForceField: Send + Sync {
    fn force(&self, x: f32, y: f32) -> [f32; 2];
}

#[svelters::export]
impl Simulation {
    /// JS: sim.setField({ force: (x, y) => [0, -9.8] }) or a class instance.
    pub fn set_field(&self, f: Arc<dyn ForceField>) { self.world.write().unwrap().field = Some(f); }
}
```

Notes on the surface:

- **`WorkCx<P>`** is injected, never a JS argument. It carries:
  - `check() / is_cancelled()`;
  - for async, `cancelled().await`;
  - `progress(P)`, latest-value-wins and delivered once per checkpoint to `options.onProgress`;
  - `emit` for the owning object's events.

  `P` defaults to `()`. Without a `WorkCx` parameter, a call can still be aborted before it starts, and its result is dropped after an abort.
- **`self: Arc<Self>`** on worker methods makes the clone explicit and shares the state. No hidden copy of `self` (Neon's pitfall).
- **`foreign` traits implemented in JS** are `Arc<dyn Trait>` proxies.
  - On the UI thread a call is synchronous: it reuses `invoke_callback`, with re-entrancy rules as today.
  - On a worker the call is posted to the UI queue and blocks the worker until the JS thread answers. If it is called during a checkpoint that waits on that worker, it would deadlock, so worker calls carry a timeout and the docs steer towards `async` variants (`async fn force(...)`).
  - **Treat this as the least important item.** Callbacks into JS from hot compute loops are an anti-pattern: 20 µs per settle (§4), plus the interpreter.
- **Serde escape hatch:** `Json<T>` with `T: Serialize + DeserializeOwned` crosses as a structured value typed `unknown`, or typed through `specta::Type` when the `specta` feature is on. That lets existing domain types cross without new derives.

### 5.3 Type mapping (Rust → JS → TS)

| Rust | JS value | TS | Notes |
|---|---|---|---|
| `bool`, `i8..i32`, `u8..u32`, `f32`, `f64` | number/boolean | `number`/`boolean` | range-checked like `SVL_D_INTEGRAL` today |
| `i64`, `u64`, `usize` | number | `number` | checked to ±2^53; `#[js(bigint)]` → `bigint` (`SVL_D_BIGINT`). Explicit, because ts-rs's bigint default is a known footgun |
| `String`, `&str` (sync only), `Cow<str>` | string | `string` | |
| `Option<T>` | value or `null` | `T \| null`; optional when trailing | |
| `Vec<T>`, `[T; N]`, tuples | array | `T[]`, `[A, B]` | copied |
| `HashMap<String, T>`/`BTreeMap` | object | `Record<string, T>` | |
| `#[derive(Record)] struct` | plain object | `interface` | copied; `#[js(default)]` → optional field |
| `#[derive(Enum)]` unit | string | string-literal union | kebab-case |
| `#[derive(Record)] enum` | object with tag | discriminated union | `#[js(tag = "kind")]`, serde-like |
| `Result<T, E: Into<NativeError>>` | value / throw / reject | `T` | `#[derive(svelters::Error)]` → codes from variants |
| `SystemTime`, `chrono::DateTime` | `Date` | `Date` | `SVL_T_DATETIME` |
| `egui::Color32`, `Vec2`/`Pos2`, `Rect` | records | `Color`, `Point`, `Rect` from `values.d.ts` | shared shapes (NON-GUI-TYPES §3.1 record shapes) |
| `&[f32]` (sync fns only) | typed array or number[] | `Float32Array \| ArrayLike<number>` | borrowed for the call, zero-copy for typed arrays (as `SVL_BYTES` inbound today) |
| `Vec<f32>` | number[] | `number[]` | copied (napi convention) |
| `Buffer<T>` (T = u8…f64) | typed array | `Float32Array` etc. | **moved**: in → JS buffer transferred and detached; out → owned ArrayBuffer over the Rust allocation |
| `Shared<[T]>` (= `Arc<[T]>`), `bytes::Bytes` | immutable typed array | `ReadonlyFloat32Array` (generated interface with `readonly [i: number]: number`) | zero-copy, `from_source_immutable`; `Arc` freed on GC |
| `Arc<O>`/`&O` for an exported object | wrapper | `class O` | identity: same `Arc` → same JS object |
| `Store<S>` | store object | `S` with readonly/writable fields | reactive getters |
| `Arc<dyn ForeignTrait>` | object with methods | `interface` | JS-implemented |
| `JsFn<(A,), R>` (UI only) | function | `(a: A) => R` | `!Send`; scope `call`/`notified` as `SVL_CB_*` |
| `ThreadsafeFn<A>` | function | `(a: A) => void` | `Send`; posts to the UI queue, fire-and-forget |
| `Sink<T>` param / `Stream<T>` return | async iterable | `AsyncIterable<T>` | bounded |
| `Emitter<E>` field | — | `on(name, fn)` overloads | from `#[derive(Events)]` |

### 5.4 `Value::Buffer`

```rust
pub enum Value { /* …existing… */ Buffer(Buffer) }       // owned model only; no new svl_value tag

pub struct Buffer {
    pub elem: Elem,                                       // U8, I8, U16, …, F32, F64
    pub data: BufData,
}
pub enum BufData {
    Owned(Box<dyn BufSource>),                            // JS gets exclusive access (from_source)
    Shared(Arc<dyn BufSource>),                           // JS gets an immutable view (from_source_immutable)
    Transferred(TransferToken),                           // a JS buffer moved out; engine-owned memory, pinned until returned/dropped on the JS thread
}
```

- **Raw `svl_value` encoding:** `BYTES` (a copy). Only Rust backends and the registry see the variant itself, so the C ABI doesn't change.
- **`TransferToken`** wraps an engine-specific record: for QuickJS, a `Persistent<ArrayBuffer>` created by `transfer()`, kept in a host table, plus the raw pointer.
  - The token is `Send`, but its `Drop` posts to the JS thread's release queue. The memory belongs to the JS allocator and must be freed there.
  - Lesson 3: the worker never frees it.
  - Returned from a worker as a `Buffer`, it becomes the same ArrayBuffer again (probe: `out[0] = 6`, `input length 0`).

## 6. Threading and compute offload

### 6.1 Execution classes

| Declared as | Runs on | `Self`/args requirement | JS result | Cancellation |
|---|---|---|---|---|
| `fn` (default) | JS thread, synchronously | any; `&mut self` on ui objects | value / throw | n/a |
| `#[js(worker)] fn` | rayon (the app's pool or global) | `self: Arc<Self>` (shared), args `Send + 'static` | `Promise<T>` | queued → skipped; running → `cx.check()` |
| `#[js(tokio)] async fn` (feature `tokio`) | tokio (app's runtime or one the host extension owns) | future `Send + 'static` | `Promise<T>` | future dropped at the next `.await` (`JoinHandle::abort`) |
| `#[js(blocking)] fn` | tokio `spawn_blocking` or a dedicated pool (file/DB I/O) | as worker | `Promise<T>` | cooperative |
| `async fn` = `#[js(local)] async fn` (the default, §11) | JS thread; polled eagerly at call, then on wake at checkpoints | `!Send` ok; may await JS promises (`JsPromise<T>`) | `Promise<T>` | dropped |
| `#[svelters::service]` | a long-lived thread or task started at app start | `Send` | a store/handle exported to JS | stopped at shutdown, **not** at reload |

There are two compile errors, each with a fix-it message:
- "`#[js(worker)]` on `Recorder`: worker methods need `#[svelters::object(shared)]` and `Self: Send + Sync`";
- "argument `cb: JsFn<…>` can't cross to a worker; use `ThreadsafeFn` or return a stream".

### 6.2 The call → worker → Promise path

```text
JS thread                                   host (svelters-host::exports)                      worker (rayon/tokio)
─────────                                   ─────────────────────────────                      ────────────────────
sim.step(1000, {signal, onProgress})
  └ callAsync → host.call ──────────────▶  convert args (descriptor-driven) to owned Send values
                                           op = Op { id, gen, cancel: CancelToken, progress: Slot }
                                           promise ← engine.new_completion(id)
                                           pool.spawn(job) ─────────────────────────────────▶  catch_unwind(|| thunk(args, cx))
  ◀── Promise (pending)                                                                          cx.progress(p) → slot.store(p); wake_once()
… JS keeps running, frames keep painting …                                                       return Ok(v) / Err(e) / panic
                                           completer.push(id, gen, Result<Value, NativeError>) ◀─┘
                                           wake_once() → backend.wake() → egui ctx.request_repaint()
checkpoint (start of next frame):
  1. drain completions (budget: ≤ 4 ms or 1024 per checkpoint, rest → re-wake)
       gen ≠ current → drop (reload), op cancelled → drop
       v → JS value (buffers zero-copy) → settle promise directly in the engine (no CALLBACK)
     drain progress slots → onProgress(latest) once
  2. stores + backend prop_changed → dirty set → __svelters_changed batch      (R1, unchanged)
  3. queued signal events (incl. Emitter events)                                (as today)
  4. one job drain: Svelte flushes once                                         (R1 order)
```

- **Where the queue lives.** The completion queue is a host structure: `Arc<Mutex<VecDeque>>` plus an `AtomicBool` "wake pending", so a burst of 10k completions costs one wake. It is not a backend structure, so the same exports work on GTK (`g_main_context_wakeup` via `wake`) and Qt.
- **Threads.** The host still starts no threads (`lib.rs` doc): the pools belong to the export runtime, which the app configures with `svelters::App::builder().workers(n).tokio(handle)`. PLAN rule 6 is respected, because worker threads compute but never time anything; timers stay on the toolkit.
- **Where completions are settled.** The probe settled them in the backend loop with `invoke_callback`, which works today but measured ≈20 µs each. The registry settles them at checkpoint start, with the promise's resolve function held in the engine's completion table.
- **Eager polling.** A `local` future that is ready at call time resolves in the same turn (Deno lesson 6). Worker results always take one checkpoint, which bounds latency to one frame.

### 6.3 Moving data

| Need | Use | Cost (measured where available) |
|---|---|---|
| small structured data | `Record`s, `Vec`, `String` | copy; fine below ~100 KB |
| Rust → JS, large, read-only (snapshots, model data, plot series) | `Shared<[T]>` / `bytes::Bytes` | 0.02–0.045 ms for 64 MiB; `Arc` released on GC (probe) |
| Rust → JS, large, JS may mutate | `Buffer<T>` (owned) | external, no copy (`from_source`) |
| JS → worker, large, in-place kernel | `Buffer<T>` param: the host calls `transfer()`; JS's view is detached until the result returns | 0.02–0.03 ms for 64 MiB; round trip returns the same memory (probe) |
| JS → Rust, reuse a Rust allocation | `svelters.alloc(Float32Array, n)` (a Rust-backed buffer), later passed as `Buffer<T>` → reclaimed by detach | no copy (probe: reclaim + sum 45–58 ms, dominated by the sum) |
| JS → sync call, borrowed | `&[T]` | zero-copy for the call's duration (today's `SVL_BYTES` borrow) |
| Rust worker → Rust widget, never through JS | `Store<S>` field or `Arc<ArcSwap<…>>` read in `Widget::ui` | none; JS only gets the structure |

- **Typing.**
  - Moved parameters get a TS doc tag: `@param samples transferred: the array is detached until the Promise settles`.
  - Immutable views are typed with generated `ReadonlyFloat32Array` interfaces, because writes are silently dropped at runtime (probe).
- **`SharedArrayBuffer` is deliberately not offered.** QuickJS has it, but racy shared mutable memory between a worker and JS gives up the main safety property. Revisit if a real case needs it.

### 6.4 Stores: Rust state that Svelte reads like `$state`

> **As built (E3):** see `plans/milestones/E3-exports-reactive.md`. The granularity is a dirty bit per field (§11 Q7, decided 2026-10-22). Differences from the sketch below: writes are `store.update(|s| ..)` (every field), `store.write(Field::X, |s| ..)` / `write_fields(&[..], ..)` (named fields) with a generated `<Name>Field` enum instead of per-field setters; `StoreHooks::on_write(&mut self, field)` runs after the assignment (it may normalize, or return `Err` to restore the old value and throw); a `StoreStatic` reaches JS through `#[svelters::export(module = "m")]` on the `static`; services start with the process's first `Host` (there is no `App::run`).

```rust
// any thread:
SIM_STATS.update(|s| { s.step += 1; s.energy = e; });   // write lock, mark all bits dirty, wake once
SIM_STATS.set_running(true);                            // generated per-field setter, marks one bit
// JS-originated write (writable field):
impl StoreHooks for SimStats { fn on_write(&mut self, field: Field, v: &Value) -> Result<(), NativeError> { /* clamp speed */ Ok(()) } }
```

- **Inside.** A `StoreCell<S>` holds an `RwLock<S>`, an `AtomicU64` dirty mask per 64 fields, and a weak link to the host's waker.
  - Writers never call into the host. They set bits and call `wake()` only on a 0→nonzero transition, the same "one wake per turn" rule as `prop_changed`.
  - If no host is attached yet (a service running before the window opens), the bits simply accumulate.
- **At the checkpoint** (step 2 above), for each store with dirty bits:
  1. take the mask;
  2. clone the dirty fields under a read lock;
  3. insert `(storeHandle, field)` into the R1 dirty set with the value already read. Proportional fields (`#[store(proportional)]`, e.g. a log `String`) are invalidation only.

  Compare-with-last drops unchanged values, so `update` may mark everything.
- **The result:** store changes, toolkit property changes and events reach JS in **one batch, one drain** (REACTIVITY §5.3). REACTIVITY measured 10k changes in one checkpoint at 0.2 ms with 2 effect runs; per-event delivery would be ~1 s.
- **In JS** the store is a `NativeObject` whose getters are `observed(this, 'energy')`, so it is reactive in effects and templates with no `$state` mirror. Writable fields use `setProp` → registry → `on_write` → read-back. R1's set path delivers a clamped value once and drops the echo.
- **In the compiler,** store fields are `exact` observability by construction. `<slider bind:value={simStats.speed} />` lowers to `bind_prop` (R2), and `bind:` on a read-only field is the existing `set_only`/read-only diagnostic.
- **Services:**

  ```rust
  #[svelters::service(module = "telemetry")]
  fn telemetry(cx: ServiceCx) -> Store<Telemetry> {
      let store = Store::new(Telemetry::default());
      let s = store.clone();
      cx.spawn(async move {                        // tokio; cancelled at shutdown only
          let mut sock = connect().await?;
          while let Some(sample) = sock.next().await { s.update(|t| t.push(sample)); }
          Ok(())
      });
      store
  }
  ```

  JS does `import { telemetry } from 'svelters:app/telemetry'`. A service starts once per process, at `App::run` or lazily on first import (`#[svelters::service(lazy)]`). It is the same Rust object after every hot reload.

### 6.5 Composition with egui's repaint

- **`wake` = `ctx.request_repaint()`.** It is thread-safe. `schedule_wake(ms)` = `request_repaint_after(ms)` (`backend-abi.md`). An idle app is idle: no polling and no timer thread.
- **The egui frame order**, as this design needs it:
  1. the backend's update callback runs `host.checkpoint()`: completions, stores, events, JS, ops into the retained scene;
  2. the backend renders the scene with `ui()` per node;
  3. once the `Ui` borrows end, widget-originated prop changes and events are handed to the host. This is the SVR14 re-entrancy rule: no JS while a `Ui` is borrowed.
  4. if any were queued, `request_repaint()`.

  A worker result is on screen in the frame after it completes.
- **High-rate data skips JS.** A Rust plot widget reads `Shared<[f32]>` or a store field inside `ui()`, so a worker can drive a 60 fps visualisation while JS changes only structure. This is the main performance pattern, and why §6.3's last row exists.
- **Backpressure against frame time.** The completion budget (4 ms per checkpoint) and progress coalescing keep a flood of worker results from stalling frames. Streams apply backpressure to the producer through the bounded `Sink`.

### 6.6 Streams, events, progress, backpressure

- **`Sink<T>` → `AsyncIterable<T>`.**
  - Capacity `buffer = N` (default 8). `send` blocks on rayon or awaits on tokio when full.
  - At a checkpoint the host hands JS every queued item.
  - Exiting a `for await` early (`return()`), an abort, or a reload closes the stream, and the producer's next `send` returns `Err(Closed)`.
  - `stream.batches()` yields arrays per checkpoint for bulk consumers.
- **`Emitter<E>`.**
  - Events from any thread are queued in order per object and delivered as signal events to `obj.on('diverged', fn)` listeners, which are scope-bound disposers exactly as `NativeObject.on`.
  - `#[event(latest)]` coalesces an event to its last occurrence per checkpoint.
  - A dev warning fires above 1000 events per checkpoint per object.
- **Progress** is a latest-value slot, at most one `onProgress` call per checkpoint. A worker can report a million times for the cost of an atomic store.
- **Superseding.** `#[js(worker, supersede)]` cancels the previous in-flight call of the same method on the same object when a new one starts, and rejects it with code `superseded`. This is "recompute the preview while the slider drags", which every UI with offload needs; similar to RxJS `switchMap`.
- **Concurrency caps.** `#[js(worker, max_in_flight = 2)]` puts further calls in a FIFO; a semaphore lives in the op table.

### 6.7 Errors and panics

| Source | JS sees |
|---|---|
| `Err(e)` with `#[derive(svelters::Error)]` | `NativeError { domain: 'app:<module>', code: '<kebab variant>', message: Display }` (+ fields as `details`) |
| `anyhow::Error`/`Box<dyn Error>` | `NativeError { domain: 'app:<module>', code: 'error', message }` |
| panic (sync thunk, worker job, tokio task) | `NativeError { domain: 'svelters', code: 'panic', message, stack (dev: Rust backtrace) }`; caught with `catch_unwind` |
| a `shared` object whose lock was poisoned by a panic | later calls reject with `code: 'poisoned'` |
| a `ui` object that panicked inside `&mut self` | marked poisoned; later calls throw `poisoned` (the state may be half-updated) |
| re-entrant `&mut self` call (a JS callback calls back into the same ui object) | `NativeError { code: 'busy' }`, never a `RefCell` panic |
| abort | `NativeError { domain: 'svelters', code: 'cancelled' }`, **settled immediately**; the late worker result is dropped, and transferred buffers are released on the JS thread when the job ends |
| owner disposed or runtime reloaded | `code: 'disposed'` (NT1's rule) |

`catch_unwind` needs `panic = "unwind"`. Under `panic = "abort"`, the docs say a panic in an export ends the app.

## 7. Rust-authored egui widgets as Svelte elements

### 7.1 Authoring

```rust
use svelters_egui::prelude::*;                   // Widget, WidgetCx, Event, Children, prop/event/slot attrs

/// <knob bind:value={gain} min={0} max={2} label="Gain" onchange={(e) => …} />
#[svelters::widget(tag = "knob")]
pub struct Knob {
    #[prop(bind)]                  value: f32,              // two-way: written by the widget through cx
    #[prop(default = 0.0)]         min: f32,
    #[prop(default = 1.0)]         max: f32,
    #[prop]                        label: Option<String>,
    #[prop(default = Color32::GRAY)] color: Color32,         // TS: Color record / CSS color string
    #[event]                       change: Event<f32>,      // onchange
    #[state]                       drag_origin: Option<f32>, // Rust-only widget state, survives prop updates
}

impl Widget for Knob {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::drag());
        if resp.dragged() {
            let v = (self.value - resp.drag_delta().y * 0.01 * (self.max - self.min)).clamp(self.min, self.max);
            if v != self.value {
                cx.set(&mut self.value, v);       // marks (node, value) dirty → observed by bind:
                cx.emit(&self.change, v);         // queued; posted after the Ui pass
            }
        }
        paint_knob(ui.painter(), rect, self.value, self.min, self.max, self.color);
        if let Some(l) = &self.label { ui.painter().text(rect.center_bottom(), egui::Align2::CENTER_TOP, l, Default::default(), ui.visuals().text_color()); }
        resp
    }
}

/// Containers take child slots; the scene graph renders the children.
#[svelters::widget(tag = "collapsing")]
pub struct Collapsing {
    #[prop] title: String,
    #[prop(bind, default = true)] open: bool,
    #[slot] content: Children,                   // default slot
    #[slot(name = "header-extra")] extra: Children,
}
impl Widget for Collapsing {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let r = egui::CollapsingHeader::new(&self.title).open(Some(self.open)).show(ui, |ui| cx.children(ui, &self.content));
        if r.header_response.clicked() { cx.set(&mut self.open, !self.open); }
        r.header_response
    }
}

/// Function form for stateless widgets.
#[svelters::widget(tag = "sparkline")]
fn sparkline(ui: &mut egui::Ui, #[prop] data: Shared<[f32]>, #[prop(default = Color32::LIGHT_BLUE)] color: Color32) -> egui::Response {
    draw_sparkline(ui, &data, color)
}
```

```svelte
<script lang="ts">
  import { Simulation, simStats } from 'svelters:app/sim';
  const sim = new Simulation({ gravity: 9.81, damping: 0.02, integrator: 'verlet' });
  let gain = $state(1);
  let progress = $state(0);
  let ctl: AbortController | null = null;
  let error = $state('');

  async function run() {
    ctl = new AbortController();
    try {
      const r = await sim.step(100_000, { signal: ctl.signal, onProgress: (p) => (progress = p) });
      console.log(`energy ${r.energy} after ${r.wallMs.toFixed(1)} ms`);
    } catch (e) {
      error = e.code === 'cancelled' ? '' : e.message;          // NativeError
    } finally { ctl = null; }
  }
  $effect(() => sim.on('diverged', (e) => (error = `diverged at ${e.step}`)));
</script>

<vbox>
  <knob bind:value={gain} min={0} max={2} label="Gain" />
  <slider bind:value={simStats.speed} min={0.1} max={10} />     <!-- store field: bind_prop -->
  <label text={`step ${simStats.step} · E = ${simStats.energy.toFixed(3)}`} />   <!-- reactive store getters -->
  <sparkline data={sim.positions()} />                          <!-- zero-copy Shared<[f32]> -->
  {#if ctl}
    <progress-bar fraction={progress} /> <button onclicked={() => ctl?.abort()}>Cancel</button>
  {:else}
    <button onclicked={run}>Run</button>
  {/if}
  {#if error}<label text={error} />{/if}
</vbox>
```

The egui core tag vocabulary (`vbox`, `slider`, `button`, `progress-bar`) is the M7 backend's to define; these tags are placeholders.

### 7.2 What the macro generates

- **A `WidgetKind` entry** registered through `inventory`, in the egui backend's app-widget pack range (pack index 62).
  - It holds the prop table: name, type, default, `bind`; the events, slots and the factory.
  - Props are stored typed per node by the scene graph. The generated code copies them into the struct before `ui()` and back after. For large props the copy is a field-wise move, and `Shared` props are `Arc` clones.
- **Bindings on the observe model.**
  - `cx.set(&mut field, v)` records (node, prop) as dirty.
  - After the `Ui` pass the backend calls `host.prop_changed(h, prop)` for each one. That happens on the UI thread with no borrows live, so the R1 machinery reads the prop back with `get_prop` from node storage, compares and batches.
  - Every widget-side write goes through `cx`, so **`#[prop(bind)]` props are `exact` observability by construction**. That is better than REACTIVITY §9.4's general egui assumption ("every prop `set_only`, bindings by polling"), which still applies to wrapped ecosystem widgets that don't report writes (§7.3).
- **Events:** `cx.emit` queues `(h, signal, args)`, posted with `post_event` after the pass.
  - Sync handlers (`call_sync`) are not available inside `ui()`. A widget needing a synchronous answer, such as a veto, declares `#[event(sync)]`. The backend then calls `call_sync` after the pass and applies the result on the next frame. That is one frame late, which is documented.
- **Semantics + typings.** The schema entry feeds:
  - the egui semantic spec: props with observability `exact` for `bind` props, events, slots, a child-slot `accepts` list;
  - the TS `IntrinsicElements` augmentation, the same mechanism as packs (CUSTOM-WIDGETS §2.3);
  - a docs card.
- **Widget state.** `#[state]` fields persist per node across frames and prop updates. They are keyed by node identity, which the scene graph maps to `egui::Id`. With `#[state(persist)]` they also survive a hot reload when the node's key survives.

### 7.3 Wrapping ecosystem widgets with little glue

`wrap_widget!` covers `impl egui::Widget` values built from props. Mutation is detected from the `Response` or by comparing before and after.

```rust
svelters_egui::wrap_widget! {
    /// <drag-value bind:value={x} speed={0.1} />
    tag "drag-value";
    props { #[bind] value: f64, speed: f64 = 1.0, range: Option<[f64; 2]> = None, suffix: String = String::new() }
    |p, ui| {
        let mut w = egui::DragValue::new(&mut p.value).speed(p.speed).suffix(&p.suffix);
        if let Some([a, b]) = p.range { w = w.range(a..=b); }
        ui.add(w)                                    // value written in place; macro diffs `value` after the call
    }
}
```

| Crate | Wrapping | Notes |
|---|---|---|
| `egui_plot` 0.38 | `#[svelters::widget(tag = "plot")]` with `#[prop] series: Vec<Series>` where `Series` is a `Record` holding `Shared<[f32]>` x/y plus style; `#[prop(bind)] bounds: Rect` | data never copies: a worker publishes `Shared` slices into a store; JS passes `{ x: stats.t, y: stats.energy }` through; or the widget reads the store directly by name (`#[prop] source: StoreRef`) |
| `egui_extras` 0.37 tables | `<table>` with `#[prop] columns: Vec<Column>`, `#[prop] rows: u32`, and a row source: a Rust `TableModel` handle (`cell(row, col) -> CellValue`), virtualized with `body.rows(h, n, …)` | Svelte-snippet rows per cell are possible (a `row` snippet slot rendered by `cx.render_snippet(i)`), but cost a JS round trip per visible row change; recommend Rust models for big tables, as X5 lists recommend native models |
| `egui_dock` 0.21 | `<dock-area bind:layout={layout}>` with child `<dock-tab key="…" title="…">` nodes; the widget keeps `DockState<TabKey>` as `#[state]`, implements `TabViewer` by rendering the child node whose key matches; layout changes emitted as `onlayoutchanged` and bindable (serialized `DockState`, a `Record`) | tabs are child slots keyed by `key`; closing a tab emits `ontabclose` (sync veto: next frame) |

### 7.4 Interface needed from the scene graph (for the other lane)

1. **Per-node typed prop storage** that the backend's `set_prop`/`get_prop` read and write. Widget factories describe its layout: prop id → type, default.
2. **A dirty-prop channel:** `cx.set` → (node, prop) queued and reported as `prop_changed` after the pass. Plus an event queue drained after the pass.
3. **Child slots:** an ordered child list per named slot, and `cx.children(ui, slot)` / `cx.child(ui, slot, i)` to render them recursively with the node's `egui::Id` scope.
4. **Per-node Rust state** (`Box<dyn Any>`) keyed by node identity. It survives prop updates and, with stable keys, reloads.
5. **Node identity → `egui::Id`** that is stable across frames (and reloads, for keyed nodes).
6. **Read access to stores/registry objects from `ui()`** without JS: `cx.store::<SimStats>()`, `cx.object::<Simulation>(h)`.
7. **The pass boundary rule:** no host or JS calls while a `Ui` is borrowed. `prop_changed`, `post_event` and `call_sync` happen after the pass.

## 8. Lifetimes, GC and hot reload

- **Ownership chain.** JS wrapper (`NativeObject`, one per handle through the WeakRef map) → handle → registry entry holding the strong `Rc<RefCell<T>>` (ui) or `Arc<T>` (shared).
  - GC of the wrapper → FinalizationRegistry → `queueRelease` → the entry is dropped at the checkpoint, on the JS thread. A ui object's `Drop` runs there.
  - A shared object's `Drop` runs wherever its last `Arc` dies, possibly a worker, which is fine because it is `Send + Sync`.
  - `dispose()` releases at once, as in NON-GUI-TYPES.
- **Identity.** Returning the same `Arc<T>` twice yields the same JS object. The registry keeps `Arc::as_ptr → handle` while the handle is live, exactly like Qt's `by_obj` map.
- **In-flight work keeps its object alive.** A worker method holds `self: Arc<Self>`, and the JS side keeps the wrapper reachable while a call is in flight (`inflight` in `core.js`). `await new Simulation(p).step(10)` therefore works.
- **Weak references.**
  - `WeakObj<T>` lets Rust refer to an exported object without keeping it alive: caches, back-pointers from children.
  - `ThreadsafeFn` and stream senders hold the target weakly and fail softly (lesson 2).
- **Cycles across the boundary.** QuickJS's collector can't see Rust edges, and an engine-neutral layer can't offer rquickjs `Trace`. A Rust object that stores a `JsFn` closing over its own wrapper therefore leaks. The design makes such cycles rare and finite:
  1. events are `Emitter` + JS-side scope-bound listeners (`on` returns a disposer the component's `$effect` runs). Native code never holds the JS function, as for toolkit signals today;
  2. a stored `JsFn` is a callback with an explicit scope (`SVL_CB_NOTIFIED`) and a `Subscription` the Rust side drops. Its handle is released on the owning object's `dispose`;
  3. `foreign` trait proxies hold a callback id per object, released when the Rust `Arc<dyn Trait>` drops (posted to the JS thread);
  4. reload clears every callback of the old runtime;
  5. dev mode: `host.live_callbacks()` per owning handle is reported on component teardown and at reload when it grows.
- **Hot reload (in-process, M2b).** The JS runtime is replaced and all JS state is lost. Rust state that Rust owns survives:
  - **survive:** `#[svelters::service]`s, `StoreStatic`s, objects registered with `svelters::persist("key", || Simulation::new(p))` (JS: `persist('sim', () => new Simulation(p))` returns the same Rust object in the next generation), and `#[state(persist)]` widget state of keyed nodes;
  - **die with the old generation:**
    - handles the old runtime held, released in bulk after the new runtime loads, since handles are tagged with a generation;
    - JS-created unpersisted objects;
    - callbacks and Promise completions (dropped by generation check);
    - streams (closed, so producers see `Closed`);
    - in-flight operations (cancel tokens fired).
  - Edits to Rust code still mean a Cargo rebuild and a process restart. Hot-patching Rust (Dioxus `subsecond`-style) is an open question.
- **Shutdown order:** drop the JS runtime (no `Persistent` may outlive it: rquickjs aborts), cancel tokens, close streams, join the pools with a timeout, stop the services, then the toolkit. Transfer tokens still held by workers at shutdown are leaked deliberately, never freed on the wrong thread.

## 9. Codegen pipeline

```text
app crate (Rust)                         cargo build
  #[derive(Record/Enum/Error/Events)], #[svelters::object/export/store/service/widget]
     │ expands to
     ├─ thunks: fn(&Registry, &mut CallCx, ArgsRef) -> Result<Value, NativeError>  (+ worker/async wrappers)
     ├─ JsType impls: fn ts() -> TypeExpr, fn desc() -> SlotDesc                  (type info resolved by trait, like specta)
     └─ inventory::submit!(ExportEntry { module, item, members, docs, span })      (link-time registration)
                     │
   app binary ── svelters::App::run(): Registry::collect() ── schema (svelters-exports/1)
                     │
       ┌─────────────┼───────────────────────────────┬─────────────────────────────────────┐
       ▼             ▼                               ▼                                     ▼
 host built-in   debug start / `svelters exports`   svelters-compile                    docs / CLI
 modules         writes .svelters/app/:             (PlatformSemantics::native_type,    `svelters docs app sim.Simulation`
 svelters:app/*  exports.json, app.d.ts,            type_property, egui widget kinds    from #[doc] comments
 (JS generated   <module>.js (for Vite/Node/        merged into the egui semantic spec:
 from schema at  TestHost aliases); rewritten only  observability, bind lowering,
 load)           when content changes               diagnostics on unknown members)
```

- **Generated JS** has the same shape as the toolkit modules. Classes extend `NativeObject` and call only `call`/`callAsync`/`observed`/`construct` from `@svelters/types/core`:

  ```js
  // svelters:app/sim (served by the host; identical to .svelters/app/sim.js)
  import { NativeObject, CONSTRUCT, call, callAsync, callStaticAsync, observed, wrapObject, wrapStore, stream } from '@svelters/types/core';
  export class Simulation extends NativeObject {
    static native = 'app:sim.Simulation';
    static [CONSTRUCT](params) { return construct('app:sim.Simulation', { params }); }
    get particleCount() { return call(this, 'particle_count', []); }
    reset(params) { return call(this, 'reset', [params]); }
    step(n, options) { return callAsync(this, 'step', [n], options); }          // options: { signal, onProgress }
    positions() { return call(this, 'positions', []); }                          // ReadonlyFloat32Array
    run(frames, options) { return stream(this, 'run', [frames], options); }      // AsyncIterable<Frame>
  }
  export function spectrum(samples, window, options) { return callStaticAsync('app:sim', 'spectrum', [samples, window], options); }
  export const simStats = wrapStore('app:sim.SimStats');                         // getters: observed(this, 'energy') …
  ```

- **Generated typings:**

  ```ts
  declare module 'svelters:app/sim' {
    import type { NativeObject, AsyncOptions, WorkOptions, ReadonlyFloat32Array } from '@svelters/types/core';
    /** Plain data, copied across. */
    export interface SimParams { gravity: number; damping: number; particles?: number; integrator: Integrator }
    export type Integrator = 'euler' | 'verlet' | 'rk4';
    export type Source = { kind: 'point'; x: number; y: number } | { kind: 'disc'; x: number; y: number; r: number };
    export interface StepReport { steps: number; energy: number; wallMs: number }
    export class Simulation extends NativeObject {
      /** @throws NativeError `app:sim` `too-many` */
      constructor(params: SimParams);
      readonly particleCount: number;
      reset(params: SimParams): void;
      /** Runs on a worker. @throws NativeError `app:sim` `diverged`; `svelters` `cancelled` | `panic` */
      step(n: number, options?: WorkOptions<number>): Promise<StepReport>;
      /** Zero-copy, read-only. */
      positions(): ReadonlyFloat32Array;
      run(frames: number, options?: AsyncOptions): AsyncIterable<Frame>;
      on(event: 'diverged', fn: (e: { step: number }) => void): () => void;
      on(event: 'checkpoint', fn: (e: { step: number }) => void): () => void;
    }
    /** Runs on a worker. @param samples transferred: detached until the Promise settles. */
    export function spectrum(samples: Float32Array, window: Window, options?: AsyncOptions): Promise<Float32Array>;
    export function loadScene(path: string, options?: AsyncOptions): Promise<SimParams>;
    /** Reactive store (exact). */
    export const simStats: { readonly step: number; readonly energy: number; readonly running: boolean; speed: number };
  }
  declare namespace svelteHTML { interface IntrinsicElements {
    knob: { value?: number; min?: number; max?: number; label?: string | null; color?: Color | string; onchange?: (e: CustomEvent<number>) => void; 'bind:value'?: number };
  } }
  ```

- **Why the schema comes from the binary rather than from `build.rs` or the macros' side effects:**
  - a `build.rs` runs before its own crate's macros expand, so it would have to re-parse the source with `syn`, like cbindgen, and could not resolve types through traits;
  - napi-style env-gated sidecar files written during expansion go stale under incremental compilation and the build cache;
  - runtime registration is exact for the binary that actually runs. That is the property the JS stubs need, which is also why the host serves them itself.
- **The variant for workspaces.** When exports live in a separate `domain` crate, the UI crate's `build.rs` may take it as a build-dependency, call `domain::svelters_schema()`, and write the typings before the UI crate compiles. That costs a second compile of the domain crate in the build profile. It is optional, for teams that want types without running the app.
- **Staleness.**
  - Vite and svelte-check read `.svelters/app/*.d.ts`, so a Rust rebuild + run updates the types.
  - `svelters exports --check` fails CI when committed typings differ.
  - The compiler warns once when `exports.json` is older than the binary.
  - PLAN rule 5 holds: Svelte edits never touch Cargo, and Rust edits regenerate JS artifacts as outputs.
- **Docs:** `#[doc]` comments go into the schema, and the docs generator renders `app` cards with the same sections as NON-GUI-TYPES §6: class, construct, props, methods, statics, events, plus "runs on".
- **Testing:**
  - `svelters::testing::Harness` runs a module's exports on a real host with the log backend, so a Rust test can `await` JS calls.
  - Node TestHost runs get schema-generated stubs that throw "native export not available in Node" unless the export crate is built as an N-API addon. The registry is engine-neutral, so a `napi` engine adapter is possible later (`node-deno-integration.md`).

## 10. Phasing (proposal)

| Phase | Content | Acceptance |
|---|---|---|
| E0 registry core | `svelters-host::exports` (handles in the app range, kinds `app:*`, routing in `construct`/`call`/`callStatic`/`getProp`/`setProp`/`listen`), `Record`/`Enum`/`Error` derives, `object` + sync `export` impls/free fns, schema dump + `.d.ts`, built-in `svelters:app/*` modules | an `examples/exports` app on GTK (works before M7!) calls a Rust object from Svelte; identity/dispose/GC tests; `svelte-check` passes against the dumped typings |
| E1 offload | completion queue settled at checkpoint start, `worker`/`async`/`blocking`/`local`, `WorkCx` (cancel, progress), panics, `supersede`, `max_in_flight`, generation tags | the probe's numbers on the registry path; a 200 ms worker keeps 60 fps; abort/panic/reload rejection tests |
| E2 buffers | `Value::Buffer`, engine buffer ops, `Buffer<T>`/`Shared<[T]>`/`svelters.alloc` | 64 MiB round trip without a copy; `Arc` released on GC; transfer token freed on the JS thread under abort |
| E3 reactive (**done**, `plans/milestones/E3-exports-reactive.md`) | `Store`/`StoreStatic`/`service`, `Emitter`, `Sink`/`Stream`, `persist` | stores in the R1 batch (10k worker updates → one batch); `bind:` on a writable store field; state survives a hot reload |
| E4 widgets (with M7) | `#[svelters::widget]`, `wrap_widget!`, semantic-spec fragment, IntrinsicElements typings; egui_plot / egui_extras / egui_dock wrappers | knob + plot + dock example; `bind:` on widget props observed `exact` |
| E5 compiler | `PlatformSemantics::native_type`/`type_property` from `exports.json`; diagnostics for unknown members, `bind:` on read-only fields | compile-time errors for typos in `svelters:app/*` member access |

## 11. Open questions

**Decided by the user on 2026-10-21** (E0 implements them; `plans/milestones/E0-rust-exports.md`):
- **Q1 specifier:** `svelters:app/<module>`.
- **Q6 `i64`/`u64`:** a checked JS `number` (a `RangeError` outside ±2^53, both ways); `#[js(bigint)]` on a parameter, record field or method opts that member into `bigint`.
- **Q8 op log:** exported-object calls are logged in full, like native calls (`construct #a1 app:sim.Simulation [...]`, `call #a1 app:sim.Simulation.step [...]`, `static app:sim.f [...]`, `set #a1 app:sim.T.prop <json>`), so TestHost replay covers them.
- E0 and G0 (non-blocking host start + started hook) ship together.

**Decided by the user on 2026-10-21 for E1** (implemented; `plans/milestones/E1-exports-offload.md`):
- **Q2 async executor: a bare `async fn` runs on the JS thread** (local, Deno-style): polled at the call (a future ready at once settles in the same turn), then whenever its waker fires, at checkpoint start. Pool execution is opt-in: `#[js(tokio)] async fn` (tokio, behind the `tokio` feature of `svelters`, not a default dependency), `#[js(worker)]`/`#[js(blocking)]` for sync code, and `svelters::exports::worker(|| ...).await` / `blocking(...)` inside a local future. Guard: documented on the facade crate, and debug builds print one console warning per member when a single poll of a JS-thread future takes over 20 ms.
- **Q3 settle-on-abort: abort rejects immediately** with `NativeError { domain: 'svelters', code: 'cancelled' }`, in the abort listener itself. The worker sees the cancel through `WorkCx` (a queued job never starts; a JS-thread future is dropped; a tokio task is aborted). A late result is dropped on the JS thread without being converted, so it mints no handle; objects and buffers in it are freed there.

**Decided by the user on 2026-10-22 for E3** (implemented; `plans/milestones/E3-exports-reactive.md`):
- **Q7 store granularity: a dirty bit per field.** A write marks the whole field changed and JS re-reads it (a clone under the store's read lock at the checkpoint, compared with the last delivered value). `#[store(proportional)]` fields (a big `Vec`, a log `String`) are invalidation only: never read or compared by the host, re-read by JS when it needs them. There are no per-item `Observable<T>` diffs in E3; per-item diffs for collection fields (feeding `@svelters/lists`) are possible future work.

Still open: Q4 (foreign traits; left out of E1), Q5 (fast path: the registry path's per-call cost is now measured, see the E0 milestone file), Q9 (subsecond), Q10 (CUSTOM-WIDGETS Q9).


1. **Specifier.** `svelters:app/<module>` matches `svelters:<provider>/<ns>`, keeps the bare `svelters:app` for QUICKJS-RUNTIME's built-in app-info module, and gives the M6 linker static roots. Alternatives: `$native/<module>` (tsconfig paths) or `native:<module>`.
2. **Default executor for `async fn`.** Recommendation: tokio, behind a default feature, with an app-supplied runtime handle. Should a bare `async fn` instead mean `local` (the JS thread, Deno-style) and require `#[js(tokio)]` for the pool? Local-by-default avoids a tokio dependency but makes `Send` futures easy to block the UI with.
3. **Settle-on-abort.** The proposal rejects immediately and drops the late result. The alternative is waiting until the worker observes the cancel, which is more honest for transferred buffers and simpler for "is it still running?" UIs.
4. **`foreign` traits called from workers** block the worker on the JS thread. Keep them, restrict them to `async` methods only, or drop them from v1?
5. **The fast path** (direct rquickjs functions for scalar `#[js(fast)]` methods): never, or only after a measured app need? This depends on the registry path's real per-call cost, which needs measuring once E0 exists. The ABI path measured 2.9–4.0 µs on a loaded machine.
6. **The `i64`/`u64` policy:** a checked `number` by default (proposed) or `bigint`?
7. **Store granularity:** a dirty bit per field (proposed), or per-field `Observable<T>` cells for nested or collection fields (`Vec` diffs for lists feeding `@svelters/lists`)? **Decided 2026-10-22: a dirty bit per field** (above).
8. **Should exported objects show up in the op log** in full (`call #h app:sim.Simulation.step [...]`) for TestHost replay, or only their creation and destruction?
9. **Rust hot-patching** (`subsecond`) for export bodies in dev: worth a probe after E1?
10. **CUSTOM-WIDGETS Q9** (Rust GTK widgets' methods): the registry gives GI-free thunks for any Rust type, and a GTK `glib::Object` subclass could be both a pack kind and an `#[svelters::object]`. Should one type be allowed to be both?
