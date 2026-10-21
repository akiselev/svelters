# E3: reactive Rust exports (stores, services, events, streams, persist)

Design: `docs/notes/egui-rust-exports.md` §5 (macro surface), §6.4 (stores, services), §6.6 (streams, events),
§8 (lifetimes, hot reload), §10 (phasing), §11 (decisions). Builds on E0 (`E0-rust-exports.md`) and E1
(`E1-exports-offload.md`), and on R1/R2 (`R1-host-batching.md`, `R2-compiler-observe.md`). This file records E3
as built.

## User decision (2026-10-22)

**Store granularity (note §11 Q7): a dirty bit per field.** A write marks the whole field changed and JS re-reads
it. `#[store(proportional)]` fields (a big `Vec`, a log `String`) are invalidation only. There are no per-item
`Observable<T>` diffs in E3; per-item diffs for collection fields (`Vec` diffs feeding `@svelters/lists`) are
possible future work. Recorded in note §11.

## As built

### Stores

```rust
#[svelters::store(module = "telemetry")]          // the class `Telemetry` of svelters:app/telemetry
pub struct Telemetry {
    pub samples: u64,                              // read-only in JS, reactive
    #[store(writable)] pub speed: f64,             // assignable (and bindable) from JS
    #[store(proportional)] pub log: String,        // delivered as an invalidation
}
impl StoreHooks for Telemetry {                    // optional
    fn on_write(&mut self, f: TelemetryField) -> Result<(), NativeError> {
        if f == TelemetryField::Speed { self.speed = self.speed.clamp(0.5, 10.0); }
        Ok(())
    }
}
#[svelters::export(module = "telemetry")]          // JS: export const telemetryStats
pub static TELEMETRY_STATS: StoreStatic<Telemetry> = StoreStatic::new();   // needs Default, or StoreStatic::with(f)
```

- **Rust API** (`svelters_host::exports::reactive`): `Store<S>` (an `Arc<StoreCell<S>>`; `Send + Sync`, clone
  to share) with `update(|s| ..)` (marks every field), `write(Field::X, |s| ..)`, `write_fields(&[..], |s| ..)`,
  `touch(Field::X)`, `get(|s| ..)`, `read()`; `StoreStatic<S>` (a `static`, created on first use, derefs to
  `Store<S>`). The macro generates `<Name>Field` (one variant per field, PascalCase) and the `StoreState` impl
  (field table, per-field clone-to-JS, from-JS assignment, typings).
- **`StoreCell<S>`**: the state under an `RwLock`, plus a list of weak per-host subscriptions (`StoreSub`): a
  dirty mask (`AtomicU64` per 64 fields), a "pending" flag and the host's `Hub`. A write takes the write lock,
  changes the state, releases the lock, then `fetch_or`s the bit(s) in every subscription; the first bit since
  the host last looked pushes the handle to the host's `Hub` and wakes the backend once per burst (the
  `Completions` pattern of E1). Writers never call into the host; with no host attached the write is just a
  locked assignment.
- **Host side** (`crates/svelters-host/src/core/reactive.rs`): a store object is an ordinary app object (class
  kind `app:<m>.<Name>`, one getter per field, one setter per writable field). `host.observe(h, field, true)`
  subscribes this host to the store (first use) and refcounts (h, prop) in R1's `observed` table with the
  field's current value as the baseline `last`. Non-store app props still answer `false`.
- **Checkpoint order**: (1) offload drain (E1); **(1b) `reactive::collect`**: the dirty masks of the notified
  subscriptions are taken and every dirty field that is observed is inserted into R1's dirty set; queued events
  become event-queue entries; (2) `take_changes` reads each dirty store field through its getter (a clone under
  the read lock), compares it with `last` and puts the changed ones (proportional ones as `undefined`) into the
  same `__svelters_changed` batch as native props; (3) events; (4) one job drain. `call_sync` collects too, so a
  sync handler sees fresh store values.
