# quickjs-jit and a Cranelift JIT for QuickJS-NG

Status: research complete (2026-10-16/17). Question: can we take longbridge/quickjs-jit's ideas and
build a JIT for QuickJS-NG with Cranelift instead of falling back to V8 (deno_core)?

## 1. What quickjs-jit is

Facts below come from reading the source at `96593cd` (v0.12.11, 2026-10-01), cloned to
`/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/jit/quickjs-jit`.

**The headline finding: quickjs-jit already *is* a Cranelift JIT for QuickJS-NG, packaged as an
rquickjs fork.** The question "take its ideas and redo them with Cranelift" is already answered
by the project itself. What is left to decide is whether to adopt it, fork it, or write a
smaller one with the same design.

### Identity, provenance and maturity
- GitHub `longbridge/quickjs-jit` is a fork of `DelSkayn/rquickjs`, created 2026-08-29. It
  forked upstream rquickjs **0.12.2** (commit `11c4cac`, 2026-08-24). Crates: `quickjs-jit`
  (lib name `rquickjs`, so it drops in for source compatibility), `quickjs-jit-core`,
  `quickjs-jit-sys`, `quickjs-jit-macro`, and `quickjs-jit-runtime` (lib `rquickjs_jit`, the JIT).
- Engine: **QuickJS-NG v0.15.1** (`fd0a021`, 2026-06-04) as a submodule. On top of it sit **28
  patches, about 9.3k lines** (`sys/patches/0001..0032`). They are applied into `OUT_DIR` at
  build time. The JIT pins `SOURCE_REVISION = 0xfd0a_0210_b7be_0095` and fingerprints the opcode
  table, the helper table, the JSValue size and the refcount offset. If any of these mismatch,
  the JIT refuses to attach.
- Our host (P2 worktree) uses **rquickjs 0.14.0 with QuickJS-NG 0.16.2**. quickjs-jit is two
  rquickjs minors and one NG minor behind.
- Author: effectively one person. All 28 JIT commits are by Jason Lee (`huacnlee`, Longbridge;
  he also wrote gpui-component). The whole JIT is 6 weeks old (2026-09-01 → 2026-10-01). Commits
  are huge: 109k, 75k and 30k lines inserted in single PRs. The repo carries `AGENTS.md`,
  `.superpowers/` and `docs/superpowers/{plans,specs}`, so it is evidently written by coding
  agents. Some internal docs are in Chinese. 16 stars, 3 forks, 0 issues.
- Size: `jit/src` has 61 Rust files and **59k lines**. The two largest are
  `compiler/optimized.rs` (11.9k) and `compiler/baseline.rs` (9.5k). There are about 45
  integration test files, a Test262 runner and a fuzz target.
- License: MIT, same as rquickjs and QuickJS-NG.
- "Production" means Longbridge's **gpui-shell**: a GPUI desktop shell that renders UI from
  QuickJS scripts. `integrations/gpui-component/rquickjs-jit.patch` wires it in.
  `docs/M0.md` reports the result: a numeric layout kernel ran **~40x** faster, but the
  **host-heavy 443-node panel was 0.99x–1.00x, i.e. no gain**. No other users were found. The
  published crate's README says `0.12.3` runtime "has not been published yet"; the docs are
  ahead of the releases.

### Architecture
- **Method JIT with two tiers, not tracing.** Both tiers use **Cranelift 0.116**
  (`cranelift-codegen/frontend/module/object/native`, plus `gimli` for unwind info). Current
  Cranelift is 0.134, so it is 18 releases behind. No dynasm, LLVM or sljit.
  - **Tier 1, "baseline"** (`compiler/baseline.rs`): a template-like translation of QuickJS
    stack bytecode into Cranelift IR. It operates **directly on the interpreter's frame**
    (`var_buf`/`stack_buf`, flattened as args, locals, operand stack). Most opcodes become calls
    to C helpers that take `(JSJitExecFrame*, stack_map_id, slot indices…)` and return a status.
  - **Tier 2, "optimized"** (`ir/optimized.rs`, `ir/scalar.rs`, `compiler/optimized/*`): a
    speculative SSA tier fed by type feedback. It has unboxed Int32/Float64 values with loop phis,
    monomorphic/bounded-polymorphic property guards (shape pointer + "shape generation" token),
    packed-array and typed-array fast paths with bounds-check elimination, guard LICM, bounded
    inlining with shadow frames, and direct compiled-to-compiled calls.
