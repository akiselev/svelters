# E2: Rust exports buffers (zero-copy typed arrays)

Design: `docs/notes/egui-rust-exports.md` §5.3–5.4 (type mapping, `Value::Buffer`), §6.3 (moving data), §6.7
(abort), §8 (shutdown order), §10 (phasing). Builds on E0 (`E0-rust-exports.md`) and E1
(`E1-exports-offload.md`), whose abort decision (reject at once, drop the late result on the JS thread)
shapes how transferred memory is released. This file records E2 as built.

## Acceptance (§10) and where it is shown

| Acceptance | Evidence |
| --- | --- |
| 64 MiB round trip without a copy | `crates/svelters/tests/buffers.rs` `round_trip_64mib_without_a_copy`: a JS `Float32Array(16 Mi)` goes to a worker and back, then to a sync export; the worker and the sync export see the **same address** (pointer identity), the moved array is detached, the counting global allocator sees **no allocation ≥ 32 MiB** after the arrays were made, `BufferStats.copies == 0`. The same for an `alloc`ed (Rust-backed) array, reclaimed twice. Timings below. |
| `Arc` released on GC | `rust_memory_is_reclaimed_and_shared_views_are_read_only_and_released_on_gc`: a `Shared<[f32]>` view holds one `Arc` reference; after the view is dropped and `collect_garbage` runs, none. Writes to the view are ignored without an exception (`write 1 false`). |
| Transfer token freed on the JS thread under abort | `transfer_tokens_are_released_on_the_js_thread_under_abort`: (1) a worker that finishes and drops the buffer on its thread: the token is gone but nothing is released until the next checkpoint (`released` unchanged, then +1); (2) **abandoned** (aborted while running): the Promise rejects at once, the memory stays pinned (`transfersLive() == 1` across checkpoints) while the zombie runs, and is released when its late result (the buffer) is dropped on the JS thread; (3) aborted while **queued** behind `max_in_flight`: the job (and buffer) is dropped on the JS thread, the call never starts; (4) **panic**: the unwind drops the buffer on the worker, the release happens at the next checkpoint. Plus shutdown and reload (below). |

## As built

### Value model (`crates/svelters-abi/src/buffer.rs`)

- `Value::Buffer(BufferValue)`: `elem` (`Elem`: u8/i8/u16/i16/u32/i32/u64/i64/f32/f64), byte `offset`, `len` and a
  payload `BufData`:
  - `Owned(Box<dyn BufSource>)`: Rust memory, JS gets exclusive access;
  - `Shared(Arc<dyn BufSource>)`: immutable memory, JS gets a read-only view;
  - `Transferred(TransferToken)`: a JS buffer moved out of JS; the memory stays in the engine.
- `BufSource` is an `unsafe trait` (stable pointer, `Send + Sync`, `as_any` for downcasts).
- `TransferToken` is `Send + Sync`; its `Drop` (any thread) calls `TransferHome::token_gone(id, false)`, and
  `into_returned()` hands the memory back to JS (`token_gone(id, true)`). It never frees anything itself.
- `BufferValue` is an `Arc` inside: clones are cheap and **share** the payload. An owned or transferred payload
  is taken exactly once (`take()`); a shared one is never taken (`shared()`). `Debug`/`PartialEq` (by content)
  for the `Value` derives.
- **C ABI unchanged.** On the raw `svl_value` path a buffer is `BYTES` borrowed from the same memory (a C backend
  that keeps it copies); `Value::from_raw` never makes one. The log backend prints `"<Float32Array n>"`.

### User types (`crates/svelters-host/src/exports/buffers.rs`, re-exported by `svelters` and its prelude)

