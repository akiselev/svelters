# svelters JS <-> Rust boundary map (for the tessel "fork rquickjs/QuickJS-NG, zero-conversion" question)

Repo: /home/user/svelters (master 1d6656c2). Read-only investigation, 2026-10-25. Nothing built or run.
All numbers below are quoted from repo docs; most were measured on heavily loaded machines (the docs say
"read ratios, not absolutes").

---------------------------------------------------------------------------------------------------
## 0. Recorded fork decision (where and why)

- svelters `CLAUDE.md:22-23`: "Decisions already made by the user (2026-10-21): - Stock QuickJS-NG; no rquickjs
  fork, no determinism work."
- `plans/research/CADATOMIC.md:55-57`: "**Decided (user, 2026-10-22): no rquickjs determinism fork.** svelters stays
  on stock QuickJS-NG, and no determinism work is planned. cadatomic's decision 9 (and PB5, the `rquickjs` row of its
  fork sets) conflicts with this".
- `plans/research/CADATOMIC.md:94` (conflict table): "the fork is dropped; svelters stays on stock QuickJS-NG
  (rquickjs 0.14.0) ... A technical note for that lane: there must be one rquickjs in the process. `rquickjs-sys`
  has no `links` key, so a second copy would link duplicate QuickJS C symbols."
- `plans/research/CADATOMIC.md:303` (pin table): "rquickjs / rquickjs-sys | 0.14.0 | not yet | stock QuickJS-NG, no
  fork (user, 2026-10-22); cadatomic's scripting host (PB5) shares the same crate".
- `plans/research/TRACKS.md:66` (settled input #1): "No rquickjs fork; stock QuickJS-NG; no determinism work |
  cadatomic decision 9, PB5 and the `rquickjs` fork set in `01-overview.md` §6 change." and `TRACKS.md:1025` (§5.6
  register updates owed by cadatomic: "Decision 9: drop the rquickjs determinism fork, PB5 'on the user's fork', and
  the `rquickjs` row of `01-overview.md` §6").
- `plans/research/ARCH-EMBED.md:132` "**C-1** Stock QuickJS-NG (rquickjs 0.14.0). No fork, no determinism work." and
  `:1039` (isolates: "stock QuickJS-NG; one in-process runtime now, `RootOwner::Isolate` reserved").
- `plans/research/ARCH-SECURITY.md:56` "**C-1** stock QuickJS-NG through rquickjs 0.14, no fork (svelters CLAUDE.md)."
- cadatomic side (/home/user/cadatomic): `docs/plan/00-decisions.md:146-152` decision 9 originally said
  "Determinism: fork rquickjs to replace `Math.random` and `Date.now` (user; forks are cheap ...)"; `00-decisions.md:
  454-455` now records "**No rquickjs fork.** This settles the conflict above with decision 9's determinism fork:
  cadatomic uses svelters' stock QuickJS-NG." (`00-decisions.md:64-70` has the general "forks are cheap, maintained
  by agents" policy; `docs/plan/05-experiments-and-research.md:161` R7 was the fork experiment.)

**What the vetoed fork was for:** determinism (replace `Math.random`/`Date.now`) for cadatomic scripting (PB5). It was
NOT about JS/scene integration. svelters solved seeded random without a fork: `crates/svelters-host/src/quickjs.rs:
42-61` (`SEEDED_RANDOM`, sfc32 installed over `Math.random` when `HostSched::seed` is set, AU1b) and a virtual clock
in the `now` native (`quickjs.rs:923-932`).

**Reasons recorded** (no single "why" paragraph exists next to the user decision; the reasons are in the research):
- `plans/research/QUICKJS-RUNTIME.md` §Summary 1 / §1: "**There is no fork to maintain.** `akiselev/rquickjs` (2026) has
  zero commits of its own ... now 283 commits behind ... Stay on crates.io. Fork only when we carry a C patch: the
  QuickJS-NG debugger PR #1421 is the one candidate. Even then, fork only the QuickJS submodule pointer and switch to
  it with `[patch.crates-io]`." The 2021 `rquickjs-old-fork` patches (public raw Ctx, eval_global, typedmap userdata,
  backtraces, reentrant mutex, koush/quickjs debugger) are all covered upstream except the debugger
  (`docs/notes/qjs-research.md:39-54`).
- QUICKJS-RUNTIME §1 Plan: keep `rquickjs = "0.14"`; bump procedure = host suites + turn-budget tests; if a C patch is
  needed make branch `svelters/v0.14.x` whose "only change is the `sys/quickjs` submodule pointer" + hand-declared
  `extern "C"` fns; "**Don't vendor.**"
- QUICKJS-RUNTIME §4 item 13: "`#[rquickjs::class]` bindings for widgets | yes | L | — | Widgets go through handles and
  the C ABI by design, so the engine stays swappable. Don't use it". §5: "Keep the `JsEngine` boundary honest."
- The JIT fork (quickjs-jit) rejected separately (§4 below): maintenance of patches against private QuickJS layouts.
- ARCH-SECURITY ISO1 (`:162-166`, `:519`): isolates send "a serializable op-batch protocol over a transport trait ...
  **provided the protocol never relies on shared memory** (no `Arc` crossing ...)". A zero-conversion, pointer-shared
  JS<->scene design conflicts with that planned process-isolate placement.
- Who may reverse it: CLAUDE.md:87-90 "Escalate to the user only ... reversing or contradicting a decision the user
  made". So a tessel fork plan must go to the user as a reversal of C-1.

---------------------------------------------------------------------------------------------------
## 1. Every JS<->Rust boundary today

### 1.1 Engine setup (`crates/svelters-host/src/quickjs.rs`)
- `Runtime::new()`, `set_memory_limit(cfg.memory_limit)`, `set_max_stack_size(cfg.max_stack_size)` (`:207-209`);
  defaults `memory_limit: 256 * 1024 * 1024`, `max_stack_size: 1024 * 1024`, `turn_budget: Some(10 s)`
  (`core.rs:100-102`).
- Interrupt handler (`:220-232`): an external `AtomicBool` flag plus a wall-clock deadline (`deadline_ns`, armed per
  entry by `arm()`/`disarm()` `:294-311`); after an aborted entry `__svelters_recover()` resets runtime state (FX6,
  `:313-338`).
- Host promise rejection tracker (`:238-248`). `Context::full(&rt)` (`:251`). No `set_gc_threshold`; GC only via
  `collect_garbage` -> `rt.run_gc()` (`:665-667`) plus QuickJS's own triggers.
- Re-entry: thread-local `CURRENT_CTX` raw `JSContext` pointer + `RunningCtx` guard (`:65-89`), because
  `Context::with` is not re-entrant; nested callbacks use `Ctx::from_raw` (`:133-144`).
- Every Rust->JS entry goes through `call_guarded` (`:340-355`): arm deadline, `ctx.with`, RunningCtx, disarm,
  OOM classification.