- **Interpreter integration** (patch `0001`): hooks inside `JS_CallInternal`. On call entry and
  on taken back-edges (every 64), the interpreter reports `record_hot` to a backend vtable
  (`JSJitBackendVTable`). On request it builds a `JSJitFunctionSnapshot` (bytecode, constants,
  exception map, source map) for **background compilation**. Next time it calls
  `acquire_entry(id, generation, pc)`. A non-zero `pc` is an OSR entry at a loop header. The
  native code then runs on the same frame and returns an exit kind: `DONE`, `EXCEPTION`,
  `INTERRUPT`, `DEOPT` or `RETRY_INTERPRETER`.
- **Deopt/bailout**: because native code keeps the interpreter frame as the source of truth at
  every helper and exit boundary, deopt amounts to "write back unboxed values, set pc/sp, return
  DEOPT". The interpreter then resumes at that pc in the same frame. Tier 2 inlining adds
  runtime-owned "shadow frames" (patch `0018`, at most 16 per runtime, 1 MiB) so that an inlined
  callee can be materialised on deopt or exception. This is the cheap trick that makes a
  Cranelift JIT feasible without Cranelift having any deopt/stackmap support.
- **Unsupported code is fail-closed**: a function containing any non-admitted opcode is not
  compiled and runs interpreted. **Generators and async functions are never compiled**
  (`JS_JIT_SNAPSHOT_GENERATOR/ASYNC`; Tier 2 and inline frames also require
  `JS_FUNC_NORMAL`). `with` and direct `eval` are excluded too. Over time Tier 1 gained closures
  (`fclosure`, var refs, `close_loc`), constructors, globals, RegExp literals, try/catch regions
  and for-in/for-of. However, the 0.12.11 auto-tiering policy deliberately keeps closure,
  generic-op, exception and iteration functions **interpreted** until profitability is proven,
  because those regressed.
- **Refcounting/GC**: the helper ABI (`quickjs-jit-helpers.h`, an X-macro table) declares
  per-argument ownership (`BORROWED`/`CONSUMED`/`OWNED`) and effect flags (`THROWING`,
  `ALLOCATING`, `REENTRANT`, `FINALIZING`). Because all live values stay in frame slots that the
  cycle collector already scans, there are no stack maps for GC. Since P1b, DUP and non-final
  FREE are **inlined** (`ref_count++/--`, the count sits at offset 0 of every refcounted
  payload, which a static assert enforces). The last-reference FREE still calls C, so finalizers
  and GC lists stay in C. A `STRESS_GC` frame flag forces a GC before and after every helper in
  tests.
- **JSValue**: it relies on the default 64-bit **16-byte struct JSValue** (`{union u; int64
  tag}`), not NaN-boxing. It validates `value_size` at attach time. 32-bit/NaN-boxed builds are
  not supported, and `wasm` builds are interpreter-only (Cranelift is not a dependency there).
- **Exceptions**: helpers return `-1`. Native code jumps to an exit with `EXCEPTION`, and the
  interpreter's own exception-region machinery (patch `0027`) unwinds. Uncatchable errors and
  interrupts go through a `POLL` helper on back-edges.
- **Inline caches**: these live in the JIT, not the interpreter. Feedback probes in the
  interpreter record shapes, call targets, arithmetic types and array modes. Patch `0012` retains
  up to 64 shapes per context so short-lived receivers can still hit. Patch `0017` adds a
  runtime-unique shape token invalidated on descriptor mutation, which grows the shape header
  from 64 to 72 bytes.
- **Platforms**: x86-64 and aarch64 on Linux, macOS and Windows (`platform/mod.rs`). Code
  memory is W^X. On macOS it uses `MAP_JIT` + `pthread_jit_write_protect_np`, or an embedder
  allow-list callback. Unwind info is registered (gimli/`__register_frame`, Windows CFG). There
  is a host capability probe that falls back to the interpreter when W^X can't be satisfied.
  iOS is not mentioned; it can't JIT anyway. Measurements exist for Linux x86-64 (i7-13700KF)
  and macOS arm64 (M3).