- `Buffer<T: Element>`: an owned typed array. `Deref`/`DerefMut` to `[T]`, `from_vec`/`From<Vec<T>>`, `zeroed`,
  `into_vec` (no copy for a whole Rust `Vec`, a copy for JS memory), `as_ptr`, `is_js_memory`. `Send + Sync`.
  - Parameter: TS `Float32Array`, **moved**: a JS buffer is transferred (`ArrayBuffer.prototype.transfer()`), a
    Rust-backed one reclaimed; the caller's array is detached for good.
  - Result: a new typed array over the memory; a buffer that came from JS goes back as the **same**
    `ArrayBuffer`.
- `Shared<T: ?Sized>(Arc<T>)`; `Shared<[T]>` is the export type.
  - Result: TS `ReadonlyFloat32Array` etc., an immutable `ArrayBuffer` holding an `Arc` clone (released on GC).
  - Parameter: a view this host made gives back its `Arc` (`Arc::ptr_eq` holds; no copy); any other typed array of
    `T` is copied once (the caller keeps it).
- Slots: `SVL_BYTES` typed in a reserved app-range block (`APP_PACK | 0x10_0000 | moved<<4 | elem`), so
  `convert.rs` recognises buffer slots in parameters, record fields and list elements alike.
- Buffers work in records (`#[derive(Record)]` fields, `Option<Shared<[T]>>`), `Vec<Buffer<T>>`, worker arguments
  and results (`frame_double`, `lens` in the tests; `Smoothed` in the example).

### Conversion (`crates/svelters-host/src/convert.rs`)

- **Two-phase arguments.** `encode` only *stages* a buffer argument (`JsReader::stage_buffer`: checks the element
  type, detached, SharedArrayBuffer, resizable, read-only, the same buffer moved twice) and leaves its staging
  index in the raw value. Only after **every** argument converted does `owned_all` call
  `JsReader::commit_buffers` (the transfers/reclaims) and build owned values with the buffers in place
  (`owned`, which replaces `Value::from_raw` on the app paths). A call that fails validation, an already
  aborted call and a call on a disposed object leave the caller's arrays untouched (tested).
- `JsWriter::buffer` makes the JS value of a `Value::Buffer` (default: a copy, for engines without support).
- `JsEngine::release_buffers(ids)` and `JsEngine::buffer_stats()` (defaults: no-op / zeros).

### QuickJS (`crates/svelters-host/src/quickjs/buffers.rs`, `prelude.js`)

- **Rust → JS:** `JS_NewArrayBuffer` over the source with our free callback (owned), or immutable
  (`JS_SetImmutableArrayBuffer`) holding an `Arc` clone (shared); `JS_NewTypedArray(ab, offset, len)`.
- **Ours:** a per-thread table maps the data address of our live external buffers to `Owned { byte_len }` or
  `Shared { Weak, views }` (the engine and its free callbacks run on one thread).
- **Reclaim** (a `Buffer<T>` argument over Rust memory): `JS_DetachArrayBuffer` with the address marked; the
  free callback hands the allocation back instead of dropping it.
- **Transfer** (any other JS buffer): the prelude calls the intrinsic `ArrayBuffer.prototype.transfer` and keeps
  the new `ArrayBuffer` in a prelude-private `Map` under a process-wide id; Rust gets a `TransferToken` for its
  memory. `transferOut(id)` restores it for a result; `transferDrop(ids)` releases. The intrinsics (`transfer`,
  the typed array getters, `detached`, `resizable`, the SharedArrayBuffer brand check) are captured before user
  code runs.
- **Release on the JS thread.** `token_gone` pushes the id to the host's completion queue (`push_release`, one
  wake per burst); the checkpoint drain (`core::offload::drain`, after settling) calls `release_buffers`, which
  deletes the table entries; QuickJS frees the memory there.
- **Runtime teardown (shutdown, reload).** A runtime is only dropped once Rust holds none of its transferred
  memory:
  - Host drop: E1's `ops_shutdown`, then the finished results nobody will settle are reaped (`ops_reap`) and the
    app objects released, *then* the engine drops.
  - Reload: the old runtime's calls are retired (cancel tokens fired, settled results reaped) and its app objects
    released **before** it is dropped (order change in `reload.rs`).
  - `QjsEngine::drop` waits up to **1 s** for live tokens (reaping late results meanwhile); past that it **leaks
    the runtime** (a `Context` clone that is never dropped), prints why and counts it in
    `BufferStats.leaked_runtimes` (per thread). Never frees memory under a worker (§8).