### 1.2 How host ops are registered
All host ops are plain rquickjs closures: `Function::new(ctx.clone(), move |ctx: Ctx<'js>, ...| -> Res<..>)` set as
properties of one plain `Object` published as `globalThis.__svelters_host` (`:984-1442`). **No rquickjs classes, no
`#[rquickjs::class]`, no JS_SetOpaque, no `Func`/`MutFn` wrappers.** Every closure captures `Weak<Core>` (upgraded per
call: `core_of` `:823-826`) and usually `Rc<QjsCallbacks>`. Arguments come through rquickjs `FromJs` for `f64`, `u32`,
`bool`, `String`, `Opt<..>`, or raw `Value<'js>` which the converter reads itself.

| op (JS name) | line | JS args (rquickjs types) | path |
|---|---|---|---|
| `create(kind)` | 990-1007 | `Value` (int id or string) | `as_int` -> `op_create_id`, else `with_name` -> `op_create(&str)`; returns `f64` handle |
| `kindId/propId/signalId/methodId/slotId(a, b?)` | 1008-1050 | `Value`, `Opt<Value>` | name -> id resolvers (cached in Core) |
| `propType(kind, prop)` | 1052-1061 | `u32,u32` | coarse type string |
| `insert(parent, child, before?, slot?)` | 1064-1090 | `f64,f64,Opt<Option<f64>>,Opt<Option<Value>>` | `handle_of` validation, slot by name/id |
| `remove(parent, child)` | 1092-1104 | `f64,f64` | |
| `rootFault/rootInfo/rootEmit` | 1105-1135 | `u32, String, ...` | EM1 root messages |
| `destroy(h)` | 1137-1144 | `f64` | |
| `setProp(h, name, value)` | 1147-1159 | `f64, Value, Value` | `conv.with_name(&name, |n| c.op_set_prop(&conv, h, n, &value))` |
| `getProp(h, name)` | 1162-1174 | `f64, Value` | `op_get_prop` -> decode |
| `call(h, method, args[])` | 1177-1192 | `f64, Value, Value(array)` | `op_call` |
| `construct(kind, props?)`, `callStatic(kind, m, args)` | 1195-1228 | | ABI 2.5 / app exports |
| `typeChain(h)` | 1230-1236 | `f64` -> `Vec<String>` | |
| `listen(h, signal, on, flags?)` | 1239-1252 | `f64, Value, bool, Opt<u32>` | `op_listen` |
| `persist(key, obj?)` | 1256-1288 | reads `obj.svlHandle` | E3 hot-reload persistence |
| `observe(h, prop, on)` | 1291-1303 | | ABI 2.6 |
| `link(store, field, node, prop)` / `unlink(id)` | 1306-1325 | `f64, String, f64, String` | CX3 native store<->prop links, no JS per change |
| `style(opcode, h, text)` | 1327-1344 | `u32, f64, Value(string|null)` | text copied into `String` |
| `bounds(h)` | 1346-1362 | -> JS array | |
| `setChildAttr/getChildAttr` | 1365-1394 | | ABI 2.2 |
| `dx(op,a,b)`, `assets(op,uri)` | 1397-1412 | Strings in, String out (DX/AS1) |
| `assetBytes(uri)` | 1414-1421 | -> `ArrayBuffer::new` (copy) |
| `alloc(TypedArrayName, n)` | 1425-1439 | E2: typed array over a Rust allocation |
| + `platform::install` | 1441 | `quickjs/platform.rs` |

Prelude natives (not user-reachable after prelude, `:886-982`): `log`, `schedule`, `cancel`, `frame_request`,
`now`, `cancelAsync`, `cancelOp`, `streamAck`, `releaseBoxed`, `queueRelease`.

`call_sync` and `request_frame` are **not** JS ops: they are backend<->host C-ABI members
(`crates/svelters-abi/include/svelters_backend.h:242` `int (*call_sync)(...)`, `:311` `void (*request_frame)(void*)`).
`call_sync` arrives in JS as `__svelters_dispatch_sync`; `request_frame` is asked by the prelude `frame_request`
native, and the due frame runs `__svelters_internal.frame()` (`quickjs.rs:568-574`).

### 1.3 Argument conversion path (JS -> backend), `setProp` in full
1. rquickjs marshals `h` to `f64` (FromJs), `name`/`value` stay `Value<'js>`.
2. `core_of` (Weak upgrade), `QjsConv::new(...)` (a struct with two RefCells, `quickjs/conv.rs:37-58`),
   `handle_of(f64)` checks integral, `>=1`, `< 2^53` (`quickjs.rs:828-834`).
3. `QjsConv::with_name` (`conv.rs:87-101`): an int is a pre-resolved id; a string is **borrowed** with
   `JS_ToCStringLen2` (zero-copy for 8-bit ASCII; guard frees it) and UTF-8-validated (`from_utf8`).
4. `Core::op_set_prop` (`core/ops.rs:398-460`) inside `timed()` (`core.rs:496-510`: `crash::note_op` pushes the op
   name into a thread-local ring, sets an exports-objects scope, optional timing):
   - `live_kind(h)`: `RefCell` borrow + `HashMap<svl_handle, svl_id>` (std SipHash) (`ops.rs:117-126`, `core.rs:312`);
   - `resolve_name` -> `resolve`: `names: HashMap<(u32, svl_id), HashMap<Box<str>, svl_id>>` (two hashes, the inner
     one SipHashes the prop-name bytes) (`ops.rs:69-94`, `core.rs:325`); miss -> `backend.resolve(...)` through the
     C ABI, then insert;
   - `member(Prop, k, p)`: `HashMap<(u32,svl_id,svl_id), Option<Rc<MemberDesc>>>` + `Rc` clone (`ops.rs:138-150`);
   - `convert::encode(r, v, slot, cx, &mut Arena)` -> a raw `svl_value` tree borrowing engine memory (§1.4);
   - `kind_of(h)` again (conversion may have run user JS that destroyed `h`) (`ops.rs:444-448`);
   - `backend.set_prop_raw(h, p, &raw)` -> C function pointer `(b.set_prop)(b.self_, h, prop, v)`
     (`svelters-abi/src/safe.rs:1091-1094`);
   - `finish_callbacks`, then `observed` HashMap check, `after_set` for ABI 2.6 echo suppression (`ops.rs:449-458`).
