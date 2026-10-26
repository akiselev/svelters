# Zero-serdes JS ↔ scene-node bindings for tessel (QuickJS-NG / rquickjs)

Research notes for cadatomic's own retained toolkit ("tessel", Rust on wgpu) with Svelte 5 compiled by
svelters into JS that drives a small runtime (`$n.element/set/on/insert/remove/destroy`) over host ops.
Question: should we fork rquickjs and QuickJS-NG so that JS objects are the scene nodes and nothing is
converted between the layers? Written 2026-10-25. Web research and source reading only; nothing was
built or run.

Legend: **[doc]** stated by the primary source cited; **[src]** read in source at the named commit;
**[local]** measured earlier in svelters/cadatomic (files named); **[3rd]** measured or claimed by a
third party; **[inferred]** my reasoning from the cited facts (estimates are marked as such).

Sources read this session:
- quickjs-ng `master` @ `7a23d8e3fedb010070ef1a03873c55500c7aa1a9` (2026-10-24; last tag v0.17.0,
  2026-09-18). File:line references below are at this commit.
- rquickjs `master` @ `c0bef90967bd91fbe12ce882b81d280db94843d0` (2026-10-20; release 0.14.0,
  2026-09-17, which the workspace uses).
- PrimJS `develop` @ `60e1b418fb2d6e18fe2231470a752b4fce07897e` (2026-10-20, `PRIMJS_VERSION`
  2.11.1-rc.1).
- Servo `main` @ `99d10126b7a40a132ea26e0a59fc9304307ce763` (2026-10-25), `components/script_bindings`.
- svelters @ `1d6656c2` (local), cadatomic @ `95b3fdd` (local).
- Local context: `cadatomic/docs/research/notes/2026-10-25-own-gui-toolkit/js-runtimes.md`,
  `svelters/plans/research/QUICKJS-RUNTIME.md`, `svelters/docs/notes/js-native-serdes.md`,
  `svelters/docs/notes/egui-js-widgets.md`, `svelters/plans/research/REACTIVITY.md`.

---

## 0. Summary

- **The JS→native crossing in QuickJS-NG is already cheap at the C level.** `js_call_c_function`
  passes the interpreter's own stack slots as `argv` (no copy unless `argc < length`), switches on a
  `cproto` and calls the C function with an `int magic` [src]. A maintainer says C functions are
  cheaper to call than JS functions, because "quickjs has to perform a fair amount of bookkeeping on JS
  function entry" [doc, discussion #225]. The measured ~0.6–0.8 µs per call (loaded machine) is mostly
  rquickjs's generic machinery, plus the interpreter evaluating arguments and looking up names
  [inferred from src §2]. And per state change, Svelte's effect machinery costs ~5.5–7 µs batched
  (~170–200 µs unbatched) [local, REACTIVITY.md], dwarfing either.
- **(a) No fork** (raw `JSCFunctionMagic` from rquickjs-sys, prop id = `magic`, scalars read from the
  JSValue tag, strings as atoms or borrowed): an estimated **40–80 ns per op unloaded** for the
  crossing, against an estimated ~250–450 ns today [inferred]. This is most of what is available.
- **(b) Light fork** (native slots, a call path without the frame/interrupt bookkeeping): an estimated
  extra 5–20 ns per op [inferred], because QuickJS has no argument array to bypass. Not worth a fork.
- **(c) Deep fork** (nodes as a custom JSValue tag, or exotic objects whose props are Rust columns):
  **the exotic-object half needs no fork.** `JSClassExoticMethods` (`get_property`/`set_property`
  receive the `JSAtom`) is public API, and `node.label = v` reaches the hook after two own-property
  misses (estimated 50–100 ns) [src]. A custom value tag is what PrimJS did (`LEPUS_TAG_LEPUS_REF`,
  37 sites in its `quickjs.cc`) [src]. It saves a JSObject per node but makes QuickJS-NG
  un-rebasable. NG lands ~30 commits a month and a release every 1–2 months, with header API changes
  in most releases [src: git log].
- **Recommendation: (a), no fork,** in rquickjs-sys raw ops behind svelters-host, with
  compiler-emitted typed setters (`set_f64(node, PROP_X, v)` as imported bindings). Measure it first
  against today's path in one A/B microbench. Move `$n.set`'s JS-side dedupe (`node.last` Maps,
  `plainData`) into Rust columns. Offer the exotic `node.prop` surface (no fork) for user code and
  `bind:this`. A fork is justified only when a release blocks the work in hand (cadatomic `CLAUDE.md`,
  "Other repositories"; svelters `QUICKJS-RUNTIME.md` §1), and nothing here is blocked.

---

## 1. QuickJS-NG internals that matter for zero-conversion bindings

### 1.1 JSValue representation

- **64-bit hosts: a 16-byte struct, not NaN-boxing.** `typedef struct JSValue { JSValueUnion u;
  int64_t tag; }`, where the union holds `uint64`/`float64`/`ptr`/`short_big_int`. NaN-boxing
  (`typedef uint64_t JSValue`) is on only when `INTPTR_MAX < INT64_MAX`, i.e. 32-bit builds
  [src `quickjs.h:172-176, 315-339`].
- **Tags** [src `quickjs.h:179-199`]. Negative tags carry a refcount:
  - `BIG_INT -9`, `SYMBOL -8`, `STRING -7`, `STRING_ROPE -6`, `MODULE -3`,
    `FUNCTION_BYTECODE -2`, `OBJECT -1`.
  - Then `INT 0`, `BOOL 1`, `NULL 2`, `UNDEFINED 3`, `UNINITIALIZED 4`, `CATCH_OFFSET 5`,
    `EXCEPTION 6`, `SHORT_BIG_INT 7`, `FLOAT64 8`.
- **Consequence [inferred].** A Rust host function reads an int, float or bool argument with one
  compare on `tag` plus one load. No call is needed: rquickjs-sys already has these as inline Rust
  (`sys/src/inlines/ptr_64.rs`, `common.rs`) [src]. A JS number may arrive as `INT` or `FLOAT64`,
  so both must be accepted.
- **Dup and free are out-of-line in NG.** `JS_FreeValue`, `JS_DupValue`, `JS_DupContext` and
  `JS_FreeContext` are `JS_EXTERN` functions [src `quickjs.h:868-873`]. rquickjs-sys binds them as
  `extern "C"` [src `sys/src/bindings/x86_64-unknown-linux-gnu.rs:308-311, 884-890`]. So every dup
  and free from Rust is a real call unless cross-language LTO is used [inferred].