- **Debugger/source maps**: QuickJS-NG has no debugger to break. Backtraces keep working because
  native code updates `sf->cur_pc` at helper boundaries (the "helper PC cursor"), so
  `Error.stack` line numbers, and therefore our source-map path, stay valid.

### What it claims (its README, Linux x86-64, ms per 10 calls, automatic tiering)
- Numeric/scalar kernels: **25–50x** faster than the interpreter and **0.4–0.7x Bun**
  (`scalar-loop`, `fibonacci-iterative`, `bitops`, `float64-dense`, packed-array and typed-array
  traversals). `property-heavy` (monomorphic) is 37.8x.
- **Everything that looks like app code is ~1.0x**: `calls-closures` 1.00x,
  `calls-recursion-closures` 1.00x, `objects-polymorphic` 1.3x, `property-polymorphic` 3.3x,
  `strings-json` 0.96x, `json-codec` 0.96x, `collections` 0.98x, `map-set-bigint` 0.99x,
  `exceptions-promises-async` slower, `for-of-array` 1.00x, `strings-regexp` 1.02x. On these
  workloads Bun is **20–80x** faster than either QuickJS or quickjs-jit.
- Correctness claim: identical Test262 results in interpreter and auto-JIT mode (80,371
  passes). That is credible for the admitted subset, because unsupported code simply isn't
  compiled.

## 2. Reproducing it with Cranelift

quickjs-jit is the existence proof, so this section explains why its design works and what any
reimplementation would have to copy.