5. Rust backends (egui): C trampoline `t_set_prop` -> `ValueRef::from_raw` -> `Backend::set_prop_ref`
   (`safe.rs:1619-1631`). The default `set_prop_ref` **copies into an owned `Value`** (`safe.rs:1491-1495`: "The
   default copies."). Only `svelters-host/examples/setprop_bench.rs:45` and `tests/serdes.rs:125` override it:
   **svelters-egui does not**, so every egui prop write builds an owned `Value` (String alloc for strings).

So the JS->Rust value path is: JS `Value` -> (descriptor-driven) borrowed `svl_value` (24 B C struct) -> owned
`svelters_abi::Value` enum -> egui typed field. No serde, no JSON on the widget path.

### 1.4 `convert.rs` (engine-agnostic, `JsReader`/`JsWriter` traits, `crates/svelters-host/src/convert.rs`)
- Header `:1-11`: "Type-directed conversion between JS values and the ABI's `svl_value` ... Inbound (`encode`): the
  backend's slot descriptor drives validation and coercion, and the result is a raw `svl_value` tree whose strings
  and byte ranges are *borrowed* from the engine ... Outbound (`decode`): owned `Value`s become JS values".
- `encode_at` (`:368-484`): `r.kind(v)` (rquickjs `type_of`), asset-ref check for every object
  (`has_field(HANDLE_KEY)`), then by slot tag:
  - bool (`:507`, also attribute strings `""/"true"/"false"`), int (`:551-580`, range checks, `"8"` strings
    parsed via `str_of` = an owned `String` copy), float (`:582-600`), datetime via `__svelters_internal.dateMs`;
  - string (`:602-630`): borrowed `JS_ToCStringLen2` or `JS_ToCStringLenUTF16` when the backend has
    `SVL_CAP_UTF16_IN`;
  - **enum** (`:638-674`): a JS string -> `str_of` (allocates a `String`) -> `TypeInfo.by_name: HashMap<String,i64>`;
    a number must be a known member value;
  - flags (`:676-729`): string (space separated) / number / array of names;
  - **record** (`:731-765`): per descriptor field `has_field(name)` + `field(name)` (two property lookups by `&str`
    each, i.e. two atom creations), recursion, fields pushed into an Arena list;
  - list (`:767-796`), map (`:798-833`, keys copied into `Box<str>`), handle (`:422-444`: a number, or an object
    with `svlHandle`), bytes (`:445-464`, borrowed `JS_GetArrayBuffer`; E2 staging for `Buffer<T>`), boxed
    (`__svlBoxed` marker array), callback (`:486-505`, registers a persistent function), dynamic/no-descriptor
    (`:841-916`).
- `decode_at` (`:932-1022`): `Value::I64/F64` -> int if integral and `< 2^31` else float (`conv.rs:370-376`); `Str` ->
  `rquickjs::String::from_str` (copy); `Str16` -> `JS_NewStringUTF16`; `Handle` -> `__svelters_node(h)` global lookup
  + JS call (`conv.rs:428-436`); `Enum` -> member **name string**; `Flags` -> array of names; `Record` -> plain
  object, `f.name.clone()` String per field then `o.set(k, x)` (`:984-1002`, `conv.rs:401-407`); `Bytes` ->
  `TypedArray::<u8>::new_copy`; `Buffer` -> zero-copy (E2); `Boxed` -> `__svelters_internal.makeBoxed`; `Error` ->
  `makeNativeError`.
- `MAX_DEPTH = 16` (`:23`); `MAX_SAFE = 2^53` (`:24`).

### 1.5 How prop names / kinds / signals travel
- **Compiled components pass strings.** Sample `crates/svelters-compiler/tests/runes/effects/output.js`:
  `const $$n1 = $n.element("label"); $n.track($$n1, "label", "notify::label", -1); ...
  $.template_effect(() => $n.set($$n1, "label", ...))`. Codegen: `codegen/component.rs:800, 922, 928`
  (`$n.set({n}, {js_str(prop)}, ...)`).
- The runtime forwards them as strings: `create` -> `hs.create(kind)` (`packages/native-runtime/src/index.js:507`),
  `set` -> `node.host.setProp(node.h, prop, value)` (`index.js:645`), `listen` -> `node.host.listen(node.h, signal,
  on[, flags])` (`index.js:906-907`). The `*Id` resolvers exist (`quickjs.rs:1008-1050`) but the runtime uses
  `kindId/methodId` only for toplevel method picking and provider methods (`index.js:386-403, 1033-1062`).
- Host side resolves name -> `svl_id` per call via the cached nested HashMap (above). egui resolves names with
  `kinds::find_prop(k, name)` etc. (`svelters-egui/src/ops.rs:101-115`).
- Rust -> JS: signals are posted by id; the host maps `(kind, signal id)` back to a **cloned `String`**
  (`core.rs:335`, `core.rs:900-904`) and calls `__svelters_dispatch(handle as f64, signal: &str, list, deferred,
  flags)` (`quickjs.rs:523-546`); `__svelters_changed` gets `[h, prop.as_str(), value, ...]` flat triples
  (`quickjs.rs:601-623`). JS then builds a key string `` `${signal}\0${flags}` `` per dispatch (`index.js:900`).

### 1.6 How handles look in JS
- A handle is a **JS number** (`f64`) everywhere on the host API (`handle_of` `quickjs.rs:828`; results `h as f64`).
  egui `NodeId` = `generation << 24 | index` in a `NonZeroU64` (`svelters-egui/src/scene/id.rs:29-39`); host
  refuses handles `>= MAX_HANDLE` (`core/ops.rs:246-249`). App-export handles have bit 52 set
  (`exports/mod.rs:51-55`: "Bit 63 ... cannot cross into JS, whose handles are numbers below 2^53, so the app range is
  [2^52, 2^53)").
- JS-side wrapper: `class Native` (`index.js:84-150`) holds `h`, `kind`, `host`, and exposes `get svlHandle()`
  (`:129-131`), which `convert.rs` reads when an object is passed to a handle slot (`conv.rs:261-268`
  `node_handle`: `o.get("svlHandle")`, a property lookup + getter call).
- `__svelters_node(h)` (`index.js:1319-1335`, installed `:2878`): a handle in a host result becomes the node we
  created (`table(hs).get(h)`), else one stable "borrowed" `Native` per handle held by `WeakRef` and released with
  `FinalizationRegistry` -> `hs.destroy(h)` (`index.js:1299-1317`). Generated `svelters:app/*` classes
  (`NativeObject` in `packages/types/src/core.js`) also expose `svlHandle` and register a FinalizationRegistry when
  there is no renderer (`exports/codegen.rs:122-126`, e.g. `examples/egui-kinds/app/.svelters/app/cad.js`).

### 1.7 Rust -> JS calls (events, frames, timers, completions)
Each is one `call_guarded` entry, looking the function up on `globals()` by name every time:
`dispatch` (`:523`), `dispatch_sync` (`:548`, result via `sync_result_value`), `run_frame`
(`__svelters_internal.frame`, `:568`), `dispatch_gone` (`:581`), `assets_changed`, `dispatch_changed` (`:601`),
`fire_timer` (`:625`), `settle_ops` (`:669`, flat `[kind, id, value]*` array), `mount_root`/`root_call` (EM1,
`:716-745`, props as plain objects from `(String, Value)` pairs `:186-194`). Callback values (`CALLBACK`) invoke
`Persistent<Function>` with args decoded from `AV` (`:100-145`). Event args: `AV::List(args.to_vec())` (a clone of
the queued owned `Vec<Value>`) then `decode` to a JS array (`:541-543`).

### 1.8 Exports, stores, records (`#[svelters::export]`, `#[svelters::store]`, svelters-macros)
- `exports/mod.rs:1-13`: "Exports never go through the backend C ABI ... kinds named `app:<module>.<Type>` (ids in the
  reserved pack 63) with ordinary `MemberDesc`s, so the host converts and validates arguments exactly as for a
  backend ... Nothing here is engine specific: thunks see owned `Value`s."
- Traits: `JsType::describe`, `FromJs::from_js(v: Value, cx)`, `IntoJs::into_js(self, cx) -> Value`
  (`exports/mod.rs:86-103`) - note these are svelters' own traits over `svelters_abi::Value`, not rquickjs'
  FromJs/IntoJs.
- `app_set_prop` (`core/apps.rs:256-277`): `encode` -> `owned_all` (copies into owned `Value`, commits E2 buffers) ->
  op-log `note_op(json_string(&value))` closure (only formats when logging) -> thunk -> macro-generated `from_js`.
  Results: thunk returns `Value` -> `decode_slot` -> JS (`app_call` `core/apps.rs:198-215`).
- Macros (`crates/svelters-macros/src/lib.rs`): `derive(Record)` `:176` (`into_js` builds
  `Value::Record { ty, fields: vec![..] }` `:234-235`), `derive(Enum)` `:244` (JS side = kebab-case names;
  `Value::Enum { ty, value: i64 }` `:283-293`), `derive(Error)` `:398`, `object` `:480`, `export` `:1076`, `store`
  `:1265`, `service` `:1438`, `derive(Events)` `:1511`, `widget` `:1665`, `wrap_widget` `:1699`, `behavior` `:2116`.
- Stores (`exports/reactive.rs:1-17`): "a write sets one dirty bit per field ... At the next checkpoint the host
  folds the dirty fields into R1's dirty set, so store changes, native property changes and events reach JS in one
  `__svelters_changed` batch with one job drain. A field is read (cloned under the read lock) only when it is
  delivered, and compare-with-last drops unchanged values". CX3 `host.link` binds a store field to a native prop
  with no JS per change (`quickjs.rs:1304-1317`, `core/links.rs:32`).
- `Json<T>` (`exports/json.rs:1-12`): serde values cross "as a plain JS value ... with no JSON text in between ... serde
  reads the owned `Value` tree directly". So serde exists only on this export path (and op logs), never on widget
  props.

---------------------------------------------------------------------------------------------------
## 2. The measurements (`docs/notes/js-native-serdes.md`, bench `docs/notes/scripts/serdes_bench/`)

Setup (§4.1, `:428-441`): bench crate with path deps on svelters-abi/svelters-host, rquickjs 0.14.0, opt-level 3.
Part A = real host path into A2 (raw C-ABI backend reading borrowed `svl_value`, the Qt/GTK case) and A1 (Rust
`Backend` trait, copies into owned `Value`, the log-backend case; also egui's case today). Part B = prototypes on a
bare rquickjs context (`js/b.js`, `src/main.rs`). "Each row is 10 000 ops ... best of 9 rounds ... min and median over
7 processes ... ns per op, including the JS loop (~150 ns/iteration). Machine: i7-8850H, `powersave` governor, **load
average 27-31 on 12 threads** ... Absolute numbers are therefore inflated, probably 2-3x".

Part A (ns/op, min / median), `:445-459`:
| op | A2 raw C-ABI | A1 Rust trait |
|---|---:|---:|
| JS loop only | 144 / 163 | 149 / 169 |
| `setProp(h,'spacing',i)` int | 1433 / 1567 | 1448 / 1615 |
| `setProp` f64 | 1423 / 1711 | 1472 / 1757 |
| `setProp` bool | 1505 / 1579 | 1632 / 1789 |
| `setProp` string 11 B ASCII | 1651 / 1704 | 1754 / 1998 |
| `setProp` string 1 KiB ASCII | 2044 / 2291 | 3906 / 4680 |
| `setProp` string 1 Ki chars non-Latin-1 | 22693 / 24745 | 34165 / 36344 |
| `setProp` enum as string `'vertical'` | 1721 / 1788 | 1965 / 2089 |
| `setProp` rect as LIST `[x,y,w,h]` | 2935 / 3086 | 3346 / 3571 |
| `getProp` int | 1403 / 1530 | 1486 / 1694 |
| `call(h,'m',[a,b])` | 2609 / 2818 | 2717 / 3057 |

Part B (ns/op, min / median), `:463-482`:
| op | min | median |
|---|---:|---:|
| JS loop only | 177 | 189 |
| JS only: allocate `{x,y,width,height}` | 1374 | 1558 |
| JS only: 4 `Float64Array` stores | 732 | 906 |
| native no-op call `(h, id, v)` (rquickjs `Function`, 3 args) | 837 | 941 |
| `setPropId(h, id, i)` int, pre-resolved id | 768 | 986 |
| `setPropId` string 11 B (borrowed UTF-8) | 812 | 1114 |
| `setPropId` enum as number | 802 | 1158 |
| `setPropId` rect as LIST `[i,2,300,400]` (array literal per op) | 1763 | 2048 |
| `setPropId` rect as object literal per op -> typed RECORD | 3171 | 4336 |
| `setPropId` rect, same object reused -> typed RECORD | 1777 | 1902 |
| `setPropId` rect, object behind a `Proxy` get-trap (like `$state`) | 4337 | 5304 |
| `setRect4(h, id, x, y, w, h)` flat-args thunk | 1276 | 1633 |
| batched int (JS encode + flush + decode + dispatch) | 1530 | 1737 |
| batched string 11 B | 1983 | 2256 |
| batched rect object literal | 3724 | 4195 |
| batched mixed (int, string, rect) per op | 1470 | 2106 |
| batched string as UTF-16 code units copied into a `Uint16Array` in JS | 8702 | 10379 |
| register + unregister a JS function as a persistent root | 2170 | 2640 |

Strings, one native call per op (`:486-491`): 11 B ASCII `String` 801/866, borrowed UTF-8 686/737, borrowed UTF-16
668/853; 1 Ki ASCII 1345/1433, 983/1088, 1279/1338; 1 Ki Latin-1 10529/12764, 5567/6167, 1042/1363; 1 Ki wide
21889/23909, 8639/12372, 595/708. Qt side (`:495-500`): 11 B `fromUtf8` 116-147 ns, `fromUtf16` 150-202 ns; 1 Ki wide
`fromUtf8` ~15 100-16 200 ns vs `fromUtf16` ~310-390 ns.

Conclusions (§4.3, `:504-519`, quoted):
1. "**The JS->native call dominates. The value encoding barely matters for scalars.** A native no-op with three
   arguments costs about as much as a full pre-resolved `setPropId` with int, string or enum (~0.8 us here). Building
   a 24-byte value on the stack is noise."
2. "**Passing prop names as strings is the largest avoidable cost in today's path.** `setProp(h, 'spacing', i)` costs
   1.43 us; `setPropId(h, id, i)` costs 0.77 us." (name `String` alloc, `resolve()` key `to_string()`, handle table +
   RefCell traffic.)
3. Strings: ASCII free with borrowing; non-ASCII "~20x cheaper" to Qt as UTF-16.
4. "**The Rust `Backend` trait's owned `Value` copy doubles the string cost** (1 KiB: 2.0 -> 3.9 us; wide: 23 -> 34
   us) ... The trait should hand out a borrowed view."