- **Object header.** `struct JSObject` [src `quickjs.c:1125-1179`] holds:
  - the GC link (the comment says "ref_count/gc_obj_type/mark live in the allocator block header");
  - flag bits, including `is_exotic` and `fast_array`;
  - `uint16_t class_id`;
  - `JSShape *shape` and `JSProperty *prop`;
  - `first_weak_ref`;
  - one union, whose user-class member is **a single `void *opaque`**.

  For C functions the union holds `{realm, c_function, uint8 length, uint8 cproto, int16 magic}`.
  So `magic` is a signed 16-bit value: up to 32767 ids per function.

### 1.2 Classes, opaque data, exotic hooks

- **`JSClassDef`** = `{class_name, finalizer, gc_mark, call, exotic}` [src `quickjs.h:684-697`].
- **`JSClassExoticMethods`** [src `quickjs.h:647-674`]:
  - `get_own_property(ctx, desc, obj, JSAtom)`, `get_own_property_names`, `delete_property`,
    `define_own_property(..., getter, setter, flags)`;
  - the optional fast trio `has_property`, `get_property(ctx, obj, JSAtom, receiver)` and
    `set_property(ctx, obj, JSAtom, value, receiver, flags)`.
  - All of these are public, and they receive the interned `JSAtom`, never a string.
- **`JS_SetOpaque/JS_GetOpaque`** [src `quickjs.c` near `JS_GetOpaque`]:
  - `GetOpaque` is a tag check, a `class_id` compare and a load, but it is an exported, non-inline
    function, so a few ns from Rust [inferred].
  - `SetOpaque` works only for classes `>= JS_CLASS_INIT_COUNT`, i.e. user classes.
  - The opaque is a full pointer-sized word. It can hold a **`u64` node id/generation directly**,
    with no Box allocation [inferred].
- **C functions** [src `quickjs.h:467-470, 1303-1368`]:
  - `JSCFunction(ctx, this, argc, argv)`;
  - `JSCFunctionMagic(..., int magic)`;
  - `JSCFunctionData(..., magic, func_data)`;
  - new in NG, `JSCClosure(..., magic, void *opaque)` via `JS_NewCClosure(ctx, func, name,
    opaque_finalize, length, magic, opaque)`. This is a Rust closure pointer without rquickjs's class
    machinery.
  - `JSCFunctionEnum` is closed: `generic`, `generic_magic`, constructors, `f_f` and `f_f_f` (a
    double→double fast path for Math), `getter`, `setter`, `getter_magic`, `setter_magic`,
    `iterator_next`.
- **Accessors on a prototype.** `JSCFunctionListEntry` with `JS_CGETSET_MAGIC_DEF(name, get, set,
  magic)` plus `JS_SetPropertyFunctionList` install C getters and setters, one C function per class
  with the prop id as `magic` [src `quickjs.h:1375-1454`].
- **The C call path** [src `js_call_c_function`]:
  1. stack-overflow check;
  2. push a `JSStackFrame` (so stack traces include the native frame);
  3. switch realm;
  4. if `argc < length`, `alloca` and pad with `undefined`; otherwise `argv` is the interpreter's
     stack;
  5. switch on `cproto`, then call.

  `JS_CallInternal` first polls interrupts, then dispatches by `class_id`. `OP_call` and
  `OP_call_method` free the callee and its arguments afterwards [src `quickjs.c:18542-18593`].

### 1.3 Atoms

- A `JSAtom` is a `uint32_t` index into `rt->atom_array`. Small integers are tagged inline
  [src `quickjs.c:3432-3447`, `__JS_NewAtom`].
- `JS_NewAtomLen` looks up an existing atom first (`__JS_FindAtom`) [src `quickjs.c:3712-3727`].
- **String literals in bytecode are atoms.** `OP_push_atom_value` does `JS_AtomToValue`, which
  returns the atom's own `JSString` (refcount bumped) [src `quickjs.c:18279`, `3861-3891`].
- `JS_ValueToAtom` on such a string takes the "str is the atom" path. `js_get_atom_index` then walks
  one hash chain by pointer compare and never hashes the content [src `__JS_NewAtom`,
  `js_get_atom_index`].
- **Consequence [inferred].** A host op can map a literal string to a Rust interned id through
  `JS_ValueToAtom` plus a `Vec<u32>` indexed by atom, in roughly 5–15 ns, with no UTF-8 work.
- Property names are atoms, so an exotic `set_property(atom)` can map atom → prop id through the same
  table. Pre-register the prop names at startup with `JS_NewAtom` and keep them alive.

### 1.4 Shapes and inline caches

- **QuickJS-NG has no inline caches today.**
  - Polymorphic ICs were added in #120 (`6b78c7f`, Divy Srivastava, 2023-11-29) and reworked in
    #609.
  - They were **removed in #884** (`7de6d46`, Ben Noordhuis, 2025-02-06): "benchmarks show it's
    currently a mixed bag: sometimes faster, sometimes slower, always more memory hungry …
    remove the current implementation and start afresh" [src: git log].
  - `docs/docs/diff.md` still lists "Polymorphic inline caching" [src `diff.md:44`]; that page is
    stale.
- **Property get** (`OP_get_field`) [src `quickjs.c:19559-19603`]:
  1. `find_own_property` hashes the atom with the shape's `prop_hash_mask` and walks the chain
     [src `quickjs.c:6836-6858`];
  2. on a hit with a plain value, the value is dup'd and the lookup is done;
  3. an accessor (`JS_PROP_TMASK`) or an **exotic object** takes the slow path,
     `JS_GetPropertyInternal`;
  4. otherwise the lookup walks `shape->proto`.
- **Property set** (`OP_put_field`) [src `quickjs.c:19655-19692`]: the fast path is an own, writable,
  plain property only. Anything else goes to `JS_SetPropertyInternal2`.
- **Accessor cost.** A prototype accessor (`get label()`) costs, per access:
  - a hash miss on the instance;
  - a hit on the prototype;
  - the slow-path re-walk;
  - `JS_CallFree(getter)`, i.e. a full `JS_CallInternal` + `js_call_c_function`
    [src `JS_GetPropertyInternal`].

  Without ICs, `node.label = v` through an accessor is a lookup *and* a call [inferred].
- **Exotic hooks** [src `JS_GetPropertyInternal`, exotic branch; `JS_SetPropertyInternal2` lines
  ~73-121 from its start]. For an exotic class, after the own-property miss the engine calls
  `em->get_property(ctx, obj, atom, receiver)` or `em->set_property(...)` directly. No JS function
  object is involved.
- **PrimJS** has no inline caches either: grep for IC symbols in `src/` finds nothing [src]. Its gains
  come from the template interpreter and the GC (§3.6).

### 1.5 Strings

- **`JSString`** [src `quickjs.c:696-723`]:
  - `len:31`, `is_wide_char:1` (8-bit Latin-1 or 16-bit UTF-16), `hash:28`, `kind:2`, `atom_type:2`;
  - kinds `NORMAL` (inline data), `SLICE` (a substring view of a parent, up to
    `JS_STRING_SLICE_LEN_MAX = 1024` bytes) and `INDIRECT` (used internally for regexp bytecode).