- **Cranelift's fit.** Cranelift is a fast, mid-level optimizing backend: roughly an order of
  magnitude faster to compile than LLVM, but far slower than a template or copy-and-patch
  emitter that pastes pre-built machine code per opcode. For a baseline tier that is acceptable
  only if compilation is **off the main thread**, and quickjs-jit does exactly that: background
  compile, then `acquire_entry` on a later call. Cranelift has **no deopt, OSR, or
  inline-cache-patching story**. It also has no runtime code patching (no self-modifying IC
  stubs) and no aggregate C ABI. quickjs-jit works around all of these:
  - *Deopt/OSR*: the interpreter frame is the only frame, so "deopt" means returning to
    `JS_CallInternal` with `pc`/`sp`. OSR entry is a separate compiled entry block per loop
    header.
  - *ICs without patching*: guards compare against shapes baked in at compile time from
    feedback. A miss deopts, and recompilation produces new code ("versioning, retry,
    demotion"). This is a speculative-JIT approach, not a classic patchable IC.
  - *Code memory*: it uses `cranelift-object` plus its own allocator, not the `cranelift-jit`
    crate's `JITModule`, so that it controls W^X, unwind registration, macOS `MAP_JIT`, Windows
    CFG and a serialisable code cache. `cranelift-jit` would be fine for a prototype. It does
    `mmap` + `mprotect` and has a `JITBuilder::symbol` mechanism for importing C helpers by
    address.
- **Calling QuickJS C helpers.** On 64-bit QuickJS-NG a `JSValue` is a **16-byte struct**
  `{ union { int32; double; void* } u; int64 tag; }`. NaN-boxing is only the default for 32-bit
  builds (`JS_NAN_BOXING`). Cranelift cannot pass C structs by value. On SysV x86-64 a 16-byte
  two-INTEGER struct can be faked as two `i64` params, but on Win64 it goes by hidden reference
  and on AArch64 the rules differ again. quickjs-jit's answer is the right one: **helpers take
  `(JSJitExecFrame*, stack_map_id, u32 slot indices)` and read and write `JSValue`s in frame
  memory**. Generated code only does loads and stores of `(payload: i64, tag: i64)` pairs at
  `frame_base + 16*slot`, with no struct ABI at all. The helper table is an X-macro, so the C
  declarations, the Rust metadata and the Cranelift signatures cannot drift.
- **Inline refcounting.** DUP is `if ((unsigned)tag >= JS_TAG_FIRST) (*(int*)ptr)++`. FREE
  decrements inline only when `count > 1` and calls C (`__JS_FreeValueRT`: finalizers, GC lists,
  possible re-entry) for the last reference. **On QuickJS-NG 0.16 (the version we use) this
  changes**: `ref_count`, `gc_obj_type` and `mark` moved out of the object into an 8-byte
  allocator block header *before* the pointer (`js_rc(p)`, `quickjs.c:504`). Inline DUP/FREE
  would use offset −8 and depend on the allocator being QuickJS's own. That is a concrete example
  of the layout churn any JIT must track.
- **What would and would not speed up a Svelte 5 app.** Svelte 5's hot path is
  `get`/`set`/`mark_reactions`/`update_reaction`/`execute_derived`/`flush` in
  `internal/client/runtime.js` and `reactivity/*`. That code is short functions doing property
  loads on a handful of shapes (Source, Derived and Effect objects with flags bitfields), array
  push/index on `deps`/`reactions`, closure calls, and — for `$state` objects and arrays — **Proxy
  traps** (`proxy.js`), which QuickJS executes as C → JS → C round trips. A baseline JIT removes
  dispatch overhead (typically 20–40% of a QuickJS interpreter profile), but every property
  access, call, closure creation and proxy trap still goes through the same C runtime functions.
  Without shape-specialised property access and direct JS→JS calls, the ceiling is ~1.2–1.5x.
  quickjs-jit's own matrix shows that even *with* those, closure- and object-heavy code is at
  1.0–1.3x. Our measurements are in §5.
- **QuickJS-NG and inline caches.** NG merged polymorphic inline caches in v0.2.0 (PR #120,
  2023-11), reworked them (#609, 2024-10) and **removed them in #884 (2025-02)**. Ben Noordhuis
  measured web-tooling-benchmark with and without ICs and got a mixed bag, from 0.94x
  (babel/babylon/buble) to 1.42x (babel-minify). The ICs were "always more memory hungry", and
  they cost something for run-once code (issue #876). Upstream's stated plan was to "start
  afresh"; nothing has landed through 0.16.2.

## 3. Prior art

Marked *(memory)* where it comes from background knowledge rather than a source check today.

- **PrimJS** (lynx-family/primjs, Apache-2.0, 1.15k stars, active): a QuickJS fork for Lynx. It
  has a **template interpreter** (per-opcode machine-code templates with stack caching and
  register pinning), a tracing GC replacing refcounting, and object-model hooks for Lynx. It
  claims **~28% over QuickJS on Octane** (3735 vs 2904). There is no JIT. It shows the order of
  magnitude available from dispatch work alone, and that it requires diverging from upstream
  QuickJS.
- **Hermes** *(memory)*: no JIT by design. It relies on AOT bytecode for startup and a
  generational GC. Static Hermes adds AOT native compilation of typed JS. Meta's conclusion for
  UI apps was that startup and memory mattered more than peak throughput.
- **V8 Sparkplug** *(memory)*: a non-optimising baseline compiler that walks Ignition bytecode
  and emits calls to the same builtins/IC stubs on the same frame layout as the interpreter, so
  tier-up/down is trivial. This is architecturally what quickjs-jit's Tier 1 is. V8 reported
  single-digit to ~15% wins on real web workloads; most time is in builtins and ICs, not
  dispatch.
- **SpiderMonkey** *(memory)*: Baseline Interpreter → Baseline JIT → Warp. The **Baseline
  Interpreter** (2019) is an interpreter that uses the same CacheIR inline-cache stubs as the
  Baseline JIT. It was added because ICs, not native code, delivered most of the baseline gain,
  and compiling everything cost too much on real pages. Chris Fallin's *Portable Baseline
  Interpreter* (for Wasm, where JIT is impossible) and **weval** (partial evaluation of the
  interpreter + IC bodies, AOT) pushed this further for StarlingMonkey.
- **JSC** *(memory)*: LLInt (an offlineasm interpreter with ICs) → Baseline JIT → DFG → FTL.
  Same lesson: the IC-equipped interpreter is the cost-effective first step.
- **LuaJIT-style tracing** *(memory)*: huge wins for numeric loops. Tracing a JS semantic surface
  (proxies, getters, exotic arrays) is a multi-year project, and UI code is branchy and
  polymorphic, which is the worst case for traces.
- **Cranelift for dynamic languages** *(memory)*: Wasmtime (its main client),
  rustc_codegen_cranelift, RustPython's `rustpython-jit` (a typed-subset JIT, never general),
  and SpiderMonkey's Wasm tier, which used Cranelift and was removed in 2022. Ruby's YJIT/ZJIT
  use their own Rust backends rather than Cranelift, citing compile latency and the need for
  code patching. quickjs-jit is the first serious Cranelift JS JIT I know of.
- **Other QuickJS JITs**: none found that are maintained. Several hobby forks ("quickjs jit" on
  GitHub) are toy x86 template JITs. "Elsa" is a Go runtime built on QuickJS, not a JIT. Not
  exhaustively verified.
- **Intermediate option, without native code generation.** In QuickJS-NG the interpreter
  already uses computed-goto dispatch (`DIRECT_DISPATCH`, on for GCC/Clang; off for MSVC and Emscripten), so threaded-code tricks are
  mostly spent. What remains:
  1. Interpreter-level ICs done better than NG's removed attempt (lazy allocation after N
     executions, monomorphic only).
  2. Superinstructions for common Svelte sequences.
  3. Cheaper Proxy and `Function.prototype.call` paths.
  4. Faster `get`/`set` on shapes.

  Expected gain is 10–30% on property-heavy code. It works on every platform including iOS and
  Wasm, and touches nothing about W^X.

## 4. Effort and risk

Rough person-weeks for one engineer who knows QuickJS internals moderately. These are planning
numbers, not commitments.

| Option | Effort | Expected gain on Svelte-style code | Notes |
| --- | --- | --- | --- |
| (a1) Adopt quickjs-jit 0.12.11 as-is (pin to its rquickjs 0.12.2 API / NG 0.15.1) | **~1 week** | measured in §5 | API delta 0.12→0.14 is small (`TypedArray::as_bytes` became unsafe, `StaticJsFunction` removed). It means **downgrading NG 0.16.2 → 0.15.1** and polling `JitRuntime` maintenance from our job pump. Wasm builds stay interpreter-only. |
| (a2) Rebase its 28 patches onto NG 0.16.2 / rquickjs 0.14, then keep rebasing | **3–6 weeks**, then **~1–2 weeks per NG minor** | same | Upstream 0.15.1→0.16.2 is 97 commits, ~3k lines changed in `quickjs.c`, including the refcount-into-allocator-header move that breaks `REF_COUNT_OFFSET = 0`, the static asserts in patch `0022` and JSObject/shape layouts. 59k lines of agent-written Rust with a bus factor of one. We would own it. |
| (b) Our own Cranelift Tier 1 (helper-per-opcode on the interpreter frame, fail-closed, ~100 opcodes, background compile, W^X allocator) | **8–14 weeks** to parity with quickjs-jit's Tier 1; **+10–20** for a feedback-driven Tier 2 with shape guards and direct calls | Tier 1 alone: ~1.0–1.3x (Sparkplug-class); Tier 2 needed for more | Needs a frame/helper ABI patch in QuickJS-NG just like quickjs-jit's `0001` (~2.5k lines). Copying their ABI design is the sane route; MIT allows copying code. |
| (c) Interpreter inline caches only (C patch to NG; lazy, monomorphic `get_field`/`put_field`/`get_array_el` on shape pointer) | **3–6 weeks** | ~1.1–1.3x on property-heavy code, ~0 on proxies | NG removed its previous attempt for mixed results and memory. It works on iOS/Wasm and is upstreamable, so maintenance could be shared. |

**Correctness risks** (any JIT):
- *Refcount leaks or double frees* on rarely-taken paths (exception in the middle of an opcode,
  deopt between DUP and store). quickjs-jit mitigates these with ownership-annotated helpers, a
  stress-GC frame flag and Test262 parity. A homegrown JIT would need the same harness before
  anyone trusts it.
- *Exceptions/interrupts* must leave the frame exactly as the interpreter would.
- *Reentrancy*: getters, proxies and finalizers can run arbitrary JS, invalidate shapes and
  recompile mid-call.
- *Generators/async* are excluded by quickjs-jit, and our `$effect`/await-heavy code would stay
  interpreted anyway.
- *Debugging/source maps*: backtraces depend on the JIT keeping `cur_pc` correct at every helper
  boundary. quickjs-jit does; a bug here silently breaks M2's source-mapped errors.

**Security/platform:**
- W^X is needed everywhere. On macOS the hardened runtime needs the
  `com.apple.security.cs.allow-jit` entitlement plus `MAP_JIT`/`pthread_jit_write_protect_np`
  (quickjs-jit implements this).
- **iOS forbids JIT** for third-party apps, so the interpreter path must stay first-class.
- A JIT turns every QuickJS memory-safety bug into a more exploitable one. We only run our own
  bundled code, so the threat is lower than in a browser.

**Maintenance** is the dominant cost. QuickJS-NG ships a minor release every few months and
freely changes `quickjs.c` internals; the JIT is welded to private layouts (`JSObject`,
`JSShape`, `JSStackFrame`, opcode numbering). Every bump is a rebase plus re-validation. V8 via
deno_core moves faster still, but its embedding API is stable and someone else maintains the
JIT.

## 5. Experiments and numbers

**Build: it builds cleanly.** `CARGO_BUILD_JOBS=4 cargo build --release --manifest-path
benchmarks/Cargo.toml --bins` worked first time on rustc 1.97, Linux x86-64. That includes
Cranelift 0.116, the 28 patches applied into `OUT_DIR`, and the C build. Stock QuickJS-NG
**v0.16.2** (the version rquickjs 0.14 bundles) was built separately with CMake Release.

**Workloads** (all in
`/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/jit/bench/`):
- `w-signals.js`: the real **Svelte 5.57.2 `svelte/internal/client` runtime**, bundled to an
  IIFE with rolldown. 100 `$.state`, 100 `$.derived` (a+2b+c), 100 `render_effect`s writing into
  a plain array. Each call resets the sources and then runs 10 rounds of 5×`$.set` + `flushSync`.
- `w-proxy.js`: Svelte deep reactivity. `$.proxy` over 200 `{id,text,done}` rows, one
  render effect per row, a derived "remaining" count that iterates the proxy, and toggle/edit +
  flush.
- `w-appmix.js`: non-reactive app glue. Four object shapes, dispatch through a closure, small
  template strings, and `filter/map/join`.
- `w-props.js`: quickjs-jit's own monomorphic `property-heavy` kernel (the favourable case).

**Protocol**: quickjs-jit's own `jit-bench worker`, `shared-js-multibatch-v3`: 1 call, 64
warmup batches of 10, 16 timed batches. The value is the upper-median ms per 10 calls. The two
quickjs-jit modes (JIT detached vs production automatic tiering) use the same binary. Stock qjs,
node 22.22.2 and bun 1.3.14 run an identical JS harness (`harness.js`) appended to the same
`batch-driver.js`. All checksums matched across engines. Three interleaved rounds; the table
shows the median of the three.

**Caveat: the host was heavily loaded** (12 cores, load average 27–33 from other agents' builds).
Individual runs vary up to ±40%, so read ratios to ±25%. The qualitative picture is consistent
across all three rounds.

ms per 10 calls (lower is better):

| Workload | quickjs-jit, interp (NG 0.15.1+patches) | **quickjs-jit, auto JIT** | stock qjs NG 0.16.2 | node 22 (V8) | bun 1.3 (JSC) |
| --- | ---: | ---: | ---: | ---: | ---: |
| props (monomorphic kernel) | 8.24 | **0.40** | 7.21 | 0.19 | 0.32 |
| appmix (shapes/closures/strings) | 36.5 | **34.6** | 26.1 | 0.98 | 1.33 |
| signals (real Svelte 5 runtime) | 136.6 | **129.1** | 100.2 | 4.86 | 6.33 |
| proxy (Svelte `$state` proxies) | 415 | **448** | 341 | 55.0 | 45.3 |

Derived ratios:
- **JIT vs its own interpreter**: props **20.8x**; appmix 1.05x, signals 1.06x, proxy 0.93x. The
  last three are statistically ties. On signals the JIT *was* active: about 680k native entries
  per process, almost all in Tier 2, 0 deopts, 17 KB of code. It compiled the Svelte runtime's
  hot functions and they did not get faster.
- **JIT vs stock QuickJS-NG 0.16.2** (what we ship today): props 18x faster; **app-like
  workloads 0.76–0.78x, i.e. slower**. Simply being on NG 0.16.2 beats quickjs-jit on everything
  except the numeric kernel. NG 0.16 brought interpreter/allocator improvements, and the JIT
  fork's interpreter carries feedback probes.
- **V8 vs stock QuickJS-NG 0.16.2**: about **20x** on the Svelte signals graph, about 27x on app
  glue and about 6x on proxy-heavy code (proxy traps are slow in V8 too). That is the size of
  the gap a JIT would have to close.
- `perf` on stock qjs, signals workload: `JS_CallInternal` 71% self time (dispatch, inlined
  opcode bodies, frame setup for many tiny calls), `find_own_property` 7.8%, `JS_FreeValue` 3%,
  `map_find_record` 2.4%, GC ~2%. Appmix: 79% / 10.8%. Time is spread over many short calls,
  which is what a method JIT with helper calls and per-call native entry does not speed up. Only
  inlining, direct JS→JS calls and shape-specialised loads would, i.e. a V8/JSC-class optimising
  tier.

## 6. Recommendation

1. **Do not build our own Cranelift JIT, and do not adopt quickjs-jit now.**
   - quickjs-jit proves the Cranelift design works. Its interpreter-frame helper ABI with fail-closed
     fallback is the right blueprint if we ever do this.
   - It delivers its 20–50x only on numeric and monomorphic loops. On the real Svelte 5 runtime it
     was a tie with its own interpreter and **~25% slower than the stock NG 0.16.2 we already use**.
   - Adopting it means downgrading to NG 0.15.1 and carrying 9.3k lines of patches against
     private layouts, which NG 0.16 already broke (refcount moved into the allocator header).
   - It also means depending on 59k lines of six-week-old, agent-written, single-maintainer
     code.
   - A homegrown equivalent is 2–8 person-months, plus a permanent rebase cost, for a likely
     ≤1.3x on our workloads.
2. **If QuickJS throughput becomes the bottleneck, V8 (deno_core) is the lever that matches the
   gap.**
   - We measured ~20x on Svelte signal propagation and ~27x on app glue.
   - No QuickJS JIT design reachable at our scale closes that gap.
   - Keep QuickJS as the default and the iOS/Wasm/size-sensitive engine. Keep the op layer
     engine-neutral (see `node-deno-integration.md` §3.2).
3. **Measure first (P5).** Most of a native-widget app's frame time may be in toolkit layout,
   rendering and ABI crossings, not JS. Capture these in P5:
   - per-frame JS time vs host time;
   - time inside `flush`/`update_reaction`;
   - property-lookup share (`find_own_property`);
   - proxy-trap share;
   - GC pauses.

   Rerun `w-signals.js`/`w-proxy.js` on a quiet machine as the P5 microbaseline.
4. **Cheap wins before any engine change**, in order:
   1. Stay on current QuickJS-NG; 0.16.2 is already ~25% faster than 0.15.1 here.
   2. Keep the reactive graph off `$state` proxies in hot paths. The proxy workload was the
      slowest per update on every engine (not a like-for-like comparison). Use `$state.raw` and plain signals for widget-prop
      plumbing.
   3. Batch host calls.
   4. Profile with `perf` (the QuickJS functions are visible).

   Upstream-friendly interpreter ICs (option c) are the only QuickJS-side engine project worth
   considering, and only if P5 shows `find_own_property` above ~15%.
5. **Watch quickjs-jit.** Revisit if it rebases onto NG ≥0.16, gains a second maintainer, and
   shows wins on closure/object workloads (its `calls-closures` and `objects-polymorphic` rows).
   Re-measuring takes about an hour with the harness above.