5. "**Structs: an object costs about as much as an array, and most of the cost is JS allocation.** Allocating
   `{x,y,width,height}` in QuickJS costs ~1.2 us over the loop, more than the native call ... Svelte `$state` proxies
   are the real hazard: reading 4 fields through a `get` trap adds ~2.5 us."
6. "**Batching does not pay in QuickJS.** Encoding into a `Float64Array` from interpreted JS costs about as much as a
   native call: 4 stores ~0.6 us vs. a call ~0.8 us. Batched int (1.53 us) and batched rect (3.7 us) are *slower*
   than direct calls ... The trade flips for JIT engines (V8 under Node/Deno)".
7. "**Callback registration is ~2 us**".
Proposed (§5, `:553-717`): keep nested 24-byte `svl_value` as the one backend ABI (v2 = tag/flags/type split, ENUM,
FLAGS, RECORD, BYTES, BOXED, CALLBACK, MAP, ERROR, UTF-16 flag, static descriptors); "No BIGINT, no CBOR/FlatBuffers on
the backend ABI. A packed op buffer is an optional *front* encoding for N-API and Deno FFI only." §5.2: pass ids not
names on hot ops; type-directed conversion in core; borrow strings; `ValueRef` instead of owned `Value`; handles
become canonical wrapper objects; finalizers only enqueue. (ABI v3 shipped this: `svelters-abi/src/lib.rs:8`
`SVL_ABI_VERSION: u32 = 3`.)