- **JS writes** (`stats.speed = v`): `setProp` → the generated setter: `FromJs`, the old value kept, the new one
  assigned under the write lock, `on_write` run if `S: StoreHooks` (autoref specialization, no opt-in flag); an
  `Err` restores the old value and throws in JS. Then R1's set path (`after_set`): the field is read back as
  `last`; a normalized value (the clamp) is forced dirty and delivered once; the store's own dirty bit for the
  write is then an echo the compare drops. After a hooked write every field is marked (the hook may change any
  of them; the compare drops the unchanged).
- **JS reads**: generated getters are `observed(this, 'field')`, so they are reactive in effects and templates
  with the native-runtime tracker installed (no `$state` mirror); typings mark non-writable fields `readonly`.
- **`exact` by construction**: every change goes through the store and is announced. `exports.json` records
  store props as `observability: "exact"`, `verified: "delivery"`, `proportional`.

### Services

```rust
#[svelters::service(module = "telemetry")]                 // JS: export const telemetry
fn telemetry(cx: ServiceCx) -> Store<Telemetry> {
    let store = TELEMETRY.store().clone();
    let s = store.clone();
    cx.thread(move |stop| while !stop.wait(Duration::from_millis(50)) { s.write_fields(..) });
    store
}
```

- The function runs once per process (`ServiceSlot`, a `OnceLock`), at **app start: when the process creates
  its first `Host`** (`Host::new` calls `svelters::exports::start_services()`, idempotent). `#[svelters::service(
  lazy)]` starts on first use: when JS evaluates the module that exports it.
- The value (`V: IntoJs + JsType + Clone + Send + Sync`: a `Store<S>` or `Arc<T>` of a shared object) is the
  module-level `export const <name>`; every JS runtime of the process gets the same Rust object.
- `ServiceCx`: `thread(|stop| ..)` (a named thread joined at shutdown), `stop_token()` (`StopToken::wait(d)`
  is an interruptible sleep), `on_stop(f)`, and `spawn(fut)` with the `tokio` feature (aborted at shutdown).