- **`alloc(Float32Array, n)`**, exported by `@svelters/types/core` (`svelters:core` in the host):
  `__svelters_host.alloc` makes a zeroed typed array over a Rust `Vec`; outside the host it is `new ctor(n)`.

### Typings and schema

- `Buffer<T>` → `Float32Array` (etc.); `Shared<[T]>` → `ReadonlyFloat32Array` (etc.), imported per module from
  `@svelters/types/core` only when used.
- `@svelters/types/core` declares `ReadonlyUint8Array` … `ReadonlyFloat64Array` (and `ReadonlyBigInt64Array`,
  `ReadonlyBigUint64Array`): `Omit<Float32Array, writers | number>` plus a `readonly` index signature and a
  `subarray` returning the read-only type. A read-only view is not assignable to `Float32Array` (so it cannot be
  passed to a moved parameter); a `Float32Array` is assignable to the read-only type. `alloc` is typed by the
  class (`alloc(Float32Array, n): Float32Array`). `packages/types/test/types/buffers_usage.ts` checks all of this
  with `@ts-expect-error` on the writes.
- Moved parameters get a doc line: `@param samples moved to Rust: the array is detached (length 0) once the call
  starts; use the result instead`.
- The macro's `&mut [T]` error now points to `Buffer<T>`.

### Example (`examples/exports`)

`src/dsp.rs` (`svelters:app/dsp`) and `app/src/Dsp.svelte`: `noise(n, seed)` makes 1 Mi samples in Rust (a
`Float32Array` over Rust memory), `smooth(samples, passes)` filters them in place on a worker (progress through
`WorkCx<f64>`; the input is detached meanwhile) and returns a record whose `samples` is the same allocation
(`zeroCopy: true`), `waveform()` is a read-only view of a sine table (`ReadonlyFloat32Array`), `stats(view)` reads
it in place (`isWaveform` shows the `Arc` came back). The GTK test `dsp_round_trips_samples_without_a_copy` clicks
it; `svelte-check` passes against the dumped typings.

## Deviations from the note