Follow-up after implementation (`docs/notes/s1-handoff.md:43-54`, best of 7, load ~30, "~90 ns JS loop"):
| row | before | after, UTF-8 backend | after, UTF-16 |
|---|---:|---:|---:|
| `setProp` int by name | 2337 | 2235 | 2456 |
| `setProp` int by id | n/a | 1607 | 1860 |
| 11 B ASCII by name | 2628 | 2284 | 2551 |
| 11 B ASCII by id | n/a | 1699 | 1864 |
| 1 KiB ASCII | 5965 | 2464 | 2774 |
| 1 KiB Latin-1 | 28702 | 7137 | 2747 |
| 1 KiB wide | 43722 | 13640 | 1834 |
| record `{x,y,width,height}` | unsupported | 4768 | 4862 |
"By id is ~28% faster than by name; the owned-`Value` Rust backend after the change costs about what master did".

Other per-op numbers in the repo:
- `docs/notes/egui-rust-exports.md:152-170` (rquickjs 0.14.0, load 11-14): "plain JS function call (QuickJS) |
  234-325 ns"; "rquickjs `#[class]` method `sim.add(1)` | 721-1041 ns"; "rquickjs `#[class]` field getter | 676-874
  ns"; "`host.call(h, 'add', [1])` through `svelters-host` + C ABI to a Rust `Backend` | 2.9-4.0 us"; "host
  `invoke_callback` (settling one completion), Rust side | 19.5-23.8 us"; "10,000 async noops in flight | 52-61 us
  per op". Conclusion `:174`: "The ABI path costs 3.5-5.5x an rquickjs method, which costs about 3x a JS call."
- `docs/notes/egui-js-widgets.md:20-22`: "Recording one draw op from JS costs about 1.0-1.1 us through a native host
  function per op. About half of that is QuickJS just evaluating the argument expressions (0.46-0.51 us for the loop
  and arithmetic alone) ... The same scene built in Rust costs 6 ns per op, about 170x less." `:58-59`: C asserts
  off: no difference; interrupt handler off: "at most ~15%".
- `docs/notes/p5-integration.md:63-93` (1000-row keyed list, 11000 ops, release): Fill: JS 1479 ms (GTK) / 1907 ms
  (Qt), **host/ABI 16 / 18 ms**, toolkit calls 423 / 226 ms. Finding 1: "**QuickJS time dominates** ... the host/ABI
  layer is negligible (0.1-1.5 % of the JS time)." Finding 5: "11000 ops cost 16-18 ms of host/ABI time in total, so
  batching to save ABI crossings is not justified". (`host/ABI` = time in host ops minus time in backend calls,
  measured inside Rust, so it excludes QuickJS's own call/marshalling overhead.)
- `plans/research/EGUI.md:105`: "batching into a `Float32Array` / JS-side encoder / today's X8 JSON | 1.2-1.4 / 2.1-2.3
  / 4.8-6.0 us per op: batching doesn't help in QuickJS".

---------------------------------------------------------------------------------------------------
## 3. JS-side runtime bookkeeping (`packages/native-runtime/src/index.js`, 2941 lines)