- **External strings.** No public API creates them; `INDIRECT` memory is freed with `js_free_rt`
  [src `js_free_string0`].
- **Ropes exist** (`JS_TAG_STRING_ROPE`, `{len, is_wide_char, depth, left, right}`):
  - concatenation uses a rope when the right operand exceeds `JS_STRING_ROPE_SHORT_LEN = 512`, or
    the left exceeds `JS_STRING_ROPE_SHORT2_LEN = 8192` [src `quickjs.c:225-233, 5505-5525`];
  - converting a rope linearizes it, which allocates (`js_linearize_string_rope`)
    [src `quickjs.c:14615`].
- **`JS_ToCStringLen2`** [src]:
  - 8-bit strings: a pass counts non-ASCII bytes; **if none and the kind is `NORMAL`, it returns a
    pointer into the JSString (zero-copy, NUL-terminated)**;
  - Latin-1 is transcoded into a new allocation;
  - wide strings are transcoded into a `len*3` buffer;
  - a slice is always copied.
- **`JS_ToCStringLenUTF16`** returns wide strings without copying [src `quickjs.h:918`; local
  js-native-serdes.md §4.2: 0.6 µs/KiB vs 8.6–12 µs/KiB via UTF-8].
- **Creating strings:** `JS_NewStringLen` (UTF-8 in) and `JS_NewStringUTF16`.
- **Local numbers** [local js-native-serdes.md §4.2, loaded machine], per op:

  | String | Borrowed UTF-8 | Through rquickjs `String` |
  |---|---|---|
  | 11 B ASCII | 686 ns | 801 ns |
  | 1 KiB Latin-1 | 5.6 µs | 10.5 µs |
  | 1 KiB wide | 8.6 µs | 21.9 µs |

### 1.6 ArrayBuffers and typed arrays

- **At v0.17.0 and HEAD, `JS_NewArrayBuffer(ctx, buf, len, max_len, realloc_func, opaque,
  is_shared)`** takes a `JSReallocArrayBufferDataFunc`:
  - "`realloc_func` may be NULL if the memory must not be managed by quickjs at all; such an
    ArrayBuffer can neither be resized nor transferred" [src `quickjs.h:1081-1098`];
  - this replaced the older free-func signature, which rquickjs 0.12+ wraps as
    `ArrayBuffer::from_source`.
- **Related calls:**
  - `JS_SetImmutableArrayBuffer`, `JS_GetArrayBuffer`, `JS_GetUint8Array`;
  - `JS_NewUint8Array(buf, len, free…)`, `JS_NewTypedArray`.
- **Consequence [inferred].** Rust-owned columns (positions, colours) *can* be exposed to JS as
  zero-copy typed arrays over Rust memory. But typed-array element stores are interpreted
  `put_array_el` ops, and the local measurement had batching through a `Float32Array` *slower* than
  one call per op (1.2–1.4 µs vs 1.0–1.1 µs) [local egui-js-widgets.md]. So this suits bulk data,
  not per-prop updates.
- **Detach hazard.** QuickJS cannot forbid detaching a JS-allocated buffer [local
  js-native-serdes.md §2.2].

### 1.7 GC

- **Algorithm.** Refcounting plus a synchronous cycle collector: `JS_RunGC` = `gc_decref` →
  `gc_scan` → `gc_free_cycles` [src `quickjs.c:7373-7494`].
  - `gc_decref` walks every GC object and calls `mark_children`, which for user classes calls the
    class's `gc_mark`.
  - It decrements each child's refcount. **An object whose refcount stays > 0 after that is held
    from outside the heap**, i.e. a root.
  - No root registration exists or is needed.
- **Trigger.** Allocation once `malloc_size > malloc_gc_threshold` (initial 256 KiB) [src
  `quickjs.c:2333`]. A new heuristic was tweaked in #1806 (after v0.17.0) [src git log].
- **Finalizers and zombies.** `JS_IsLiveObject` exists because "zombie objects are visible in
  finalizers when freeing cycles" [src `quickjs.c:7496-7506`].

### 1.8 What an embedder cannot do without patching QuickJS-NG [src + inferred]

1. **More than one native word per object.** There is one `opaque`. Workarounds: store an id (no
   allocation), or keep extra JS values in ordinary properties under private symbols
   (`JS_NewPrivateSymbol`, `quickjs.h:1166`).
2. **Typed fast-call signatures.** `JSCFunctionEnum` and its switch in `js_call_c_function` are
   closed. Adding `cproto`s such as `(node, i32)` or `(node, f64)` that skip `argv` needs a patch.
3. **Skipping per-call bookkeeping** (interrupt poll, stack-overflow check, `JSStackFrame` push).
   These are what give native frames stack traces and exceptions.
4. **Custom value tags** (a JSValue that *is* a node id, with no JSObject). Every tag switch in
   `quickjs.c` must learn it: free, dup, mark, typeof, compare, ToString, get/put field, bytecode
   writer.
5. **Inline caches or prototype-accessor fast paths.** These were removed upstream (§1.4).
6. **External, non-copied strings** (Rust-owned text visible as a JS string).
7. **Custom opcodes** that the compiler could emit (e.g. `set_prop_id`). The parser and bytecode
   format are internal.
8. **Pinning a JS-allocated ArrayBuffer** against detach.

---

## 2. rquickjs 0.14 overhead, per call and per value

Read at `c0bef90` [src]:

- **Functions are class objects, not C functions.** `Function::new(ctx, closure)` creates a
  `RustFunction(Box<dyn RustFunc>)` instance of rquickjs's single "callable" class. QuickJS calls it
  through `JSClassDef.call`:
  1. `ffi::call` [src `core/src/class/ffi.rs:66-81`] does `JS_GetRuntime`, reads the callable
     class id from the runtime opaque, calls `JS_GetOpaque`, then makes a vtable call;
  2. `call_impl` [src `ffi.rs:261-279`] builds `Params::from_ffi_class`, which runs
     `Ctx::from_ptr` → `JS_DupContext` [src `params.rs:21-62`, `context/ctx.rs:110-117`];
  3. `ctx.clone()` (another `JS_DupContext`);
  4. `handle_panic` (`std::panic::catch_unwind`; a landing pad, near-free when nothing panics
     [src `result.rs:672-698`, `util.rs:154-158`]);
  5. a `JsCell` borrow;
  6. a `Box<dyn>` call into the `IntoJsFunc` adapter;
  7. per argument, `FromParam` → `Value::from_js_value_const` = `JS_DupValue` + `JS_DupContext`,
     then `FromJs`, then `Drop` = `JS_FreeValue` + `JS_FreeContext` [src `core/src/value.rs:79-157`];
  8. the result `IntoJs` → `into_js_value` (`JS_FreeContext`).
  - Unaligned `argv` is copied only on 32-bit MSVC [src `params.rs:37-50`].