- **Names.** The payload is `BufferValue` (the note's `Buffer`), because `Buffer<T>` is the user type. Clones of a
  `BufferValue` share the payload (take-once), so `Value: Clone` stays cheap (the op log clones arguments).
- **The token's engine record is a prelude-private JS `Map` entry**, not a host table of
  `Persistent<ArrayBuffer>`: nothing can outlive the runtime (rquickjs aborts on a `Persistent` that does), and a
  release for a runtime that is gone is a no-op (ids are process-wide).
- **Engine buffer ops** are `stage_buffer`/`commit_buffers` (two phases, so failed calls move nothing) and
  `JsWriter::buffer`, plus `JsEngine::release_buffers`, rather than `external_buffer`/`transfer`/`reclaim`.
- **`svelters.alloc`** is `alloc(ctor, n)` from `@svelters/types/core` (no `svelters` global exists).
- **`Shared<[T]>` as a parameter** (not in the note's table): identity for views the host made, one copy
  otherwise. `bytes::Bytes` is not supported (no new dependency).
- **Teardown:** a bounded wait (1 s) and then a deliberate leak of the whole runtime (the note's "leaked
  deliberately"), at reload as well as at shutdown. Reload now cancels the old calls and releases the old app
  objects before dropping the old runtime (it used to drop the runtime first).
- **Not supported:** `Uint8ClampedArray`/`Float16Array`, buffers inside maps (`HashMap<String, Buffer<T>>`: map
  values are dynamic), resizable `ArrayBuffer`s for `Buffer<T>` (rejected), SharedArrayBuffers (rejected, as
  §6.3 says), buffers over 2 GiB (QuickJS's limit), `&[T]`/`&mut [T]` borrowing a typed array for a sync call
  (`&[T]` still copies through an array, as in E0).

## Measurements

Release build, QuickJS-NG via rquickjs 0.14, log backend, WSL2 16 cores, **load average 9–10** (other lanes
building). `cargo test --release -p svelters --test buffers -- --ignored --nocapture buffer_costs`, best of 5 per
run, three runs (64 MiB = 16 Mi `f32`):

| Path | ms |
| --- | --- |
| sync `pass(a)`: JS buffer moved to Rust (`transfer()`) and handed back (same `ArrayBuffer`) | 0.072–0.152 |
| worker `await passWorker(a)`: transfer, rayon, completion, checkpoint, restore | 1.35–1.38 (≈1 ms of it is the test loop's 1 ms sleep between checkpoints) |
| sync `pass(b)` of an `alloc`ed array: reclaim (detach) and hand back (new external `ArrayBuffer`) | 0.013–0.018 |
| `view64()`: read-only view of a 64 MiB `Arc<[f32]>` | 0.048–0.060 |
| baseline: one JS copy of the same 64 MiB (`a.slice()`) | 78–80 |

So moving 64 MiB costs about 1/1000 of one copy, in line with the probe (§4: 0.02–0.045 ms for the view,
0.024–0.031 ms for `transfer()`, 80–94 ms per copy). The acceptance test (debug build) also shows no copy
structurally: pointer identity across JS → worker → JS → Rust, no large Rust allocation after setup, zero
fallback copies. (QuickJS allocates with libc `malloc`, not the Rust allocator, so the counter covers Rust-side
copies; JS-side, QuickJS's `transfer()` reuses the data pointer, which the pointer identity confirms.)

## Tests

- `crates/svelters/tests/buffers.rs` (9, plus the ignored `buffer_costs` benchmark): the 64 MiB round trip;
  reclaim of Rust memory and `Shared` views (read-only, identity, copy for foreign arrays, `Arc` released on GC);
  detach semantics and untouched arrays on failed/refused calls (wrong element type, `ArrayBuffer`, plain
  array, read-only view, the same buffer twice, a later argument failing); records/lists/worker args/results;
  release on the JS thread under finish/abort/queue/panic; shutdown (cooperative worker: no leak, < 0.9 s; stuck
  worker: runtime leaked after the 1 s grace; an undrained result and a UI object holding a buffer: no leak, no
  wait); reload with a buffer in flight (cooperative: freed; stuck: leaked); the op log; the typings.
- `crates/svelters-abi/src/buffer.rs` (5 unit tests): raw path borrows the same bytes, take-once clones, shared
  payloads, tokens report drop/return across threads, range checks, element names.
- `examples/exports/tests/exports.rs`: `dsp_round_trips_samples_without_a_copy` (GTK), and
  `committed_typings_are_current` now covers `dsp.js`.
- `packages/types/test/types/buffers_usage.ts` (tsc, in `pnpm test`).
- The offload and buffer tests also pass in `cargo test`'s shared-process mode.

## Completion record

Code head **a37cda55** on `ci/e2` (E2 commits merged with origin/master 35e598b1, a STATUS-only change). This
record and the design note's status line are a docs-only commit on top.

Local gates, all re-run on that tree, `SVELTERS_SLOT_JOBS=4`:

- `cargo check --workspace --all-targets`: clean (pre-existing warnings only); the portable backend crates
  (`svelters-apple`, `svelters-appkit`, `svelters-winui`, `counter-mac`, `counter-win`) checked too.
- `cargo nextest run --workspace --test-threads 4` (headless, under the shared nextest lock): **2438/2438
  passed**, 10 skipped.
- `pnpm -r build`, `pnpm test` (node tests 138/138, `test:meta` 155 passed / 1 skipped, the example's
  svelte-check 0 errors, the types tsc incl. `buffers_usage.ts`), `python3 tools/docs/build_ir.py` +
  `pnpm docs:test` (python 11 OK / 1 skipped, node 17/17): all exit 0.
- `crates/svelters` buffers and offload tests also pass in `cargo test`'s shared-process mode.

CI on a37cda55, all success: js 39662234931, linux-gtk 39662234723 (gtk 2234/2234, portable 48/48, gtk-4-14
1930/1930 with 2 retried flakes outside E2's code: `generated::verify::observe_gtk_menubutton` and
`observe_gtk_spinner`, both passed on try 2), linux-qt 39662235058, macos-appkit 39662234765, windows-winui
39662235169.

Earlier WIP heads failed macos-appkit and linux-gtk's portable job: `svelters-apple` had an exhaustive `match` on
`Value` (fixed in 27d53705; a local `--workspace` check had stopped at the first failing crate).

## Open items

- `&[T]`/`&mut [T]` sync parameters borrowing a typed array in place (lesson 4's "borrow" mode): `&[T]` still
  copies through a JS array (E0), `&mut [T]` is a macro error that points to `Buffer<T>`.
- A `Buffer<T>` from JS stored long-term in Rust (a service, a persisted object: E3) pins the JS runtime's memory,
  and a reload then leaks that runtime after 1 s. `into_vec()` copies into Rust memory; a "detach to Rust
  allocation" path (copy once at reload) would avoid the leak.
- A reload waits up to 1 s (blocking the UI thread) when a worker ignores the cancel; a graveyard that drops the
  old runtime once its tokens are gone would not block.
- Node TestHost: `svelters:app/*` exports (and so buffers) remain QuickJS-host only.
- External `ArrayBuffer`s (Rust memory) do not count toward the QuickJS `memory_limit`; transferred ones do.

## Conflict risks

- `crates/svelters-abi/src/safe.rs` (one variant, one `build_raw` arm), new `buffer.rs`, `lib.rs` (one `mod`): any
  exhaustive `match` on `Value` in another lane needs a `Value::Buffer(_)` arm (fixed here:
  `svelters-backend-log`, `svelters-apple`).
- `crates/svelters-host/src/convert.rs`: `JsReader`/`JsWriter` default methods, the `SVL_BYTES` branch of
  `encode_at`, the `Value::Buffer` arm of `decode_at`, `owned`/`owned_all` at the end of the file.
- `core/apps.rs` (`app_args`, `app_set_prop` use `owned_all`), `core/offload.rs` (`app_start_op` argument
  block; `drain` tail; `ops_end_generation`/`ops_shutdown` call the new `ops_reap`), `exports/offload.rs`
  (`Queue.released`, `push_release`, `take_released`, `take_done_except`), `exports/mod.rs` (one `mod`, one
  `pub use`), `exports/codegen.rs` (`typescript()` split into `ts_module_body`, `doc_member`).
- `engine.rs` (`BufferStats`, two default methods), `lib.rs` (`buffer_stats`, `Host::drop` releases app objects),
  `quickjs.rs` (`QjsCallbacks.home`, `settle_ops` passes the callback store, `alloc` host op,
  `release_buffers`, `Drop for QjsEngine`), `quickjs/conv.rs` (two fields, three trait methods), `prelude.js`
  (helpers before `__svelters_internal`, which lists them), `reload.rs` (the success branch's order).
- `crates/svelters/src/lib.rs` (docs, re-exports), `svelters-macros` (one error message).
- `packages/types/src/core.{js,d.ts}` (appended), `packages/types/test/types/tsconfig.json` (one entry).
- `examples/exports`: `src/lib.rs` (`pub mod dsp;`, one line in `link()`), `app/src/App.svelte` (one import, one
  `<Dsp />`), `tests/exports.rs` (one test, `dsp.js` in the typings list); the generated
  `app/.svelters/app/{app.d.ts,exports.json}` conflict with any lane that adds exports: regenerate with
  `cargo run -p exports-example -- --dump examples/exports/app/.svelters/app`.