Header (`:1-8`): "There is no JS tree of DOM-like nodes and no anchors: containers and regions keep just the
ordering native insertion needs." Globals: `__svelters_host / __svelters_dispatch / __svelters_dispatch_sync /
__svelters_gone / __svelters_node / __svelters_recover`.

Per-node allocation (`class Native`, `:84-109`): one object with ~20 fields and **8 `Map`s + 1 `Set` + 1 `{}`**
eagerly: `last`, `captured`, `tokens`, `handlers`, `listening` (Set), `cattrs`, `passive`, `slots`, `attrs`. Then
`table(hs).set(n.h, n)` (handle -> Native `Map` inside a `WeakMap` keyed by host, `:64-69`) and
`scope.natives.push(n)` (`:504-511`). Ownership `Scope` objects (`:74-82`) with `natives[]`, `children: Set`,
`handlers[]`. Containers/regions: `Slot`, `Root`, `Region` with `items[]` (`:173-202`); insertion computes the next
native sibling by walking regions with `indexOf` (`nextNative` `:234-243`).

Per `set(node, prop, value)` (`:617-646`) before the host call:
- RESTORE check; `alive(node)`; `plainData(value)` (`:657-687`: `is_state_proxy` check; deep copy of any `$state`
  proxy, else returns the value; for plain objects/arrays it walks every key);
- `same` = `Object.is` or deep `sameParam` (content compare for proxied data, `:538-546`);
- `node.requested` read: **not initialised in the constructor**, so the lookup misses own props and falls into the
  Proxy installed as `Native.prototype`'s prototype (`:1063-1072`), whose `get` trap does `Reflect.get`, `instanceof
  Native` and `providerMethod` (two `Map` lookups in `kindMethods`, `:1033-1062`). Same for `node.snippets`,
  `node.inertFlag` (read for every ancestor in `inertNode` per dispatch, `:1075-1078`), `node.anchorScope`. (Inferred
  from code; not measured anywhere.)
- last-value cache: `node.last.has(prop) && same(node.last.get(prop), value)` then `node.last.set(prop, value)`
  (`:636-637`): drops unchanged writes in JS (no host call);
- function -> snippet prop; `prop === 'anchor'`; then `node.host.setProp(node.h, prop, value)` with the **string**
  prop name.
Compiled code wraps this in `$.template_effect(() => $n.set(...))` (Svelte signal graph cost on top).

Events (`dispatch` `:1123-1130`, `dispatchTo` `:1171-1187`): `table()` (calls `host()` = `globalThis.__svelters_host`
lookup, `WeakMap.get`, `Map.get(h)`), `alive`, `inertNode` ancestor walk, optional `dispatchHook` (EM2 per-root
context), `hkey(signal, flags)` template string, `handlersOf` copies the handler array, passive mirrors first. Sync
signals allocate a `NativeEvent` object (`:1087-1114`) and fill DOM-style fields from positional args for propagating
events (`propagatingEvent` `:1225-1265`). `bind` (`:984-996`) = `template_effect` + `on(signal)` + `getProp` read-back
when the signal carries no value.

Mapping node <-> handle: `Native.h` (number) is the only link; handle -> node via the per-host `Map`
(`table`), borrowed handles via `WeakRef` map + `FinalizationRegistry` (`:1299-1335`). Nothing native lives on the JS
object; the scene holds no JS reference.

How much is JS bookkeeping vs crossing: no doc isolates runtime bookkeeping per op. Evidence: P5 says JS (Svelte +
runtime) ~1.5-1.9 s vs host/ABI 16-18 ms for 11000 ops (~100x); egui-js-widgets says half of a ~1 us per-op host call
is QuickJS evaluating arguments; JIT note perf (`quickjs-jit-cranelift.md:316-321`): "`JS_CallInternal` 71% self
time ... `find_own_property` 7.8%, `JS_FreeValue` 3%, `map_find_record` 2.4%, GC ~2%".

---------------------------------------------------------------------------------------------------
## 4. QuickJS-NG / rquickjs versions, features, build, loader, JIT

- Root `Cargo.toml:57`: `rquickjs = { version = "0.14.0", default-features = false, features = ["std", "loader"] }`.
  `Cargo.lock:4097-4120`: rquickjs / rquickjs-core / rquickjs-sys 0.14.0 from crates.io. QuickJS-NG = `0fdea21` =
  v0.16.2+26 (`docs/notes/qjs-research.md:11`). No `[patch.crates-io]`, no vendored C, no patches.
- `Cargo.toml:64-67`: "QuickJS is C compiled by the cc crate; at the dev profile's opt-level 0 it runs 5-10x slower ...
  `[profile.dev.package.rquickjs-sys] opt-level = 2`". No release profile override (default opt-level 3). No
  `disable-assertions` in the workspace (egui-js-widgets measured it: no difference).
- svelters-host features (`crates/svelters-host/Cargo.toml:45-51`): `default = ["intl"]` (IN1: own `Intl` over ICU4X,
  `quickjs/intl.rs`), optional `tokio`.
- Module loader: `crates/svelters-modules` (`src/lib.rs:1-15`): engine-agnostic `ModuleProvider` (`BundleProvider`,
  `SourceProvider` with live `.svelte` compile via svelters-compiler + oxc resolution, `NativeModules`/
  `StaticModules`, `DylibModules` behind `dlopen`, `Svb` bundle format); `ModuleCode::Source{js,map}` or
  `Cached{engine, bytes}` (`:46-51`). quickjs.rs keeps a thin `ProviderResolver`/`ProviderLoader` (`:1455-`) and
  `declare_code` (`:1532-1549`): bytecode cache via `Module::load`/`Module::write`, HMR `?v=n` names, `import.meta.hot`.
- Upgrade candidates noted: NG 0.17.0 with rquickjs 0.15 (QUICKJS-RUNTIME §4 #16); NG PR #1744 (GCC 14+ JSValue
  codegen, "1.19-1.20x geomean ... A free ~1.2x for everything", `egui-js-widgets.md:372`, `EGUI.md:119`); NG PR #1421
  debugger (the one C patch that would justify a submodule-pointer fork, Q7).
- `docs/notes/quickjs-jit-cranelift.md` conclusions: quickjs-jit "already *is* a Cranelift JIT for QuickJS-NG,
  packaged as an rquickjs fork" (28 patches, ~9.3k lines, NG 0.15.1, 59k lines of 6-week-old agent-written code, one
  maintainer). Measured (ms per 10 calls, `:296-301`): props kernel 8.24 interp / **0.40** JIT / 7.21 stock NG 0.16.2
  / 0.19 node / 0.32 bun; appmix 36.5 / 34.6 / 26.1 / 0.98 / 1.33; signals (real Svelte 5) 136.6 / 129.1 / 100.2 / 4.86
  / 6.33; proxy 415 / 448 / 341 / 55.0 / 45.3. "JIT vs stock QuickJS-NG 0.16.2 ... props 18x faster; **app-like
  workloads 0.76-0.78x, i.e. slower**"; "V8 vs stock ... about **20x** on the Svelte signals graph". Recommendation
  (§6): "**Do not build our own Cranelift JIT, and do not adopt quickjs-jit now** ... A homegrown equivalent is 2-8
  person-months, plus a permanent rebase cost, for a likely <=1.3x"; "If QuickJS throughput becomes the bottleneck,
  V8 (deno_core) is the lever"; "Keep the op layer engine-neutral". The later rebased quickjs-jit 0.14.1 (exactly our
  engine) re-measured in `egui-js-widgets.md:375-392`: empty loop 17-26x, but "one native host fn call per op ...
  **0.7-0.9x**" (host calls got slower under the JIT). Verdict "No".

---------------------------------------------------------------------------------------------------
## 5. `plans/research/QUICKJS-RUNTIME.md` (status: research proposal, 2026-10-17)

- §1 "The forks" table (`:38-43`): `akiselev/rquickjs` (2026): "0 own commits ... 283 behind master | Nothing to
  keep"; `akiselev/rquickjs-old-fork` (2021): public raw Ctx, eval_global, typedmap userdata, backtraces,
  is_null/undefined, reentrant runtime mutex, QuickJS -> akiselev/quickjs: "Upstream has replaced all of it ... The
  host's `RunningCtx` handles re-entry. Drop"; `akiselev/quickjs` (2021, koush debugger): "Obsolete base. Its debugger
  is the only feature without a current equivalent"; `akiselev/mquickjs-rs`: MicroQuickJS, ES5-ish, "Svelte 5 cannot
  run on it". So the "rquickjs fork" being discussed was the user's own GitHub fork, kept for the old v1 needs
  (raw ctx for toolkit-thread callbacks, global eval for the JS Svelte compiler, typed userdata, debugger), not a
  scene-integration fork.
- Upstream since the fork point (`:45-57`): 0.12 import attributes, interrupt+ExecutePendingJob GC fix, rope-string
  fixes, "`ArrayBuffer::from_source` for zero-copy", `FromJs`/`IntoJs` derives, class macro fixes; 0.13 NG 0.16.2,
  `TypedArray` byte views `unsafe`; 0.14 `execute_pending_job` clears the exception; master on NG 0.17.0.
- Plan (`:59-77`): stay on crates.io 0.14.x; C patch -> branch `svelters/v0.14.x` with only the submodule pointer;
  `[patch.crates-io]` in root only; don't vendor.
- §4 ranked capabilities (`:252-270`), # / feature / works today / value / cost:
  1 dev loader (yes, H, M); 2 bytecode cache/table (yes, H, S, "10x faster warm loads"); 3 in-memory source maps +
  CallSite frames (yes, H, S); 4 async host ops on own completion queue (patterns exist, H, M, "not rquickjs
  `futures`"); 5 built-in `svelters:*` modules (`ModuleDef`, M, S); 6 debugger NG PR #1421 + DAP (no, needs C patch,
  H(DX), L, "The only reason to fork"); 7 WeakRef/FinalizationRegistry for handle GC (yes, M, "keep the explicit
  destroy paths"); 8 memory limit, interrupt handler, stack size (yes, used); 9 worker runtimes, messages as
  `svelters_abi::Value` (yes, M, M, "No shared JS objects; `SharedArrayBuffer` + `Atomics` exist if needed"); 10
  profiler via interrupt-handler sampling (partly, M, M); 11 Intl (no, M, L; since done as IN1); 12 structuredClone /
  Temporal (no, L); 13 "`#[rquickjs::class]` bindings for widgets | yes | L | — | Widgets go through handles and the C
  ABI by design, so the engine stays swappable. Don't use it"; 14 `parallel` feature (L, not needed); 15 newer ES
  (yes on 0.16.2); 16 NG 0.17 (M, S); 17 heap snapshots (no).
