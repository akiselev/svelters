# Lane RUNTIME: native runtime and host (lazy nodes, event-time dispatch, fast ops)

Branch `ci/presto-runtime`. Owns `packages/native-runtime/src/index.js` (existing functions),
`crates/svelters-host/**`, `crates/svelters-abi/**` (additive), and svelters-egui's dispatch path where
event-time dispatch needs it (`crates/svelters-egui/src/frame.rs`, `events.rs` — only the outbox → event-time
change). Read: `compiler-and-js.md` §1, §3.3, §3.10, §5; notes `round2/js-host-local.md`,
`round2/zero-serdes-js.md`. Lane COMPILER adds `template.js` and new imports; coordinate through records.

Deliver (push after each):
1. **Lazy `Native` objects (W-C3):** no eager Maps/Sets; fields initialised in the constructor (no fall-through
   to the proxy prototype); `set` compares inline for typed scalar values; `$n.set` stays for proxies,
   functions, records. Move the last-value dedupe into Rust columns for hosts that support it (an ABI flag).
2. **Event-time dispatch (W-H1):** a host entry "dispatch and drain microtasks" (`call_sync` today drains
   nothing); widget signals (`clicked`, `changed`, `bind:`) delivered at event time instead of through the
   outbox on egui (behind `EmbedConfig::same_frame_reactions`, default on, with the R17 relaxation recorded as
   "Decided (user, 2026-10-25: follow the recommendations)").
3. **Fast ops (W-J1):** the hot host ops (`create`/`element`, `insert`, `setp` by id, `listen`) registered as
   raw `rquickjs-sys` C functions with `magic` = op/prop id, scalars read from the `JSValue` tag, ASCII strings
   borrowed in place, literal strings as atoms; typed `set_prop_ref` writes without an owned `Value` where the
   backend implements it. Keep the engine-agnostic `JsReader`/`JsWriter` path for everything else. Lazy
   wrappers hold (id, generation) in the opaque slot; handlers reported to the GC through `gc_mark`.
4. **`host.instantiate`/`destroy_range` (with lane COMPILER):** the ABI ops (optional; generic expansion when
   a backend lacks them) and the op-log expansion.
5. **The microbench (W-J0 code):** `crates/svelters-host/benches/boundary.rs` comparing rquickjs closure, raw C
   function, exotic class, with strings and the dedupe; run later on a quiet machine (record "not run").

Tests (write, don't run locally): runtime unit tests (`packages/native-runtime/test`), host tests for the
fast ops and GC marking (a handler capturing its node is collected after destroy), egui same-frame dispatch
(frame count 1 for `onclicked`, reusing `crates/svelters-egui/tests/input_latency.rs` from svelters branch
`ci/input-latency` if it merged, else a small new test).