- **Shutdown**: `svelters::exports::shutdown_services(timeout)` fires every token, runs the `on_stop` hooks,
  joins the threads (a `ShutdownReport` lists the ones that missed the timeout). The example calls it after
  `host.run()` returns and the host is dropped (note §8's order). Never at a hot reload.

### Events

```rust
#[derive(svelters::Events, Clone)]
pub enum SensorEvent { Reading { value: f64, seq: u32 }, #[event(latest)] Level(f64), Reset }
#[svelters::object(shared, module = "rx")]
pub struct Sensor { events: Emitter<SensorEvent>, .. }        // the macro finds the Emitter field
self.events.emit(SensorEvent::Reading { value, seq });         // any thread
```

- JS: `sensor.on('reading', (e) => ..)` (`NativeObject.on` → `host.listen`): the host subscribes to the
  object's `Emitter` (one `EmitSub` per host and handle: a listened-events mask) and names the signal.
- `emit` checks the listening hosts first: an event nobody listens to is dropped before it is cloned or
  converted. Otherwise the payload is queued (boxed, converted on the JS thread with the object table at hand)
  in the host's `Hub`, in order. `#[event(latest)]` events keep one slot per (host, object, event): delivered at
  most once per checkpoint, with the last value, at the position of the first occurrence since the last
  checkpoint.
- At the checkpoint (`collect`) payloads become ordinary signal events, delivered after the change batch (a
  handler sees the store values written before the emit), in emit order per object.
- Payloads: unit variant → no argument; one-field tuple → that value; struct variant → a plain object
  (camelCase keys). Debug builds warn once per object above 1000 events in one checkpoint.
- Typings: one `on(event: 'name', fn: (e: T) => void): () => void` overload per event (no catch-all, so a wrong
  event name or payload type fails svelte-check).

### Streams

```rust
#[js(worker, buffer = 4)]
pub fn count_to(n: u32, out: Sink<u32>) -> Result<u32, Closed> { for i in 1..=n { out.send(i)?; } Ok(n) }
pub fn numbers(&self) -> Stream<u32> { let (tx, rx) = channel(16); /* tx to a thread */ rx }
```

- A `Sink<T>` parameter (offloaded members only: worker, blocking, `async fn`, tokio) or a `Stream<T>` result (a
  plain fn; the call returns at once) makes the member a stream: JS gets a `NativeStream<T>` (async iterable,
  `batches()`, `close()`) instead of a Promise.
- Built on E1's op table: the stream is bound to the call's op (`OpShared` stream slot); items are pushed to the
  op's completion queue (`push_items`, one notification until taken) and settled with the other completions in
  the one engine entry per checkpoint (`OpOutcome::Items`, `settleOps` kind 3). The last items of a call that
  ends go right before its end in the same batch.
- **Bounded**: at most `buffer` (default 8) items unread: queued in Rust or handed to JS but not yet taken by the
  iterator; the iterator acks what it hands out (`native.streamAck`). `send` blocks (a condvar), `feed(x).await`
  waits (async producers), `try_send` fails with `Full`.
- **Closing**: leaving a `for await` early (`return()`), `close()`, an abort of `options.signal`, a hot reload
  (generation retire) or the host going away cancels the op; the stream's cancel hook closes it, so the
  producer's next `send` returns `Err(Closed)` (`Closed` converts to `NativeError`/`String`/`ExportError`).
  A stream dropped unread is closed when the GC collects its iterator (the op table reaches it only weakly), so
  a blocked producer is never stuck forever.
- **The end**: a `Sink` stream ends when its call returns (an `Err` rejects `next()` after the items sent before
  it); a returned `Stream` ends when every `Sink` of its channel is dropped, or with `Sink::fail(e)`.

### Persist and hot reload

- JS: `persist(key, () => new Simulation(p))` (`@svelters/types/core`) returns the object persisted under `key`
  by an earlier runtime (as its class: generated modules `registerClass` theirs), else `make()`'s result,
  persisted now. `unpersist(key)` forgets it. Host op `__svelters_host.persist(key[, obj|null])`; the host keeps
  the Rust object (not the handle) per host. Without a host that persists (Node TestHost) it is `make()`.
- **Handles across a reload**: app object entries carry the generation of the runtime that last received them.
  After a successful reload the host releases the old runtime's handles that the new one did not receive again;
  a persisted object, an app-wide store, a service's value or any Rust object the new runtime got back keeps
  its handle (E0 released them all, which would have killed a handle the new runtime had just received). JS
  objects that are not persisted die with their runtime (their handles are released, their `Drop` runs).
- The old runtime's observations of and listeners on app objects are set aside while the new bundle loads:
  dropped if it loads, restored if it fails (so the old runtime keeps getting store changes once, not twice).
  A failed load's persisted objects are dropped (the persist map is restored).
- **Persisted state and E2 buffers.** A JS-originated `Buffer<T>` kept long-term in Rust (inside a persisted
  object, a store, a service or a static) pins the JS runtime that allocated it: E2's teardown then waits 1 s at
  the reload and leaks that runtime. A persisted object is opaque to the host, so E3 cannot copy for it;
  instead the reload **diagnoses** it: after the old runtime's calls are retired and its stale handles
  released, and before it is dropped, a live transfer token count above zero logs a console warning naming
  the cause and the fix (`Buffer::into_vec()` copies JS memory into Rust memory). Tested both ways
  (`a_persisted_object_holding_js_memory_is_diagnosed_at_reload`: the held buffer warns once, leaks one
  runtime and waits ≥ 0.9 s; the `into_vec` copy warns nothing, leaks nothing, reloads fast). Persisted objects
  and stores are also dropped in `Host::drop` before the engine (after E2's app-object release), so shutdown
  does not hit the leak through them.

### Typings, stubs, schema

- `.d.ts`: store classes (`private constructor()`, `readonly` read-only fields, writable fields assignable, a doc
  line), `export const <value>: <Class>` with a doc line (static/service), event overloads, `NativeStream<T>`
  results; `NativeStream`, `persist`, `unpersist`, `registerClass` in `@svelters/types/core`.
- Stubs: store getters `observed(...)`; values `export const x = /* @__PURE__ */ ((x) => wrapObject(C, x))(
  callStatic('app:m', 'x', []))`; `stream(...)`/`streamStatic(...)`; `registerClass(...)`.
- `svelters-exports/1`: classes gain `store`, `events`; store props `observability: "exact"`, `verified`,
  `proportional`; members `stream` (item type); modules `values` (`kind`: static/service).

### `bind:` and the compiler

`<scale bind:value={telemetry.speed}>` already lowers to `bind_prop`: the lowering is decided by the element
prop's observe fact (`Gtk.Range:value` is exact + delivery-verified), and the bound value only needs to be an
assignable member expression, which the compiler handles. **No compiler change was needed for `bind:` on a
writable store field**; E3 adds a golden (`crates/svelters-compiler/tests/r2/store-field-gtk.svelte`: `scale`,
`spin-button`, `entry` bound to store fields, no diagnostics with `--strict-native`) and records store
observability in `exports.json` for E5. Reading `exports.json` in `PlatformSemantics::native_type`/
`type_property` (so `native_read_not_observable` and a `bind:` on a read-only store field are diagnosed at
compile time) is E5; today a write to a read-only field throws a `TypeError` at run time, and svelte-check
rejects it through the `readonly` typing.

## Deviations from the note

- **Writers**: `update` / `write(Field, f)` / `write_fields` / `touch` instead of generated per-field setters
  (`SIM_STATS.set_running(true)`): a generated extension trait would need importing; the field enum is checked.
- **`on_write(&mut self, field)`** runs after the assignment, typed by the field enum (the note had
  `on_write(&mut self, field, v: &Value)` before it). It normalizes in place or returns `Err` (old value restored,
  JS throws). Implementing `StoreHooks` is enough (no attribute).
- **`StoreStatic` reaches JS through `#[svelters::export]` on the `static`** (the note left the link open).
- **Services start with the first `Host`**, not at `App::run` (there is no `App` type yet).
- **Store fields with `#[js(bigint)]` are rejected** at compile time: the change batch and event delivery decode
  values without the member's slot (stream items do use theirs).
- **Event payload objects** cross as values, but the stubs do not wrap object handles inside event payloads into
  classes (records with object fields neither, as in E0).
- **`WorkCx::emit`** (the note: "emit for the owning object's events") is not added: a worker method holds
  `self: Arc<Self>` and calls `self.events.emit(..)`.
- **`svelters::persist` on the Rust side** is not added: Rust keeps objects alive with statics, stores and
  services; `persist` is the JS-side registration of a JS-created object.
- **svelte-check in the example runs with `--diagnostic-sources js`** (as `@svelters/native-runtime`'s own check
  does): svelte's DOM rules reject `bind:value` on a native `<scale>`; the svelters compiler validates bindings
  during `vite build`, and the typings are still enforced (a deliberately misused store and event fail).

## Measurements

Machine: WSL2, 16 cores, load average 9–12 during the runs (other lanes building). Release builds unless noted.

**10k worker writes, one batch** (`crates/svelters/tests/reactive.rs` `ten_thousand_worker_writes_are_one_batch`,
log backend, no checkpoint during the writes): **1 `__svelters_changed` batch, 1 field delivered** (`count`,
value 10000; `energy`, also observed, untouched and not delivered); the checkpoint that delivered it took
**0.14–0.18 ms** (release, 2 runs).

**Store burst vs event burst on GTK** (`examples/exports` test `store_burst_is_batched_event_burst_is_not`,
`SVELTERS_BURST=10000`, release, Xvfb, Svelte effects in the real app, the GTK loop and the telemetry service
running meanwhile; 3 runs):

| Path | 10k updates from a worker | Change batches | Svelte effect runs | Click to final value |
| --- | --- | --- | --- | --- |
| store (`BURST.write(..)` × 10k) | coalesced per checkpoint | 3–4 over 7–8 checkpoints (this count includes the telemetry service's own ticks) | **2–3** | **1.7–5.9 ms** |
| `Emitter` events (`emit(Value(i))` × 10k) | one event each | n/a | **10000** | **1174–1338 ms** |

So the store path is 200–700x faster end to end, and runs the dependent effect 2–3 times instead of 10000.
R1 measured the same shape for native props (10k changes: 1 batch, 0.2 ms; per-event 0.84–1.0 s). Debug build,
2000 updates: store 1 effect run, 7.5 ms; events 2000 effect runs, 358 ms.

**`bind:` on a writable store field (GTK)** (`telemetry_store_drives_a_bound_scale`): a Rust write
(`setSpeed(4)`) moves the scale to 4; a user drag to 15 is clamped by `on_write` to 10, the scale follows, and
the dependent effect runs 1–2 times (JS's own write, then the clamped value) with no echo loop. The exact
"delivered once, echo dropped" accounting is asserted on the log backend (`writable_field_clamps_once_and_drops
_the_echo`: one `changed speed=10`, nothing on later checkpoints; `accepted_writes_echo_nothing_...`: JS's own
accepted writes deliver nothing, a Rust write delivers once).

## Tests

- `crates/svelters/tests/reactive.rs` (18): stores (10k writes one batch; `update` + compare drops the
  unchanged; clamp delivered once, echo dropped; accepted writes echo nothing, rejected writes throw and keep the
  old value, read-only fields throw; proportional invalidations; getters subscribe through the tracker; a live
  non-scripted host); events (order, `latest`, after the change batch, an unheard event goes nowhere, `off`;
  flood warning, debug); streams (bounded and ordered with backpressure, early break closes, a stream dropped
  unread is closed when collected, abort rejects and closes, an error after the items, returned streams and
  `batches()`, a returned stream on a UI object); persist + app-wide store across a failed and a good reload
  (the unpersisted object dies, the persisted one and the store live on, observations counted once, a failed
  load's persisted object released); a persisted object holding a JS `Buffer<T>` diagnosed at reload (E2 x E3);
  typings/stubs/JSON.
- `crates/svelters/tests/services.rs` (1, its own binary: services are process-wide): eager start with the
  first host, lazy start on import, survives a reload (no restart, the new runtime reads the same store),
  shutdown joins the thread.
- `examples/exports/tests/exports.rs` (+2, GTK): `telemetry_store_drives_a_bound_scale`,
  `store_burst_is_batched_event_burst_is_not`; `committed_typings_are_current` covers `telemetry.js`.
- `crates/svelters-compiler/tests/r2_observe.rs` (+1 golden): `store_field_gtk`.
- `packages/types/test/stream.test.mjs` (4): `NativeStream` order/acks/end, early break, error after items,
  `batches()`, `persist`/`unpersist`/`registerClass`.
- `examples/exports/app`: `pnpm test` (vite build + svelte-check against the dumped typings): 0 errors; a
  deliberately misused file (assign a read-only store field, wrong event name, wrong payload type) gives 4 errors.

Local gates and CI: see "Completion record" below.

## Open items

- **E5**: `PlatformSemantics::native_type`/`type_property` from `exports.json` (compile-time diagnostics for
  read-only store fields, unknown members); the JSON already carries store observability.
- **Per-item diffs** for collection fields (`Observable<T>`/`Vec` diffs for `@svelters/lists`): future work per
  the Q7 decision.
- Object handles inside event payloads and records are not wrapped into classes by the stubs.
- `#[js(bigint)]` store fields and bigint event payloads (the batch and event paths decode without the slot).
- The GTK `wake` runs at idle priority (FU2): the telemetry service wakes the loop every 50 ms; the tests pump
  the loop themselves and use no busy timer.
- Pre-existing, not E3: `crates/svelters/tests/exports.rs` `identity_dispose_and_gc` fails in `cargo test`'s
  shared-process mode (a process-wide drop counter raced by a concurrent test); nextest (separate processes, as
  CI runs it) passes. The E3 test binaries pass in both modes and pinned to two cores.
- Node TestHost: app exports (and so stores, events, streams) remain QuickJS-host only.
- A "detach to Rust memory" path for JS buffers held across a reload (copy once at reload instead of leaking the
  runtime, E2's open item) would make the diagnosed case work; E3 only warns. `Shared<[T]>` (an `Arc`) is the
  right type for buffer-valued store fields.
- A store field holding a buffer value never compares equal (`same_value` has no buffer arm), so it is
  delivered whenever it is marked.

## Merge with E2 and FU2

`origin/master` (E2 zero-copy buffers, FU2 GTK wake priority) was merged into `ci/e3` (merge commit bd797eb9).
Conflicts and how they were resolved:

- `exports/offload.rs`: the completion queue keeps both lists (E3 `items`, E2 `released`) and both method sets.
- `lib.rs` (`Host::drop`): E2's app-object release first, then E3's `reactive_shutdown` (hub detached,
  subscriptions and persisted objects dropped), both before the engine.
- `reload.rs` (success branch): E2's order (calls retired and app objects released before the old runtime is
  dropped) with E3's generation-aware release (`app_release_stale`: re-received handles survive; E2's
  `app_release_all(old)` would have killed them) and the buffer diagnostic before the drop.
- `exports/codegen.rs`: E3's values, events and `NativeStream` import moved into E2's `ts_module_body`.
- Facade re-exports and prelude, the design note's status line, and the example's `lib.rs`, `App.svelte` and
  `tests/exports.rs` keep both lanes (E2's DSP panel and test next to E3's telemetry). The generated
  `.svelters/app` files were regenerated with `--dump` (`committed_typings_are_current` checks `dsp.js` and
  `telemetry.js`).
- Auto-merged and checked: `core/offload.rs` `drain` (stream items, then completions, then stream tails, then
  E2's buffer releases; `ops_reap` only completes settled or retired calls, which never produce stream tails),
  `core/apps.rs` `app_set_prop` (E2's `owned_all`, then E3's read-back; a `BufferValue` clone is an `Arc`
  clone), `Value::Buffer` (the compare's catch-all arm treats buffers as never equal).

## Conflict risks

Shared structures changed (pushed early, in the first WIP commit 2d09642d):

- `crates/svelters-host/src/exports/mod.rs`: `Sig` gains `items: Option<TypeRef>` (every `Sig { .. }` literal
  must set it: the macros, `schema.rs`); new modules `reactive`, `service`, `stream`; re-exports; `__private`
  additions.
- `exports/schema.rs`: `ClassSchema` gains `store`, `events`, `emitter`; `ModuleSchema` gains `values`;
  `Schema::resolve` answers `NameKind::Signal`, `member` returns `None` for it; new inventory tables
  `StoreDef`, `EmitterDef`, `ValueDef`, `ServiceDef`.
- `exports/offload.rs`: `OpShared` gains a `stream` slot (`set_stream`, `stream`); `Queue` gains `items`
  (`push_items`, `take_items`); `tokio_handle()`.
- `exports/objects.rs`: entries carry a generation; `adopt` updates it; `object`, `begin_generation`,
  `not_received`, `restore_generation`.
- `exports/codegen.rs`: imports line of the stubs (adds `observed`, `stream`, `streamStatic`, `registerClass`),
  store getters, values, stream members, event overloads, JSON keys.
- `core.rs`: `Core.reactive`; checkpoint step 1b `reactive::collect` (after `offload::drain`); `call_sync`
  collects; `take_changes`/`read_last` app-handle branches; `take_events` app kinds.
- `core/offload.rs`: stream items in `drain` (before completions), `op_complete` stream tail, `op_items`,
  `op_stream_ack`, `pending_items`; `core/apps.rs`: set path read-back, `app_forget`, generation helpers;
  `core/ops.rs`: `op_observe`/`op_listen` app routing.
- `engine.rs`: `OpOutcome::Items`; `quickjs/conv.rs` (`op_value` kind 3); `quickjs.rs` (`native.streamAck`,
  `__svelters_host.persist`); `prelude.js` (`makeOp` stream port, `settleOps`/`settleOpSoon` kind 3).
- `reload.rs`: generation begin/stale release/restore, reactive stash.
- `lib.rs`: `start_services()` in `Host::with_engine`, `reactive_shutdown()` in `Drop`, `reactive_stats()`,
  `persisted()`.
- `crates/svelters-macros/src/lib.rs`: `Js.buffer` (`#[js(buffer = n)]` no longer an error), `ArgKind::Sink`,
  `Stream<T>` results, `generic_arg`, `pascal`, Emitter detection in `#[svelters::object]`, new `store`,
  `service`, `Events`, `export` on statics.
- `crates/svelters/src/lib.rs` (facade re-exports, prelude), `packages/types/src/core.{js,d.ts}`,
  `examples/exports` (lib, main, app, committed `.svelters/app`, tests, `package.json` test script).