- §5 engines (`:272-294`): QuickJS-NG stays (6 ms startup, 1.5 MB static binary; weak points no Intl, no JIT
  "(an interpreter at 1/12-1/85 of JSC JIT on compute kernels)", no debugger); MicroQuickJS no; JSC Apple-only
  option behind `JsEngine` (iOS 1.6-1.8x, no JIT there; 27-32 ms startup, 36 MB RSS); Boa/Nova pure Rust, Boa
  "historically slower", Nova experimental; quickjs-jit rejected. "Keep the `JsEngine` boundary honest."
- §6 phasing Q0 ("Stay on crates.io 0.14.x ... No fork") .. Q7 (debugger fork via `[patch.crates-io]`).

---------------------------------------------------------------------------------------------------
## 6. Where the scene stores prop values (egui) and the ABI Value type

### 6.1 Scene (`crates/svelters-egui/src/scene`)
- `mod.rs:1-3`: "one arena of nodes stored as parallel columns per slot". `Scene` (`mod.rs:88-139`): `slots`,
  `core: Vec<NodeCore>`, `children: Vec<Vec<NodeId>>`, `layout: Vec<LayoutBox>`, `payload: Vec<Payload>`, `cold:
  Vec<Option<Box<NodeCold>>>`, `common: Vec<Option<Arc<Common>>>` (SG6 interned), `styles`/`commons` interners,
  `ext: ExtColumns` (provider data, one typed column per `ExtKey`, SG2), `journal`, `roots`, indexes, hit lists,
  `style` (CSS engine state) ...
- `NodeCore` (`node.rs:340-362`): parent, `kind: u32`, root, flags, dirty, owner, state bits, `listen: u64` (a bit per
  signal ordinal), `explicit: u64` (a bit per prop ordinal set by a runtime), `egui_id`, `rect`, `drawn`, `changed`.
- Common props (`node.rs:90-140+`, `struct Common`): typed Rust fields: `enabled: bool`, `visible: bool`, `tooltip:
  String`, `accessible_name: String`, `anchor: String`, `anchor_scope: Option<Box<str>>`, `width..max_height:
  Option<f64>`, `flex_grow: f64`, `align_self: &'static str`, `flex_direction: &'static str`, `gap: f64`, ...
  **Enums are stored as `&'static str`** picked from a static list.
- Per-kind payload `enum W` (`node.rs:19-59+`): `Label { text: String, wrap: &'static str, selectable: bool, weight:
  i64, rich }`, `Button { text, selected, frame, small, weight }`, `TextEdit {..}`, `ComboBox { options: Vec<String>,
  selected: i64, .. }`, `Ext(ExtWidget)` (provider `#[svelters::widget]` structs), `VirtualList(Box<..>)`, `Table(..)`,
  ...
- Prop types: `kinds::Ty` (`kinds/mod.rs:107-126`): `Bool, F64, I64, Str, Enum(&'static [&'static str]), StrList,
  IdList, NumList, Snippet, Provider`. Defaults `Def { Null, Bool, F64, I64, Str }` (`:130-136`). Described to the
  host as `tag_of`: `Ty::Str | Ty::Enum(_) => SVL_STR` (`scene/props.rs:404-414`), i.e. egui enums travel as
  strings, not ABI `ENUM`.
- Write path: `EguiBackend::set_prop(h, prop: svl_id, v: &Value)` (`svelters-egui/src/ops.rs:209-223`):
  `scene.borrow_mut()`, `scene.live(h)`, DZ0 overrides HashMap check, `Scene::set_value(id, prop, v, Origin::Js)`
  (`scene/access.rs:891-`): `kinds::prop_def(kind, prop)`, anchor bookkeeping, for common props `set_common`
  (`mod.rs:285-295`: clone the interned `Common`, mutate, compare with DEFAULT, hash + intern) then
  `props::set_prop` (`props.rs:~85-140`): `v.clone()` when not null, then `as_str` (`props.rs:39-47` clones the
  String again), `as_enum` (`:49-52`: `as_str` + linear search over `&'static [&str]`), `as_f64`, `as_len`, ...; returns
  `Dirty` bits; `touched(root)`.