- **Per-call total [inferred].** For a 3-argument call that is roughly 15–25 out-of-line C calls
  plus dyn dispatch and `Result` plumbing. This is an estimate from the code path; no rquickjs
  benchmark was found (searched; none published).
- **`Ctx`/`Context::with`.** The runtime lock is a `RefCell` borrow without the `parallel` feature, a
  mutex with it, plus `JS_UpdateStackTop` per entry [local js-runtimes.md §6.2]. That cost is paid
  per Rust→JS entry, not per JS→Rust call.
- **`Class<T>` (`#[rquickjs::class]`)** [src `core/src/class.rs:223-253`]. Each instance:
  - does `JS_NewObjectProtoClass`;
  - **allocates a `Box<ClassCell<JsCell<T>>>`** and sets it as the opaque;
  - looks up its prototype.
- **`#[qjs(get, set)]` accessors** become `Accessor` → `JS_DefineProperty` with getter and setter
  **`Function::new` objects (RustFunction instances)**, not `JS_CFUNC_getter_magic`
  [src `core/src/value/object/property.rs:211-251`]. So `obj.prop` pays the accessor slow path
  (§1.4) *and* the full rquickjs call machinery.
- **Exotic classes in rquickjs** wrap the atom and receiver in owned `Atom`/`Value` (dup + context
  dup) before calling the Rust method [src `ffi.rs:281-298`].
- **`Persistent<T>`** is `{rt, value}`, a strong ref with a runtime check on `restore`
  [src `core/src/persistent.rs:8-112`]. To the cycle collector a Persistent is an external root
  (§1.7).
- **Using rquickjs-sys directly is supported.** Available: `Ctx::as_raw`/`from_raw`, `JS_NewClassID`,
  `JS_NewClass`, `JS_NewCFunction2` with magic, `JS_NewCClosure`, `JS_SetPropertyFunctionList`, and
  the inline tag accessors. So the whole host-op layer can bypass `Function::new` without forking
  either crate [inferred from src]. rquickjs allocates its own class ids through `JS_NewClassID`, so
  ours do not collide.