- Typed zero-conversion reads from Rust exist: `PropKey<T>` (`scene/keys.rs:1-60`; ARCH-SCENE `:595-616` "Typed prop
  read, zero-copy ... a typed read is a borrow of the field and costs no `Value` conversion"). Typed writes still
  go through `PropValue::into_value` -> `Value` -> the same write path (`keys.rs:13-48`).

### 6.2 ABI value (`crates/svelters-abi`)
- C struct (`src/lib.rs:186-192`): `#[repr(C)] struct svl_value { tag: u16, flags: u16, type_: svl_id(u32), u:
  svl_value_u }`, union of `b: bool, i: i64, u: u64, f: f64, h: svl_handle(u64), s: svl_str{ptr,len}, l:
  svl_list{ptr,len}, bytes: svl_bytes{ptr,len}` (`:176-185`) = 24 bytes. Tags `SVL_NULL..SVL_ERROR` (0..14,
  `:14-28`), flags `UTF16, NUL_TERM, UNSIGNED, OWNED_BY_CALLEE` (`:31-34`), well-known types `RECT, SIZE, POINT, INSETS,
  COLOR, DATE, TIME, DATETIME, VARIANT, TYPED_VARIANT, TYPE` (`:37-48`), descriptor flags (`:52-61`), member flags
  (`:62-75`), `SVL_ABI_VERSION = 3` (`:8`).
- Owned Rust enum (`src/safe.rs:118-148`): `Value::{Null, Bool(bool), I64(i64), U64(u64), F64(f64), DateTime(f64),
  Str(String), Str16(Vec<u16>), Handle(svl_handle), List(Vec<Value>), Enum{ty, value: i64}, Flags{ty, mask: u64},
  Record{ty, fields: Vec<Value>}, Bytes(Vec<u8>), Boxed{ty, handle}, Callback{id, ty}, Map(Vec<(String, Value)>),
  Error(Box<NativeError>), Buffer(BufferValue)}`. Borrowed views `ValueRef<'a>` / `ArgsRef` (`safe.rs:432-444`).

### 6.3 Representation per value, each layer (today)
| value | JS (Svelte/runtime) | rquickjs/host | ABI | egui scene |
|---|---|---|---|---|
| int | number (QuickJS int32 or float64 JSValue) | `f64` via `r.number` -> range check | `svl_value{I64}` | `Value::I64` -> `i64` / `f64` field |
| float | number | `f64` | `F64` | `f64`, `Option<f64>` |
| bool | boolean (also `""`/"true"/"false" strings) | bool | `BOOL` | `bool` |
| string | JS string (8-bit or 16-bit JSString) | borrowed C string (ASCII zero-copy) | `STR{ptr,len}` borrowed | `Value::Str(String)` copy -> clone -> `String` field |
| enum | kebab string | string -> `HashMap<String,i64>` (backend ABI enums) | `ENUM{type,i}` | egui: `STR` -> `&'static str` by linear search |
| flags | array of names | mask | `FLAGS{type,u}` | - |
| record | plain object | per-field `has_field`+`field` | `RECORD{type, l: fields}` | provider/`Ext` structs |
| handle | `Native` object (`svlHandle` getter) or number | `f64` -> u64 | `HANDLE{h}` | `NodeId` (gen<<24|idx) |
| list | array | per-index `get` | `LIST` | `Vec<String>`/`Vec<i64>`/`Vec<f64>` |
| bytes / big data | ArrayBuffer/TypedArray | borrowed ptr; E2 `Buffer<T>` transfer | `BYTES` borrowed / `Value::Buffer` | n/a |
| callback | function | `Persistent<Function>` in `QjsCallbacks.funcs` HashMap by id | `CALLBACK{id}` | n/a |

---------------------------------------------------------------------------------------------------
## 7. Shared memory, typed arrays, zero-copy, opaque, classes

- E2 buffers (implemented): `crates/svelters-host/src/quickjs/buffers.rs` - `JS_NewArrayBuffer(raw, ptr, len, 0,
  Some(ext_free), opaque, false)` over Rust memory (`:164`); SharedArrayBuffer refused: "a {what} over a
  SharedArrayBuffer cannot cross to Rust" (`:314`); `JS_NewArrayBufferCopy` fallback (`:453`). `exports/buffers.rs:17`
  "no data race, and no `SharedArrayBuffer`". Milestone `plans/milestones/E2-exports-buffers.md` ("zero-copy typed
  arrays"; SharedArrayBuffers "rejected", `:129`). `alloc(Float32Array, n)` host op (`quickjs.rs:1422-1439`).
- Measured (`docs/notes/egui-rust-exports.md:35-39, 155-163`): 64 MiB `Arc<[f32]>` -> read-only `Float32Array`
  "0.019-0.045 ms" vs copy 80-94 ms; `ArrayBuffer.prototype.transfer()` "0.024-0.031 ms"; ABI `SVL_BYTES` 315-652 ms;
  writes to the immutable view "silently ignored". `:517`: "**`SharedArrayBuffer` is deliberately not offered.**
  QuickJS has it, but racy shared mutable memory between a worker and JS gives up the main safety property."
- `#[rquickjs::class]` was only probed (`docs/notes/scripts/egui-exports-probe/src/direct.rs:3,63`;
  `docs/notes/scripts/ownership/rs/src/bin/qjs_fin.rs:37`), never used in the host; QUICKJS-RUNTIME §4 #13 "Don't use
  it". egui-rust-exports `:150`: a "quickjs-fast" feature could emit direct rquickjs functions for scalar-only
  `#[js(fast)]` methods - "Don't build it until an app shows a chatty hot path."
- No `JS_SetOpaque`/`JS_NewClass` use anywhere in crates; opaque pointers appear only as the ArrayBuffer free-func
  opaque in E2.
- Static-template op / AOT: `plans/research/EGUI.md:221` "84% of prop writes in our 41 GTK components are constants,
  but on egui host calls are already in-process; a **static-template op** (all backends) is the better variant";
  egui-js-widgets E-W2 paint-subset AOT to Rust (~6 ns/op).
- Isolates: ARCH-SECURITY ISO1 (`:519`): "The transport trait carries bytes or `svelters_abi::Value` only, never
  `Arc`/pointers; `Shared<[T]>` crosses as a copy or a named shared-memory segment".
- js-native-serdes §2.2 bytes: "QuickJS gives no way to forbid detaching a JS-allocated buffer, so pinning is reserved
  for buffers the native side allocated" (`:239`).

---------------------------------------------------------------------------------------------------
## 8. Where time goes per op today (synthesis)

For one `$n.set(node, 'text', v)` on egui:
1. Svelte: template_effect re-run (signal graph; the JIT note's perf shows `JS_CallInternal` 71% of time on the Svelte
   signals workload) - dominant in real apps (P5: ~100x host/ABI).
2. Runtime JS: `set()` bookkeeping (plainData, last-value Map, Proxy-prototype miss on `requested`), `host()` global
   lookup, method lookup on `__svelters_host`. Not separately measured.
3. Boundary: rquickjs `Function` call with 3 args ~0.84 us loaded (serdes Part B no-op, loop included; ~0.66 us net;
   the doc estimates 2-3x inflation, i.e. a few hundred ns real). egui-rust-exports: plain JS fn 234-325 ns vs rquickjs
   class method 721-1041 ns.
4. Host Rust: name resolve by string (setProp by name 1.43 us vs by id 0.77 us in serdes; 2235 vs 1607 ns after S1,
   ~28%), handle/name/member HashMaps (std SipHash), RefCell borrows, crash ring push, descriptor-driven encode,
   liveness re-check, observe check. P5: host/ABI ~1.5 us/op wall (16-18 ms / 11000).
5. ABI + egui: C fn pointer, owned `Value` copy (default `set_prop_ref`), `v.clone()` + `as_str` clone, `set_common`
   clone+intern, dirty bits, `touched`. Strings ~2x more expensive through the owned path (serdes §4.3 #4).