- **Measured today** [local js-native-serdes.md §4.2, i7-8850H, load average 27–31; "absolute
  numbers … inflated, probably 2–3×"]:

  | Op | ns/op |
  |---|---|
  | JS loop alone | 177 |
  | rquickjs no-op 3-arg call | 837 |
  | `setPropId(h, id, i)` | 768 |
  | `setProp(h, 'spacing', i)` with a name | 1433 |

  egui-js-widgets.md: a host call adds ≈0.55 µs on a 0.46–0.51 µs argument-evaluation floor.
  Unloaded, the rquickjs call is therefore roughly **200–330 ns over the loop** [inferred].
- **External reference points:**
  - JSC C++ API no-op 2 ns, N-API no-op 7 ns [3rd: Jarred Sumner, https://bun.com/blog/compile-and-run-c-in-js,
    2024-09-18];
  - bun:ffi 3 ns, napi.rs 15 ns, Deno 1.21 FFI 431 ns, all macOS aarch64
    [3rd: https://github.com/PeerRich/bun/blob/main/bench/ffi/plus100, a fork of oven-sh/bun].
  - These are JIT engines, so they bound what is possible, not what QuickJS does.

---

## 3. How big engines integrate DOM nodes with zero conversion

### 3.1 Blink + V8

- **Two halves.** "Each DOM object has two halves: a V8 wrapper that JavaScript uses and a C++ object
  in the DOM", referencing each other [doc: https://v8.dev/blog/tracing-js-dom, Degenbaev, Filippov,
  Lippautz, Payer, 2018-03-01].
- **Object grouping (Chrome ≤56)** kept whole document groups alive. It hid retaining paths,
  collected wrappers early (replacing them with empty wrappers) and handled event-listener closures
  badly.
- **Cross-component tracing (Chrome 57+)** traces "from JavaScript to the C++ implementation of the
  DOM and back", incrementally, with write barriers [doc, same post].
- **Oilpan/cppgc.** A tracing GC for C++, hosted in V8 since v9.4 and in Blink from M94. It gives a
  "unified JavaScript and C++ heap" in which "TracedReference models references into V8, and internal
  fields model references out of V8". It replaced Blink's reference counting, "which leaked on cycles
  and caused use-after-free bugs" [doc: https://v8.dev/blog/oilpan-library, 2021-11-10].
- **Pointer storage.** Wrappers store the C++ pointer as an embedder field. Current V8 has
  `Object::Wrap/Unwrap` with a `CppHeapPointerTag`, so the pointer goes through a sandbox pointer
  table with type-range checks [doc: `include/v8-sandbox.h`,
  https://chromium.googlesource.com/v8/v8.git/+/main/include/v8-sandbox.h; `gin/wrappable.h`].
- **V8 Fast API calls.** Optimized code calls a typed C function directly. The callback "can't
  allocate on the JS heap or trigger JS execution"; there is a slow fallback, and NaNs are passed
  as is [doc: https://chromium.googlesource.com/v8/v8.git/+/main/include/v8-fast-api-calls.h]. This
  is a JIT feature; no ns figure was found for Blink.
- **Lesson [inferred]:**
  - an IDL generates the bindings;
  - the wrapper holds a tagged pointer;
  - JS and C++ are traced together;
  - fast calls take scalar signatures that the JIT can see.

  In QuickJS the analogues are class `gc_mark` (tracing), opaque (embedder field) and magic C
  functions (generated bindings). There is no JIT to exploit typed signatures.

### 3.2 Servo (SpiderMonkey)

- **Ownership.** The JSObject reflector *owns* the Rust DOM object. `reflect_dom_object` "yields
  ownership of the object to the SpiderMonkey Garbage Collector", and the reflector's finalizer
  destroys the DOM object [doc: `components/script/docs/JS-Servos-only-GC.md`,
  https://third-party-mirror.googlesource.com/servo/+/refs/heads/main/components/script/docs/JS-Servos-only-GC.md].
- **Reserved slot.** The DOM pointer is kept in **reserved slot 0** (`DOM_OBJECT_SLOT = 0`,
  `JS_SetReservedSlot(..., PrivateValue(raw))`)
  [src `components/script_bindings/conversions.rs:243-268`, `wrap.rs:124`].
- **Tracing.** Rust fields of type `Dom<T>` trace the reflector through `JSClass.trace`.
- **JIT info.** Generated bindings emit a `JSJitInfo` per getter, setter and method: `opType`,
  `aliasSet`, `returnType`, `isInfallible`, `isMovable`, `isEliminatable`, `isAlwaysInSlot`,
  `slotIndex`, `protoID`, `depth`. Ion can then inline, hoist or CSE DOM getters and read cached
  values straight from slots [src `components/script_bindings/codegen/codegen.py:5285-5330`].
- **Lesson [inferred].** "Slot-cached getters" are the JIT-world version of reading a column without
  a call. QuickJS has no JIT. The closest equivalent is storing hot read-only values as plain own
  properties, which `OP_get_field` resolves without a call (§1.4).

### 3.3 Bun / JavaScriptCore

- **bun:ffi.** "Roughly 2-6x faster than Node.js FFI through Node-API". JSC implements
  `dlopen`/`CFunction` natively, and hot sites compile through DFG/FTL into direct native calls
  [doc: https://bun.com/docs/runtime/ffi].
- **DOMJIT.**
  - JSC took over the getter prologue's ClassInfo check ("Hoist DOM binding attribute getter
    prologue into JavaScriptCore taking advantage of DOMJIT / CheckSubClass"), and Node accessors
    were implemented in DOMJIT [doc: WebKit commits
    https://webkit.googlesource.com/WebKit/+/f496c6b2d65fd6257e8ef4a6da342ee0a1916136 ,
    https://webkit.googlesource.com/WebKit/+/bef79f8b144b5e357c9eedaefc7ad1adf71062fe].
  - Caching getter calls in the DFG was "a 40% speed-up in the Dromaeo DOM Traversal average" [3rd,
    via a mirrored WebKit commit message; not verified against WebKit bugzilla].
  - JIT-only. Not transferable to QuickJS.

### 3.4 Deno ops (deno_core `op2`)

- **History.** The JSON op ABI was replaced by serde_v8 in 2021 (denoland/deno#9540, AaronO).
  - Per-op, from ns per 100-iteration loop: void ≈ 65 ns, `add` 2.8 µs JSON → 0.19 µs serde_v8
    [3rd: https://github.com/denoland/deno/issues/9540, 2021-03-09].
  - Codegen ops (#13861, Divy, 2022) took `op_void_sync` from ~50 to ~28 ns/op
    [3rd: https://github.com/denoland/deno/pull/13861].
- **Today.** `op2` generates a V8 Fast API path for `#[op2(fast)]` ops: scalars, bools, typed-array
  slices and `&mut OpState`. "When V8 optimizes a slow function into a fastcall, it will switch the
  implementation over if the parameters are compatible" [doc: https://docs.rs/deno_core/latest/deno_core/attr.op2.html].
- **Deno FFI** claims "~1ns on M1 16GB RAM" [doc: https://docs.rs/crate/deno_ffi/0.241.0].
- **Lesson [inferred].** Deno's 20× came from dropping generic serialization (JSON, serde) for
  generated per-signature glue. That is exactly rquickjs `FromJs` → raw tag reads for us. Its last 10×
  came from the JIT.

### 3.5 React Native JSI, and Static Hermes / Hermes V1

- **Margelo, 2026-04-23 (Alex Shumihin)** [3rd: https://margelo.com/blog/make-jsi-run-faster;
  engine and device not stated]. Over 1M iterations:

  | Comparison | Per call |
  |---|---|
  | HostObject vs HostFunction | 181 ns vs 27 ns |
  | HostObject vs Object + NativeState | 188 ns vs 38 ns |

  - The `HostObject::get` cost is virtual dispatch, `PropNameID` → `std::string`, string compares,
    and often a new HostFunction per lookup.
  - Advice: HostObject only for "dynamic property interception"; otherwise plain objects with
    NativeState and function properties.
- **Lesson [inferred].** This is the exotic-get-property trap. A per-property hook with *string*
  names is slow. QuickJS's exotic hook gets an atom (an integer), which avoids the string half, but
  not the hook call.
- **QuickJS-NG issue #797.** Someone porting RN to QuickJS used a HostObject as `__proto__` of a
  plain object and cached values on it; the exotic hooks then intercepted the caching writes
  [doc: https://github.com/quickjs-ng/quickjs/issues/797, 2025-01].
- **Static Hermes** claimed native FFI call overhead "between 15x to 80x lower than JSI" [doc:
  https://speakerdeck.com/tmikov2023/static-hermes-react-native-eu-2023-announcement?slide=14] and
  "zero-cost calls to C functions" [doc: Chain React 2024 deck,
  https://speakerdeck.com/tmikov2023/optimizing-with-static-hermes-chain-react-2024?slide=6].
  - The new VM shipped as **Hermes V1**: opt-in in RN 0.82 (2025-10-08) and default in RN 0.84
    (2026-02-11) [doc: https://reactnative.dev/blog/2025/10/08/react-native-0.82 ,
    https://reactnative.dev/blog/2026/02/11/react-native-0.84].
  - No public number was found for typed-FFI overhead in Hermes V1.
  - The mechanism is AOT compilation of sound-typed JS, which is out of reach for an interpreter.

### 3.6 PrimJS (Lynx) vs QuickJS

- **Template interpreter.** Per-bytecode handlers in macro assembler, native stack, global register
  allocation, top-of-stack caching, dispatch table [doc `docs/template_interpreter.md` @ `60e1b41`].
- **Tracing GC.**
  - Parallel mark-sweep, replacing refcounting, with HandleScope/GCPersistent/GlobalHandle roots.
  - Targets: "performance is still better than RC by 10% -20%" and a pause under 30 ms for a
    50 MB heap on mid-range devices.
  - "Adjust GC trigger timing in combination with business … rendering and other key scenes, can
    suppress GC" [doc `docs/gc.md`].
  - Its own table, GC vs RC (Octane): Richards +30.5%, RayTrace +85.9%, Splay +113%, Typescript
    +43% [doc `docs/gc.md`].
- **Octane on an M1 Max vs QuickJS `6e2e68`** (Bellard 2021-era): e.g. RayTrace 1273 → 2751,
  Typescript 18158 → 22855, CodeLoad 18137 → 16992 [doc `docs/benchmark.md`].
- **Custom value tag.** PrimJS added `LEPUS_TAG_LEPUS_REF = -10` and `LEPUSLepusRef {header, tag,
  void *p, LEPUSValue lepus_val}`, with runtime callbacks `get_property(ctx, this, JSAtom, idx)`,
  `set_property`, `get_length`, `convert_to_object`, `free_value` and `ref_counted_obj_visitor`
  [src `src/interpreter/quickjs/include/quickjs.h:438, 725-760`]. The tag appears at 37 sites in
  the 53,444-line `quickjs.cc` [src].
  - This is the precedent for option (c): native values that JS sees without a JSObject. The cost
    is a hard fork. PrimJS is renamed throughout (`LEPUS*`), C++, based on 2021-era QuickJS, and does
    not track NG [src, inferred].
  - Interpreter-only score on v8-v7, Linux x64: PrimJS 760 vs QuickJS-NG 0.17 646 vs Bellard 1201
    [3rd: https://github.com/ahaoboy/js-engine-benchmark; local js-runtimes.md §6.1].
- **Lynx Element PAPI on PrimJS.**
  - Main-thread scripts get elements as JS values "without a full object model (no getters/setters,
    only passable as parameters)", and call about 45 global functions on them: `__CreateElement`,
    `__AppendElement`, `__SetAttribute`, `__SetInlineStyles`, `__AddEvent`, `__FlushElementTree`, …
    [doc: https://lynxjs.org/guide/scripting-runtime/main-thread-runtime.md ,
    https://lynxjs.org/api/engine/element-api/__createelement].
  - My reading [inferred, not verified in Lynx source] is that elements travel as opaque native refs
    (the LepusRef tag) and every operation is a free function. That is option (a)'s shape, not
    (c)'s.

### 3.7 Node-API, Boa, Nova

- **Node-API:** an engine-neutral C ABI with handle scopes and `napi_value` indirection. 7 ns per
  no-op under JSC (Bun), 15 ns via napi.rs [3rd, §2]. Its value is portability. A tessel host could
  expose an N-API-like neutral layer, but QuickJS-NG would pay an extra indirection.
- **Boa 0.22:** `Class` trait (`data_constructor`, `init` with `class.method`), native data deriving
  `Trace`/`Finalize`/`JsData`, `NativeFunction::from_fn_ptr`, recovery via
  `object.downcast_ref::<T>()` [doc: https://docs.rs/boa_engine/latest/boa_engine/class/index.html].
  It is pure Rust with a Rust GC, so cross-heap tracing would be native. It was historically slower
  than QuickJS (svelters QUICKJS-RUNTIME.md §5).
- **Nova 1.0** (crates.io): data-oriented, a `Value` enum with 32-bit handles into typed heap
  vectors, an `Agent` that resolves handles. The project says "interpreter performance is also a
  goal, but not yet a high priority" and reports ~80% of test262 [doc: https://docs.rs/nova_vm/latest/nova_vm/ ,
  https://trynova.dev mirror]. Its id-handle design matches a column store, but not as a production
  engine for Svelte today [inferred].

---

## 4. Lifetime and GC design for scene nodes exposed to JS

### 4.1 Options

| | Servo-style: wrapper owns node | Node owns strong wrapper | **Node is primary; wrapper holds an id; wrapper traces node-owned JS values** (recommended) |
|---|---|---|---|
| Who frees the node | JS GC finalizer | explicit destroy | explicit `destroy`/remove from the tree (Svelte always calls it) |
| Rust-created nodes (no JS) | impossible | fine | fine: wrappers are created lazily, as in Blink |
| Handler-closure cycles (closure captures the wrapper) | GC-visible if traced | **leak**: the Rust-held JSValue is an untraced root (§1.7) | collected after destroy, because the wrapper's `gc_mark` reports the handlers |
| Stale access after destroy | n/a | dangling | id + generation check → `StaleHandleError` (svelters already does this) |

### 4.2 The recommended scheme concretely [inferred, from §1.2, §1.7, local js-native-serdes.md §5.4]

1. **Wrappers.** A JS wrapper is an object of our own class whose `opaque` = `(node_index:u32,
   generation:u32)` packed into the pointer word. Creating one costs no Rust allocation. The scene
   keeps a `wrapper: Option<*JSObject>` per node for canonical identity (one wrapper per node).
2. **Handler storage.** `$n.on(node, sig, fn)` stores `fn` (dup'd) in a Rust handler table keyed by
   node. Those refcounts are *owned by the wrapper*: its `gc_mark` calls `JS_MarkValue` on each
   handler of its node.
3. **Liveness.** While a node is alive and has a wrapper, the scene holds one strong ref on the
   wrapper, so the wrapper is a root and its handlers stay alive. On `destroy`, the scene drops the
   handlers' refs and its wrapper ref, bumps the generation, and clears the opaque. A closure cycle
   (handler → closure → wrapper) then becomes ordinary garbage.
4. **Finalizers.** They run inside `JS_RunGC`, during any allocation and possibly mid-host-op. They
   only clear `node.wrapper` and push onto a `Cell`/lock-free queue. They never call JS, the scene's
   `RefCell`s or the renderer (the gjs bug class; local js-native-serdes.md §5.4 risk 1).
5. **Weak references from JS.** `WeakRef` and `FinalizationRegistry` stay available for user code,
   with unspecified timing [local QUICKJS-RUNTIME.md §4 #7].
6. **Rust-held JS values** (e.g. `bind:` getters) obey the same rule: owned by a wrapper and marked
   by it, or explicitly released at destroy. Never bare `Persistent`s, which root forever.

This is what Blink moved to (tracing across the boundary instead of grouping) and what svelters'
local notes already require ("callbacks tied to an owner handle, generation-tagged ids").

---

## 5. Zero-serdes options, with estimated cost per op

All absolute numbers here are **[inferred] estimates** for an unloaded x86-64 desktop, release
build. They come from the code paths read in §1–§2, scaled from the local loaded-machine
measurements (÷2–3) and bounded by the JIT-engine figures in §2. **Nothing in this table was
measured.** The first action under any option is the A/B microbench in §5.6.

### 5.0 Baseline (today, svelters-host)

- **The call shape.** `$n.set(node, prop, value)` runs JS first [src svelters
  `packages/native-runtime/src/index.js:617-646`]:
  - the `RESTORE` check, `alive(node)`, `plainData(value)` (a recursive `$state`-proxy copy);
  - `node.requested`/`node.last` `Map` lookups and sets;
  - a snippet check;
  - then `node.host.setProp(node.h, prop /* string */, value)` through `Function::new`.
- **Crossing:** ~0.8–1.4 µs per op (loaded) [local], ≈ 250–450 ns unloaded [inferred].
- **JS-side `set` work:** not measured. QuickJS `Map` get/set are C calls with hashing; an estimated
  several hundred ns to >1 µs per `set` (loaded) [inferred].
- **Whole update:** about 5.5–7 µs per state change batched, 170–200 µs unbatched [local,
  REACTIVITY.md §0].

### 5.1 (a) No fork: raw magic C functions via rquickjs-sys

- **Registration.** Register host ops with `JS_NewCFunction2(..., JS_CFUNC_generic_magic, magic =
  prop id)`, or one C function per *type* with the prop id as an argument. Use `JS_NewCClosure` where
  a Rust state pointer is needed.
- **Compiler output.** The compiler emits module imports: `import { set_f64, set_str, set_bool, … }
  from 'svelters:scene'`. A call is then `OP_get_var_ref` + args + `OP_call`, with no global or
  `$n.` property lookup [inferred from §1.2–1.4].
- **Typed specialization.** The compiler knows each prop's type from the kind metadata, so it emits
  `set_f64(node, P_WIDTH, v)`. Svelters' compiler lowers templates itself (`crates/svelters-compiler`).
- **Inside the Rust op:**
  1. `JS_VALUE_GET_TAG`: an inline compare;
  2. `node`: `JS_GetOpaque` → id/generation check against the scene columns;
  3. the value: ints and floats from the tag, no FromJs;
  4. strings: `JS_ValueToAtom` → interned-id table when the value is an atom (literals);
     `JS_ToCStringLen2` borrowed when ASCII; `JS_ToCStringLenUTF16` when wide;
  5. the dedupe (`last == v`) done in Rust against the column, replacing the JS `Map`s;
  6. the column write plus a dirty bit.
- **Exceptions.** Wrap the Rust body in `catch_unwind` only around code that can panic, or abort on
  panic in release.
- **Cost estimate:**
  - QuickJS intrinsic call (arguments pushed, `OP_call`, interrupt poll, frame push, cproto switch,
    argument frees): **~25–50 ns**;
  - Rust body: **~5–20 ns**;
  - **≈ 40–80 ns per op**, vs ≈ 250–450 ns today.
  - With the JS-side `set` work also moved into Rust, the saving per update is roughly 0.5–1.5 µs
    (loaded scale) of a ~5.5–7 µs batched update: **10–25%** [inferred].
- **Risks:**
  - `unsafe` FFI in one module (keep it in svelters-host's QuickJS adapter, behind the engine trait);
  - re-entrancy (getters and Proxies when reading objects; scalars avoid it);
  - rquickjs-sys bindings are per target and regenerated by rquickjs on each NG bump (no work for us);
  - errors must go through `JS_Throw*`.
- **Maintenance:** none beyond today's version bumps.

### 5.2 (b) Light fork: native-slot objects and a fast-call path

- **What it could add:**
  - a `JSObject` variant with N inline native words, which saves one `JS_GetOpaque` (~2 ns);
  - typed `cproto`s (`node_i32`, `node_f64`, `node_atom`) that pass unboxed values;
  - a "leaf native" flag that skips the interrupt poll, the stack-overflow check and the
    `JSStackFrame` push.
- **Gain estimate:** ~5–20 ns per op over (a), i.e. **~30–60 ns per op**. QuickJS has no argument
  array to bypass: `argv` already points into the interpreter stack (§1.2).
- **Losses:**
  - native frames vanish from stack traces;
  - the interrupt watchdog loses one polling site;
  - we carry a C patch.
- **Maintenance:**
  - a `quickjs.c` patch near hot, frequently edited code (`JSObject`, `js_call_c_function`,
    `JS_CallInternal`);
  - rebased on each NG release (~every 1–2 months, §5.5) and on each rquickjs release, which pins an
    NG commit (7 releases in 12 months: 0.10 2025-10-24 … 0.14 2026-09-17 [src `CHANGELOG.md`]);
  - a fork of rquickjs-sys's submodule pointer (`[patch.crates-io]`, as QUICKJS-RUNTIME.md §1
    describes for the debugger patch);
  - our own `extern "C"` declarations.
  - Estimate ~0.5–2 days per bump [inferred].
- **Verdict:** the patch's cost is fixed and its gain is ~10–20% of a crossing that is itself ~10% of
  an update. Not justified.

### 5.3 (c) Nodes as JS objects whose properties are Rust columns

**(c1) Exotic class, no fork.**

- **Mechanism.** A node wrapper is an object of our class with `JSClassExoticMethods{get_property,
  set_property, has_property, get_own_property_names, define_own_property=NULL}`.
  - `node.label = v` → `OP_put_field`: an own-property miss, then `JS_SetPropertyInternal2`, a
    second miss, then the exotic check and `em->set_property(ctx, obj, atom, v, receiver)`.
  - Our hook maps atom → prop id from a table (pre-registered atoms) and writes the column.
  - Unknown atoms fall back to ordinary storage (`JS_DefinePropertyValue`). Methods resolve through
    the prototype from inside `get_property`.
- **Cost estimate:** ~50–100 ns per access (two hash misses + hook + table) [inferred from §1.4].
  This is about the same as (a), with no call frame.
- **Rule for writes:** only the compiler's own writes take the direct-call path; user code
  (`bind:this` refs, `node.value`) gets the natural object surface.
- **Pitfalls:**
  - every property access on the wrapper hits the hook, methods and `then` included;
  - `Object.keys` and spreads need `get_own_property_names`;
  - Svelte or runtime code that stores expandos on nodes must keep working: undefined
    `define_own_property` falls back to ordinary own properties, which then shadow the hook
    (§1.4 order);
  - QuickJS-NG issue #797 shows how hooks and caches interact (§3.5);
  - rquickjs's own exotic support adds per-call `Atom`/`Value` dups (§2), so register the class
    through rquickjs-sys.

**(c2) Deep fork, custom value tag (the PrimJS route).**

- **Mechanism.** A node is a JSValue `{tag = JS_TAG_NODE, u = id|gen}`: no JSObject, no GC object,
  no refcount. `OP_get_field`/`OP_put_field` branch on the tag into Rust.
- **Gain:** ~30–60 ns per access, plus no wrapper allocation per node (≈ one `JS_NewObjectClass`,
  ~50–150 ns, once per node creation, not per update) [inferred].
- **Cost:**
  - every tag switch in the 65k-line `quickjs.c` (PrimJS: 37 sites in its fork);
  - `typeof`, `===`, `instanceof` and `WeakMap` keys need defined semantics;
  - prototype methods need a synthetic prototype;
  - the bytecode writer must refuse the tag;
  - fuzzing and test262 coverage are ours to maintain.
- **Effectively a permanent fork** (PrimJS is the end state: diverged and renamed). Maintenance is
  the highest of all options.

### 5.4 How Svelte's call shape interacts

- **We own the shape.** svelters compiles the templates, so the compiler chooses between:
  1. `set_f64(node, P_X, v)` (typed and imported): cheapest in QuickJS, engine-portable (the same
     JS runs under the Node TestHost), and works with Svelte's template effects unchanged;
  2. `$n.set(node, 'x', v)`: today's shape. A `$n.` property lookup plus a string name, which costs
     1.43 vs 0.77 µs/op [local].
  3. `node.x = v` with exotic objects: the nicest output to read. Under QuickJS it has no edge over
     (1), and under V8 (TestHost) exotic objects become Proxies or host objects, the slow path there
     (Margelo HostObject numbers, §3.5).
- **Recommendation:** (1) for compiled template writes, (3) only as a user-facing API on `bind:this`
  values.
- **Where the time goes per update is Svelte, not the crossing.** Template effect scheduling,
  dependency tracking and `Map`-based dedupe cost ~5–7 µs per batched change [local]. Pushing
  dedupe into Rust columns ("last value" is a column) and batching invalidations (REACTIVITY.md: 30×)
  are larger wins than any fork.

### 5.5 Fork maintenance: upstream churn (measured from git at `7a23d8e`)

- **Commits:** 367 on master in the last 12 months (since 2025-10-25), 183 in the last 6, 102 in
  the last 3 (~30 a month). 221 of the 12-month commits touch `quickjs.c`. Most frequent authors on
  `quickjs.c`: Saúl Ibarra Corretgé 91, Ben Noordhuis 56, Divy Srivastava 18.
- **Releases** (10 in 12 months): v0.11.0 2025-10-28, v0.12.0 2026-02-10, v0.12.1 02-12, v0.13.0
  03-21, v0.14.0 04-11, v0.15.0 05-21, v0.15.1 06-04, v0.16.0 07-31, v0.16.1 08-04, v0.16.2 08-20,
  v0.17.0 09-18. NG's docs say "an average of a new release every 2 months" [doc `diff.md`].
- **`quickjs.c` + `quickjs.h` diff per minor release:** 0.11→0.12 +2673/−808; 0.12.1→0.13 +1218/−780;
  0.13→0.14 +619/−174; 0.14→0.15 +2726/−192; 0.15.1→0.16 +2183/−735; 0.16.2→0.17 +243/−153.
  0.11→0.17 total: +9522/−2649.
- **Public header** (`quickjs.h`) changes in every release, e.g.:
  - +50/−10 in 0.16.0;
  - after 0.17.0: "Pass JSContext to JSInterruptHandler (#1802)";
  - `JS_NewArrayBuffer` moved from a free func to a realloc func;
  - object refcount and GC type moved into the allocator block header.
- **Reading [inferred].** A patch to `JSObject` layout, the call path or the tag set conflicts in most
  releases. The Static Hermes story is the cautionary tale: two years on a separate branch meant "many
  performance gains never reached users" (Nicola Corti, as summarized in
  https://www.callstack.com/podcasts/from-static-hermes-to-hermes-v1-the-road-to-default).

### 5.6 The measurement that decides it (first step for any option; ~1 day; not run)

1. **One rquickjs-sys bench, three variants,** in a single loop calling `setX(node, 7, i)`:
   rquickjs `Function::new` (today), raw `JS_NewCFunction2` magic, and an exotic class's
   `node.x = i`. Add strings (literal/atom, 11 B ASCII, 1 KiB wide) and a `$n.set`-equivalent JS
   wrapper with `Map` dedupe.
2. **Quiet machine,** per cadatomic's timing rule. Report ns/op minus the empty loop.
3. **Decision rule:** a fork enters the discussion only if (a)'s crossing exceeds ~30% of a
   realistic batched update after the JS-side dedupe moves to Rust.

---

## 6. Strings and text

- **The mismatch.**
  - JS strings are 8-bit Latin-1 or 16-bit UTF-16, sometimes slices or ropes (§1.5).
  - tessel's text stack (shaping and layout over UTF-8, `&str`) needs UTF-8 at layout time
    [inferred].
- **Conversion costs** [local js-native-serdes.md §4.2, loaded]:
  - ASCII: borrowed, zero-copy, and only an O(n) scan;
  - Latin-1: ~5.6 µs/KiB;
  - wide: ~8.6–12 µs/KiB to UTF-8, ~0.6 µs/KiB to UTF-16.
  - Labels are short, so a few tens of ns each [inferred].
- **Options, in order:**
  1. **Compile-time interning of static strings.** The compiler hoists literal labels, classes and
     enum names into a string table registered once at module load (`register_strings([...])`), and
     emits `set_text_id(node, P_LABEL, 17)`. No string crosses per update. Valdi does the same
     (`StringBox`, interned attribute names; local js-runtimes.md §4).
  2. **Atoms for literal strings at runtime.** `JS_ValueToAtom` on a literal string returns its
     atom without hashing (§1.3). Keep `Vec<Option<TextId>>` indexed by atom.
  3. **Dynamic short strings.** Borrow `JS_ToCStringLen2` (zero-copy ASCII); transcode non-ASCII
     once, at set time, into the node's text column (an `Arc<str>` or a shaped-run cache key). Never
     convert per frame. Dedupe against the column first; Svelte re-sets unchanged values.
  4. **Large text** (editors, logs, the `quire` code editor). Never as JS strings. A Rust-owned rope
     or buffer is exposed as a handle object with methods (`slice(a,b)`, `insert(pos, s)`, `length`,
     change events carrying offsets). JS gets substrings on demand. This follows the "boxed handle"
     pattern (local js-native-serdes.md §2.2) and the GtkTextIter-as-offset rule.
  5. **Outbound strings** (Rust → JS, e.g. an input's value): `JS_NewStringLen` from UTF-8 copies
     and transcodes. Cache the last JSValue per node prop (marked by the wrapper) so repeated reads
     return the same string, and use `JS_NewStringUTF16` when the source is UTF-16.
- **No external-string API exists** in QuickJS-NG (§1.5). Zero-copy Rust text visible as a JS string
  would need a fork (an `INDIRECT` kind with a free callback). Options 1–4 make that unnecessary
  [inferred].

---

## 7. Recommendation

1. **Option (a), no fork.**
   - Replace rquickjs `Function::new` host ops on the hot path (`create`, `set_*`, `insert`,
     `remove`, `on`) with raw rquickjs-sys `JS_NewCFunction2`/`JS_NewCClosure` magic functions, in
     the QuickJS adapter of svelters-host. Keep rquickjs for everything else: modules, loader,
     promises.
   - The compiler emits typed, imported setters with prop ids, and static strings as table ids.
2. **Move `$n.set`'s dedupe into Rust columns** ("last" values live in the scene), and keep
   REACTIVITY.md's batched invalidation. These dominate the per-update cost.
3. **Lifetimes:** §4.2. Id+generation opaques, a canonical lazy wrapper, handlers traced by the
   wrapper's `gc_mark`, enqueue-only finalizers.
4. **User-facing `node.prop`:** an exotic class through rquickjs-sys (no fork), when `bind:this` APIs
   want it.
5. **No QuickJS-NG or rquickjs fork now.** (b) buys ~5–20 ns per op; (c2) buys one JSObject per node.
   Each costs a patch rebased against ~30 commits a month and a release every 1–2 months. Revisit
   only if the §5.6 bench shows the crossing still above ~30% of a batched update after steps 1–2.
   Forking is the user's proposal and a design choice, so the coordinator should bring this analysis
   to the user rather than decide it.

## 8. Open questions and gaps

- No published rquickjs or QuickJS per-call benchmark exists (searched). All ns estimates in §5 are
  inferred, and §5.6 settles them.
- Lynx's element representation in PrimJS (LepusRef or object) was not verified in Lynx source.
- No ns figure was found for V8 Fast API calls in Blink, or for Hermes V1 typed FFI.
- The "40% Dromaeo" DOMJIT figure comes from a mirrored commit message, not WebKit bugzilla.
