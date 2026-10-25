# svelters compiler <-> toolkit integration map (for "tessel")

Read-only survey of `/home/user/svelters` at master `1d6656c2`, 2026-10-25. Nothing was built or run.
All paths are relative to `/home/user/svelters` unless they start with `/`. Line numbers are at `1d6656c2`.
Context read first: `/home/user/cadatomic/docs/research/2026-10-25-own-gui-toolkit.md` §5, §6.1, §7, §9 (note its §13
says "Not touched: the backend ABI; the compiler; the module loader and HMR; svelters-style". The user now wants the
opposite for the compiler, so this map lists where the compiler can know the toolkit).

---------------------------------------------------------------------------------------------------------------------

## 0. One-paragraph picture

`svelters-compile` (crate `crates/svelters-compiler`) is a verbatim Rust port of Svelte 5.57.2's parser and scope
analysis plus a native lowering pass that resolves every tag, prop, event, slot, binding and style declaration against
a **backend semantic spec** (`metadata/semantics/<backend>.json`, the `PlatformSemantics` trait). It emits ES modules
that call two runtime namespaces: `$` (Svelte's own reactive core, re-exported) and `$n`
(`packages/native-runtime/src/index.js`, ~2.9k lines). `$n` keeps a thin JS mirror (`Native` objects, slots, regions,
scopes) and calls `globalThis.__svelters_host` (registered in `crates/svelters-host/src/quickjs.rs`), **one native call
per op, by string name**; the host resolves names to `svl_id`s (cached per kind) and calls the backend C ABI
(`crates/svelters-abi/src/lib.rs`), which already uses integer ids. The egui spec is generated from the Rust kind tables
(`crates/svelters-egui/src/kinds/*`, `spec.rs`); member ids there are positional integers that exist at run time but are
**not** in the spec JSON. A tessel backend fits into exactly the same slots, and every place below where the compiler
could "know the toolkit" is a field in that spec or a new `$n` primitive.

---------------------------------------------------------------------------------------------------------------------

## 1. The compile pipeline end to end

### 1.1 Phases (`crates/svelters-compiler/src/compile.rs`)

| Step | Where | What |
|---|---|---|
| options | `compile.rs:18-46` `CompileOptions` | `backend: &dyn PlatformSemantics`, `dev`, `css_scope`, `target: &dyn RuntimeTarget`, `strict_native`, `portability`, `hmr`, `scoped_css`, `utilities` |
| parse | `compile.rs:145` | Svelte parser port; `native: !backend.html_syntax()` (native mode: no HTML void/implicit-close rules) |
| TS strip | `compile.rs:153` | `svelte::ts::strip` |
| scopes | `compile.rs:163` | `analyze_component_scopes` (Svelte `phases/scope.js` port) |
| `@apply` | `compile.rs:175` | U4 utility expansion |
| scoped CSS | `compile.rs:176-181` | `scoped_css::scoped_css` (Rust port of Svelte's CSS pipeline) when `scoped_css` (egui, loader path) |
| sheet | `compile.rs:183` | `style::parse_style` (native subset; reports only setter declarations) |
| **lower** | `compile.rs:184` -> `lower/mod.rs:44` | AST + spec -> IR (`lower/ir.rs`) |
| checks | `compile.rs:185-199` | utilities, template await, `native_reads::check`, `portability::check` (`--backend gtk,qt,...`), `--strict-native` promotion |
| **generate** | `compile.rs:203-218` -> `codegen/component.rs:146` | `ScopeRewriter` (transform/) + codegen; DZ0 `source-loc` closure `has_loc` built at `compile.rs:211-216` |
| check-only | `compile.rs:134-136`, `codegen/component.rs:161` | CX1: same pipeline, writer sink |

Modules (`.svelte.js`): `compile_module` `compile.rs:244` -> `transform::transform_module` (`transform/mod.rs:828`).

### 1.2 Where the backend spec is consulted

| Question | API (`semantics/mod.rs`) | Called from |
|---|---|---|
| tag -> kind | `resolve_element` `:306` | `lower/mod.rs:760` |
| prop exists / writable | `property` `:324`, `writable_property` `:326` | `lower/mod.rs:1047`, `native_attrs.rs:289` |
| static value typing | `coerce_static` `:365` | `lower/mod.rs:1270-1277` (`push_static`) |
| event | `event` `:337` (returns `EventSpec{signal, sync, phases, args...}` `:154-170`) | `native_attrs.rs:96-125` |
| binding plan | `plan_binding` `:339` | `semantics/observe.rs:153` `plan_bind`, `:124` `plan_bind_commit` |
| observability | `observability` `:400` | `plan_bind` |
| placement | `place_child` `:341`, `attaches_to` `:344`, `lifetime` `:319` | `lower/mod.rs:835-886` |
| text content / runs | `text_content` `:348`, `inline_text` `:373`, `text_node` `:368` | `lower/mod.rs:1222-1240` |
| style | `style_property` `:350`, `style_engine_in_backend` `:353`, `is_setter_property` `:358`, `style_declaration` `:363` | `lower/mod.rs:47-51`, `1331-1350`, `1442+` |
| child attrs | `child_attr` `:377`, `coerce_child_attr` `:381` | `native_attrs.rs:193-228` |
| dev source locations | `source_loc` `:311`, `source_locs` `:316`, `SOURCE_LOC = "source-loc"` `:286` | `compile.rs:211-216` |
| bind sources (CX3 link/draft) | `bind_source` `:413` | `lower/mod.rs:513` `bind_via` |

Implementation: `SpecBackend` (`semantics/backend.rs`), built-ins embedded with `include_str!` (`backend.rs:8-15`) and
selected in `SpecBackend::builtin` (`backend.rs:53-62`: `gtk|qt|appkit|winui|egui`). Fragments of provider kinds:
`merge_fragment` (`backend.rs:64-75`). **A tessel spec = one more `include_str!` + one `builtin` arm** (or
`from_path`); everything else is data. `Overlay<P>` (`semantics/overlay.rs`) overrides answers in Rust.

### 1.3 Static vs dynamic writes (one-off `$n.set` vs `$.template_effect`)

- **Lowering** decides *static vs expression*: a literal attribute becomes `Op::Set { value: Json }` via
  `coerce_static` (`lower/mod.rs:1073-1078`, `1270-1277`); anything with `{}` becomes `Op::SetDynamic { value: Dynamic }`
  (`lower/mod.rs:1079-1082`). Text children fold into the kind's text prop (`text_content`, `lower/mod.rs:1282-1292`:
  pure text -> `Op::Set`, else `Op::SetDynamic` with `Dynamic::Template`).
- **Codegen** decides *reactive vs one-off* for dynamic values: `Codegen::phase` (`codegen/component.rs:600-619`):
  `SetDynamic | ChildAttr(Dynamic) | WidgetName(Dynamic) | Text(Dynamic)` go to `Phase::Update` iff
  `dynamic_reactive(value)` (`:1227-1233`), else `Phase::Init` (one-off `dynamic_call`, `:1237-1241`).
- `dynamic_reactive` -> `Rewriter::is_reactive` (`transform/mod.rs:469-514`): an identifier is "stateful" unless it
  is a function, a `Static` binding, or a never-reassigned `Normal` binding initialised with a literal; imports always
  count as stateful; plus `has_call` (`transform/mod.rs:539-601`, Svelte's `ExpressionMetadata.has_call`).
- All `Phase::Update` ops of one fragment go into **one** `$.template_effect` (`codegen/component.rs:626-665`), with call
  expressions memoized as deriveds (`memo_exprs` `:668-725`). Order rules (CF2) in `docs/notes/native-compiler/runtime-contract.md`
  lines 39-64: init (create, static sets, children, then insert into parent), update (one template effect),
  after_update (events, then bindings).
- Style-derived updates are always Update: `StyleProp | StyleSetter` (`:611`); dynamic `Classes`/`InlineStyle`/`Token`
  (`:608-610`). `update_prefix` (`:776-793`) emits `$n.capture` / `$n.assume` (spec default as last value) before.
- Element creation and static ops: `Codegen::element` `codegen/component.rs:507-582` (`$n.element|toplevel|attached`,
  `$n.source_loc` at `:522-525`, `$.derived(() => $n.classes(...))` at `:526-531`, then ops by phase, children, append).
- Reference shape (counter): `runtime-contract.md` lines 11-37.

Facts relevant for a template op: EGUI.md-era count on the 41 GTK gallery components: **84% of `$n.set` are constants**
(535 of 636), 440 `$n.append`, ~152 template/bind effects, 126 `$n.on` (`docs/notes/egui-all-rust.md` ~lines 265-280).

### 1.4 Event handlers (Listen ops, sync flags, phases)

| Source | Lowering | Emission |
|---|---|---|
| `on<event>={h}` | `listen_op` (`lower/native_attrs.rs:96-125`): `sem.event(er, attr)`; on failure strips a `capture` suffix -> `LISTEN_CAPTURE` (2); `check_phases` (`:128-147`, `listen_phase_unsupported`); `check_sync_handler` (`:150-188`, `sync_handler_arguments` / `sync_handler_return` warnings) | `Op::Listen { signal, handler, flags, sync, span }` (`ir.rs:120`) |
| `on:x` bare (FW1 forwarding) | `lower/mod.rs:984-997`, `Handler::Forward` | `function (...$$args) { $$props.onx?.apply(this, $$args) }` (`codegen/component.rs:764-766`) |
| `on:x|capture` | `lower/mod.rs:1000-1012` (`forward::capture_only`) | capture listener |
| `on:x|modifiers` | `forward.rs:122-200`: **error** `unsupported_feature` + fix-it; `passive`/`nonpassive` are accepted silently and **ignored** (`forward.rs:151`) | none |
| `{@attach listen('sig', fn, {after, capture})}` | `attach_op` (`native_attrs.rs:308-357`) resolves statically -> `Op::ListenAttach` | `$.attach(n, () => $n.listen(sig, fn, flags))` (`codegen/component.rs:931-936`) |
| TR1 runtime events `onintrostart|onintroend|onoutrostart|onoutroend` | `lower/mod.rs:960-965` (no spec lookup) | `$n.on` (runtime dispatches them itself, `hostListen` skips `RUNTIME_EVENTS`, `index.js:903-912`) |

Codegen: events are `Phase::Event` (`codegen/component.rs:612`), written after the template effect by `listen`
(`:746-772`): `$n.on(n, "signal", handler[, flags])`; `handler_memo` (`:730-743`) memoizes a handler expression with a
call (`onclick={make(x)}`) in a `$.derived`. In dev, non-function handlers go through `$n.apply` (Svelte's `$.apply`).

`sync` is *only* used for diagnostics at compile time; it is not emitted. The runtime does not know a signal is sync;
the **host** decides: the backend calls `call_sync` and the host calls `__svelters_dispatch_sync`
(`crates/svelters-host/src/core.rs:1207-1277`). egui's DOM events (`pointerdown ... focusout, wheel, pointermove`) are
all `sync: true` (`crates/svelters-egui/src/kinds/mod.rs` `dom_signals!`, ~lines 26-45) and are dispatched **one
`call_sync` per node per phase** along the path (`packages/native-runtime/src/index.js:1204-1210` comment,
`dispatchSyncTo` `:1267-1281`, result bits 1 stopPropagation / 2 preventDefault / 4 stopImmediate).

Phase flags: `LISTEN_AFTER = 1`, `LISTEN_CAPTURE = 2` (`semantics/mod.rs:173-174`; ABI `SVL_LISTEN_*`
`svelters-abi/src/lib.rs:95-99`, packed into the signal id's top 4 bits, `SVL_SIGNAL_FLAGS_SHIFT = 28`).

**Passive listeners (toolkit doc §7.1):** no IR field, no spec field. The research's "compiler marks it non-passive when
its body can reach `preventDefault`" needs: a `passive` bit on `Op::Listen` (from `|nonpassive`/body analysis), emitted
as a flag (e.g. `LISTEN_PASSIVE = 4` in `listen_ex` flags, which the ABI already carries), and a spec `phases`/
`cancelable` fact per signal.

### 1.5 Transitions / animate (TR1) and what the runtime does

- Lowering: `transition_op` (`lower/native_attrs.rs:571-619`): Svelte flags `TRANSITION_IN 1`, `OUT 2`, `GLOBAL 4`,
  duplicate/conflict/modifier checks -> `Op::Transition { flags, name, arg }`. `animate:` only on the sole element of a
  keyed each (`animate_host` `:552-564`; `lower/mod.rs:1036-1045`) -> `Op::Animate`.
- Phase: `Binding` (after_update, with bindings; `codegen/component.rs:615-617`). Emission: `$n.transition(flags, n,
  () => fn, () => params)` / `$n.animation(n, () => fn, () => params|null)` (`codegen/component.rs:957-979`).
- Runtime: `transition`/`animation` (`index.js:2849-2858`) call **Svelte's own** `$.transition`/`$.animation` on the
  runtime node. The node provides an `Element` facade (`motion.js:588-620` `defineFacade`; `proto.animate = (kf, opts)
  => new NativeAnimation(this, kf, opts)` at `motion.js:608-609`). Svelte computes keyframes in JS once per transition
  by sampling `css(t, u)`.
- `svelte/transition` and `svelte/animate` are aliased to `transition.js` / `animate.js` (Svelte's functions with
  `getComputedStyle` replaced by `computed(node)` = `host.computedStyle`/inline opacity, `getBoundingClientRect` by
  `host.bounds`, ABI v2.8).
- `NativeAnimation` (`motion.js:300+`): maps CSS declarations to **four channels** sent with `host.style`:
  `OPACITY 4`, `TRANSFORM 8` (`"tx ty sx sy ox oy"`), `CLIP 9` (`"w h"`), `INERT 10` (`motion.js:8-28`).
  - **Backend plays it natively** if `host.style(OP_ANIMATE = 11, h, json)` answers 1 (`startNative`, `motion.js:271-290`;
    payload `{id, duration, delay, frames}` at `:280`): egui does; it reports the end with `animationend`
    (dispatch exception for inert nodes at `index.js:1126-1127`). **No JS per frame.**
  - Otherwise the JS player runs on Svelte's `raf` (host `requestAnimationFrame`, `prelude.js:84-96`), one style op
    per channel per frame.
  - Unsupported transforms (rotate, skew, matrix, %) are dropped with a warning (`parseTransform`, `motion.js:110+`).
- `svelte/motion` (`spring`, `tween`) is not native: it runs on rAF in JS (toolkit doc §9.2 proposes a native
  `svelters:motion`).
- CSS transitions are engine-side: `svelters-style` `Transitions` (`crates/svelters-style/src/transition.rs:1-15`,
  `restyled` `:72`, `view` `:138`, `sample` `:146`, `tick` `:163`), sampled in Rust by the egui scene.

### 1.6 `bind:` plan choice (`semantics/observe.rs`)

`plan_bind` (`observe.rs:153-279`) asks the platform for the declared signal plan (`plan_binding`), the property and its
`observe` fact, then:

| Fact | Lowering (`BindLowering`, `observe.rs:90-112`) | Runtime |
|---|---|---|
| `exact` + `verified: delivery`, writable | `Prop { fallback: signal plan }` | `$n.bind_prop` (`bind-observe.js`): template-effect write + `observeProp` (host `observe`, ABI v2.6) |
| `exact` + delivery, read-only | `Readonly` | `$n.bind_readonly` |
| `exact` without delivery evidence | `Signal` if a plan exists, else `Prop` + `bind_observation_unverified` warning | `$n.bind` / `bind_prop` |
| `partial` | `Signal` (or `Prop`/`Readonly`) + `bind_partially_observed` | |
| `set_only` writable | error `bind_set_only` | |
| `none`/`constant` | error `bind_not_observable` | |
| no fact | `Signal` or error `bind_no_plan` | `$n.bind(node, prop, signal, arg, get, set)` (`index.js:984-1002`) |
| `|nonreactive` | `Once { readonly }` | `$n.bind_once` |
| `|commit` (CX3) | `plan_bind_commit` `observe.rs:124-150` -> `Commit { signal, arg }` | `$n.bind_commit` (`index.js:1004-1006`) |

Lowering site: `lower/mod.rs:1094-1126`; `BindVia::Link` wraps a linkable plan into `Op::BindLink` (`:1117-1122`).
One-way props that the native side also edits get `Op::Track` (`lower/mod.rs:1085-1087`; runtime `track`
`index.js:1014-1020`, passive mirror so `set` does not write the old value over the user's edit).
egui generates `observe` per prop from `PropDef.native` (`Native::Widget` -> `exact`/`delivery`, `Native::Never` ->
`set_only`/`weak`; `crates/svelters-egui/src/spec.rs:58-72`).

### 1.7 Native links (CX3) — values flowing without JS

- Spec: `bind_sources` row `{module: "svelters:app/*", export: "*", lowering: "link"}` (egui generator
  `crates/svelters-egui/src/spec.rs` `semantics()` ~`:556-563`; semantic-spec.md "bind_sources").
- Lowering: `bind:prop={store.field}` where `store` is imported from a `link` module -> `Op::BindLink { prop, obj, field,
  fallback }`; codegen `$n.bind_link(node, prop, () => obj, "field", () => { <planned binding> })`
  (`codegen/component.rs:1134+`).
- Runtime: `bind_link` (`bind-observe.js`) calls `host.link(obj().svlHandle, field, node.h, prop)`; 0 = refused ->
  fallback installs the JS binding; teardown `host.unlink(id)`.
- Host: `quickjs.rs:1305-1325` (`link(store: f64, field: String, node: f64, prop: String) -> u32`, `unlink(id)`);
  `crates/svelters-host/src/core/links.rs:1-20` (design), `op_link` `:30+`.
  - **What can be linked:** only a writable, non-proportional field of a Rust store (`#[svelters::store]`, E3 app
    object) <-> an observable prop of a native node. Not node<->node, not derived expressions, not `|commit`/
    `|nonreactive`/read-only props (`lower/mod.rs:1109`, `1117-1122`).
  - **How values flow:** both ends are observed through the refcounted `observed` table; propagation happens in
    `take_changes` at the checkpoint (`core.rs:762+`): a changed store value is written to the node with `set_prop`, a
    changed node value goes through the store's generated setter (`on_write`); pairs observed only by links are never
    delivered to JS. Latency: **one checkpoint** (not same-frame), because it rides the checkpoint's change batch.
  - For tessel: the same table could become a direct scene binding (node prop <- store field written in the UI phase),
    and could be generalised to node->node links (`bind:value={other.value}` shapes) and to "constant expressions of a
    store field" if the compiler can prove purity.

---------------------------------------------------------------------------------------------------------------------

## 2. The egui semantic spec and how a tessel spec would be produced

### 2.1 Generation chain

| Artifact | Generator | Source of truth |
|---|---|---|
| `metadata/semantics/egui.json` (562 KB, 60 kinds, 57 tags) | `crates/svelters-egui/src/spec.rs` `semantics()` `:540-576`, `semantics_text()` `:579`; checked by `crates/svelters-egui/tests/spec.rs` (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` rewrites) | `crates/svelters-egui/src/kinds/*.rs` (`KINDS` in `builtin.rs`, families per file), `kinds/docs.rs` (DOC1 a11y/states/paint), provider registry |
| provider-kind fragment (`kinds.json`) | `spec::fragment()` `:676` | `#[svelters::widget]` (RA1, `crates/svelters-macros/src/lib.rs`) via `inventory` |
| `packages/native-runtime/types/egui.d.ts`, `kinds.d.ts`, `behaviors.d.ts` | `spec.rs:913`, `:939`, `:1035` | same |
| `crates/svelters-style/src/tables.generated.rs` + `packages/style/src/props.generated.js` | `tools/metadata/style/gen_props.py` (`TOOLKITS = ["gtk","qt","appkit","winui","egui"]`), reading the `style` section of `metadata/semantics/<tk>.json` | for egui: `spec.rs` `style_json()` `:381-475` |
| GTK/Qt/AppKit/WinUI specs | `tools/metadata/semantics/gen_semantics.py` (+ `rules/*.yaml`, overrides) | ingested toolkit metadata (semantic-spec.md "Generated vs hand-written") |

So **egui (and tessel) specs are emitted by the backend binary itself from Rust tables**; the compiler embeds the JSON.
A tessel spec is produced the same way: a `svelters-tessel/src/spec.rs` over its kind registry (the research's §9.4 `Kind`
trait), plus a `tests/spec.rs` staleness check, plus adding `"tessel"` to `gen_props.py`'s `TOOLKITS` and to
`SpecBackend::builtin`.

### 2.2 Per kind (egui.json keys)

`a11y` (role, name_from, actions, variants, author_id), `abstract`, `bindings`, `category`, `chain`, `children`
(policy, default_slot, slots{many, accepts, route_by_kind, order, attrs}), `css_hidden`, `css_note`, `doc`, `interface`,
`lifetime`, `methods`, `native`, `paint`, `parts`, `present`, `props`, `provenance`, `provider_states`, `signals`,
`states`, `text_content`.

Per prop: `construct_only, default, doc, js, native, nullable, observe{class, fact, mechanism, verified}, of, readable,
type, values, writable` (`spec.rs:58-96` `prop_json`). Per signal: `args, disconnect_on_drop, doc, handler_result,
js_event, name, phases, sync, sync_required`.

**Not present:** integer member ids; whether a prop is layout / paint / compositor-property (toolkit doc §6.1 assumes
the kind's metadata says so; egui only has `PropDef.layout: bool`, `kinds/mod.rs:397-406`, used to emit style-table
setters, not exported per prop); passive/cancelable per signal; "pure/derivable natively" facts.

Top level: `backend, bind_sources, event_aliases, framework, inline_text, kinds, method_lookup, schema, source, style,
tags, theme` (theme: tokens per set with VS Code mapping, densities).

`style` section (egui): `dialect: "none"`, **`engine: "backend"`** (`spec.rs:447`), `media_features`, `properties`
(93: layout props as `via: setter` with `kinds`/`tags`/`type`/`values`/`range`, plus `ENGINE_PROPS` as `via: css`
with `decl_type`), `pseudo_classes`, `provider_states`, `setter_selectors: ["type","class","id"]`, `units {px: 1.0}`.

### 2.3 Are prop/member ids stable integers? What goes on the wire?

- **egui run time:** yes, integers, but positional: kind id = `index + 1` (providers `62 << 22 | local+1`); member id
  = `(defining kind index + 1) << 16 | (member index + 1)` (`crates/svelters-egui/src/kinds/registry.rs:41-47`,
  `:421-446`). "Stable for a given binary"; appending is safe, inserting/reordering a table row renumbers.
  Ids are not written into `egui.json`.
- **ABI:** integer `svl_id` everywhere after `resolve(SVL_KIND|SVL_PROP|SVL_SIGNAL|SVL_METHOD|SVL_SLOT|SVL_ATTR, scope,
  name)` (`svelters-abi/src/lib.rs:86-92`, `svl_backend.resolve/create/set_prop/listen/...` `:292-350`).
- **Generated JS and runtime:** **string names** (`$n.set(n3, 'label', ...)`, `host.create(kind)` in `create`
  `index.js:504-511`, `node.host.setProp(node.h, prop, value)` in `set` `index.js:617-650`).
- **Host:** accepts either. `__svelters_host` comment: "Member arguments are names or the ids the `*Id` resolvers return;
  ids skip the name lookup and string handling entirely" (`quickjs.rs:984-985`). Resolvers `kindId`, `propId`,
  `signalId`, `methodId`, `slotId` exist (`quickjs.rs:1008-1050`), `QjsConv::with_name` takes an int as a pre-resolved id
  or borrows the string (`quickjs/conv.rs:86-101`). Name cache: `CoreState.names: HashMap<(u32, svl_id), HashMap<Box<str>,
  svl_id>>` (`core.rs:325`). The runtime uses `kindId`/`methodId` only for provider methods (`index.js:386-395`,
  `1031-1070`). Measured (toolkit doc §9.1): `setPropId` 0.77 µs vs 1.43 µs by name.
- Op logs and fixtures are by name by contract: "Kinds, props, signals and slots appear by their JS-facing names, never
  by IDs" (`plans/architecture/backend-abi.md:194`).

### 2.4 Op signatures as registered in `crates/svelters-host/src/quickjs.rs`

```rust
// :990  create(kind: Value) -> f64                      (int = kind id fast path, else name)
// :1008 kindId(a: Value) / propId|signalId|methodId|slotId(kind: u32, name: Value) -> u32
// :1052 propType(kind: u32, prop: u32) -> Value|null
// :1064 insert(parent: f64, child: f64, before: Opt<Option<f64>>, slot: Opt<Option<Value>>) -> ()
// :1092 remove(parent: f64, child: f64) -> ()
// :1137 destroy(h: f64) -> ()
// :1147 setProp(h: f64, name: Value, value: Value) -> ()
// :1162 getProp(h: f64, name: Value) -> Value
// :1177 call(h: f64, method: Value, args: Value /*array*/) -> Value
// :1195 construct(kind: Value, props: Opt<Value>) -> Value
// :1210 callStatic(kind: Value, method: Value, args: Value) -> Value
// :1230 typeChain(h: f64) -> Vec<String>
// :1239 listen(h: f64, signal: Value, on: bool, flags: Opt<u32>) -> ()
// :1256 persist(key: String, value: Opt<Value>) -> Value
// :1291 observe(h: f64, prop: Value, on: bool) -> bool
// :1306 link(store: f64, field: String, node: f64, prop: String) -> u32
// :1319 unlink(id: u32) -> ()
// :1327 style(opcode: u32, h: f64, text: Value /*string|null*/) -> i32
// :1346 bounds(h: f64) -> [x, y, w, h] | null
// :1365 setChildAttr(parent: f64, child: f64, name: Value, value: Value) -> ()
// :1380 getChildAttr(parent: f64, child: f64, name: Value) -> Value
// :1107/1119/1128 rootFault / rootInfo / rootEmit (EM1)
// plus dx, assets, assetBytes, alloc; platform::install adds more (e.g. computedStyle)
```

Per-op cost path for `setProp` (`core/ops.rs:398-450`): `timed` -> `live_kind` hash lookup -> `resolve_name`
(`ops.rs:96-101`, a hash of a borrowed UTF-8 string) -> `member` descriptor cache -> `convert::encode` into an arena ->
backend `set_prop`. Entry from JS into Rust uses rquickjs `Function::new` closures (argument conversion per call).

---------------------------------------------------------------------------------------------------------------------

## 3. CSS on the egui backend (`style.engine: "backend"`)

### 3.1 Compile time

| What | Where |
|---|---|
| `<style>` rules are **not** lowered (no compile-time matcher on egui) | `lower/mod.rs:45-47`: `let styles = if sem.style_engine_in_backend() { None } else { styles };` |
| the engine's own parser + table check on the sheet, diagnostics mapped to `.svelte` spans (`style_bad_value`, `style_unknown_declaration`, `style_syntax`, `style_unsupported_at_rule`) | `lower/engine_css.rs:1-16`, `check` `:35-80` (`svelters_style::Stylesheet::parse`, `Stylist::new(Table::for_toolkit(backend)).add_sheet`) |
| scoping with Svelte's algorithm (Rust port), hash `svelte-<hash>` | `scoped_css.rs` (Q3w); loader sets `css_scope = svelte-{hash(logical id)}` (`crates/svelters-modules/src/source/transform.rs:90-99`) |
| every native element gets the scope class (not only those Svelte marks) | `lower/mod.rs:1183-1191` -> `Op::Classes`; `ScopedCss.scoped_elements` is computed but "the codegen does not use them yet" (`scoped_css.rs:33-35`) |
| static inline setter declarations (`style="gap: 8px"`) | lowered to `Op::Set` at compile time (`inline_style` `lower/mod.rs:1331-1350`, `style_ops` `:1470-1500`); CSS-only inline declarations stay text |
| `class="a {b}"`, `class:x` | `Op::Classes { base, dynamic, toggles }` -> `$n.attr(n, "class", $n.class_name(base, dyn, toggles))` (`codegen/component.rs:822-839`, static `:980-986`) |
| U4 utilities / `@apply` | `compile.rs:175`, `utilities::check` |
| DZ0 CSS source map (runs relative to `<style>` content) | `scoped_css.rs:36-60` |

The setter matcher that runs on other backends (`match_selector` `lower/mod.rs:1787`, `match_compound` `:1829`, `style_ops`
candidates `:1500+`) already **precomputes selector matching at compile time** for the "one compound selector, type/
class/id, no states" subset, with runtime conditions only for dynamic classes/ids (`CondAtom::Class/Id`). CSS.md D9
recommends retiring it and re-adding a "static template" variant only if first paint needs it
(`plans/research/CSS.md:398-410`, `:900-903`).

### 3.2 Run time

- Sheet delivery: the loader wraps each component's scoped CSS in a style module `registerStyles(id, { css, file?, map? })`
  + `import.meta.hot.accept()` (`crates/svelters-modules/src/source/mod.rs:636-647`; Vite path `packages/native-runtime/vite.js:376`, `:489`).
- `style.js`: `OP = { TARGET 0, CLASSES 1, NAME 2, INLINE 3, OPACITY 4, SHEET 5, SCHEME 6, DECLS 7 }` (`style.js:48`);
  `TARGET` answer 4 = engine (`:51-52`). Engine path `applyEngine` (`:366-387`): sends `CLASSES` (user classes + scope),
  `NAME` (`#id`), `INLINE` (raw `style=` text) **each when changed**, through `host.style(op, h, text)`.
- Batching: `attr()` (`index.js:823-833`) -> `markStyleDirty` (`style.js:462-468`) -> **`Promise.resolve().then(flushDirty)`**:
  style ops go out at the end of the job (microtask), after the structural ops; in the counter-egui op log the
  `style #n classes "..."` lines come after `call #1 present []` (`fixtures/counter-egui/expected.oplog:41-48`).
- Engine (`crates/svelters-egui/src/style.rs:1-35`): the scene is the `Element` tree; `CLASSES/NAME/INLINE` mark
  invalidation at write time; structure/attribute/state changes come from a journal observer, drained at the logic step
  (`restyle_pending`), restyle top-down; same-frame hover/press/focus states; outputs to taffy style and paint;
  precedence: a prop the runtime set (`NodeCore::explicit`, `scene/node.rs:354`) > inline > rules > native default.
- `svelters-style` (`crates/svelters-style/src/lib.rs`): `Stylist` (`cascade.rs:143`, `add_sheet` `:189`,
  `matched_rules` `:285`, `compute` `:455`; Stylo `selectors` crate with `MatchCaches`), `Invalidation` dependency maps
  (`invalidation.rs:1-40`, `class_changed` `:146`, `state_changed` `:165`, `structure_changed` `:178`; `Reach` bits
  SELF/DESCENDANTS/LATER_SIBLINGS/ANCESTORS/TREE), `Transitions` sampler (`transition.rs`), `Table` (`table.rs:59-82`:
  `Prop { name, via, target, ty, values, tags, inline_only }` — no layout/paint/compositor phase field).
- `.svb` release bundles reserve "versioned, optional sections for data added later (pre-parsed stylesheets, OW-CSS)"
  (`crates/svelters-modules/src/svb/mod.rs:1-18`): the natural home for precompiled sheets/templates.

### 3.3 What could be precomputed

- Static scope classes + static `class=` strings are compile-time constants (a template can carry them; today one
  `host.style` call per element per mount).
- With the scene and sheets known, **initial matched-rule sets for fully static subtrees** are computable at build time
  for selectors that only test type/class/id/structure inside the component (the compiler knows the tree shape of a
  fragment). States (`:hover`), `:global`, cross-component selectors and media queries must stay runtime. CSS.md
  already names this ("precompute the matches of fully static elements at build time as an initial style",
  `CSS.md:408-410`).
- Pre-parsed sheets (binary `Stylesheet`) in `.svb` sections; selector dependency maps (`Invalidation`) per sheet.

---------------------------------------------------------------------------------------------------------------------

## 4. The native runtime (`packages/native-runtime/src`)

### 4.1 Files

`index.js` (2941), `style.js` (612), `motion.js` (621), `lists.js` (831), `bindings.generated.js` (1004: `TOPLEVELS`,
bindings), `embed.js` (336, many roots), `observe.js` (199), `bind-observe.js` (127), `transition.js`, `animate.js`,
`lists-diff.js`, `core/{index,internal,svelte,tracing}.js` (Svelte adapter; the only importer of `svelte/internal/*`).

### 4.2 Per-op paths

| Op | Path | Host calls |
|---|---|---|
| `element(kind)` | `create` `index.js:504-511`: `new Native(kind, hs.create(kind), top)`; `table(hs).set(h, n)`; `scope.natives.push(n)` | 1 (`create` by name) |
| `set(node, prop, v)` | `index.js:617-650`: RESTORE; `alive`; `plainData` (deep copy of `$state` proxies); `requested` (track) compare; `node.last` compare (`Object.is` or `sameParam` deep); snippet props; `anchor`; `node.host.setProp(node.h, prop, value)` | 0 or 1 |
| `append(container, node)` | `index.js:1346-1350` -> `attach` `:245-279`: `sendSourceLoc` (dev, `setProp('source-loc')`), toplevel show policy, `anchorScope`, `nextNative` (sibling search through regions), `host.insert(parent, h, before, slot)`, `styleInserted`, `applyChildAttrs` | 1-3 |
| `on(node, signal, fn, flags)` | `index.js:914-925`: handler list per `signal\0flags`, scope registration, `host.listen` on first | 0 or 1 |
| `attr(node, 'class'|'id'|'style', v)` | `index.js:823-833` -> `markStyleDirty` (microtask) -> `applyEngine` | 0-3 at flush |
| `bind(...)` | `index.js:984-1002`: a template effect + an `on` handler moved to the front | |
| dispose | `dispose` `index.js:1417-1433`: remove top natives, dispose child scopes, `host.destroy` **every** native post-order, drop handlers | 1 per native |

Objects per native: `Native` holds ~15 Maps/Sets (`last`, `captured`, `tokens`, `handlers`, `listening`, `cattrs`,
`passive`, `slots`, ...; `index.js:85-110`). A template op that creates natives without JS `Native` objects for
untouched nodes would need `dispose` to destroy by subtree (one `destroy` of the root) instead of per native.

### 4.3 Dispatch, sync dispatch, frames, timers

- `mount` installs `__svelters_dispatch`, `__svelters_dispatch_sync`, `__svelters_gone`, `__svelters_node`
  (`index.js:2872-2880`).
- `dispatch(h, signal, args, deferred, flags)` `index.js:1123-1130` -> `dispatchTo` `:1171-1188` (passive `track`
  mirrors first, then handlers). State writes are flushed by Svelte's microtask batch at the host's job drain.
- `dispatch_sync` `index.js:1195-1203` -> `dispatchSyncTo` `:1267-1281` (`NativeEvent`, result/defaultPrevented/bits).
- `setDispatchHook` (`index.js:1137-1140`): embed.js wraps dispatches in the root's context (EM2).
- Host side: `JsEngine::dispatch` looks up `globalThis.__svelters_dispatch` by name per call (`quickjs.rs:523-546`),
  `dispatch_sync` (`:548-566`), `run_frame` (`:568`, `__svelters_internal.frame`), `dispatch_changed`
  (`:601`, `__svelters_changed([h, prop, value, ...])`).
- rAF (`crates/svelters-host/src/prelude.js:57-96`): callbacks batched per host frame; `native.frame_request()` ->
  `Core::frame_request` (`core.rs:1158-1170`) -> backend `request_frame`; `host.frame` -> `core::frame`
  (`core.rs:1175-1198`): `run_frame`, then `run_jobs`. Scripted hosts use a 16 ms virtual timer.
- Timers: `setTimeout`/`setInterval` -> `native.schedule(id, delay, repeat)` (`prelude.js:37-56`), fired by the
  checkpoint (`core.rs:1128-1141`). `performance.now = native.now()` (virtual clock in scripted mode, `prelude.js:25-29`).
- Microtasks: `queueMicrotask` -> `Promise.resolve().then` (`prelude.js:31-36`); drained only by the host's
  `run_jobs` (`quickjs.rs:633+`, `execute_pending_job` loop).

### 4.4 Where a "template instantiate" op or a per-transaction flush hook would go

| Hook | Natural place | Notes |
|---|---|---|
| `$n.template(id, container)` / `$n.instantiate(tpl, slots)` | new `$n` export next to `element` (`index.js:499`) + a new IR item (`Item::Template` or a flag on `Element` subtrees) built in `lower` from fragments whose ops are all `Op::Set`/static classes/static child attrs; codegen in `Codegen::element` (`codegen/component.rs:507-582`) | Must return the handles (or `Native`s) of nodes referenced later: dynamic writes, listeners, `bind:this`, actions, transitions, children blocks. Keep the CF2 order: static props before insert, toplevel `present` after the template effect. |
| host op | new `host.instantiate(tplId, parent, before, slot) -> [handles]` in `quickjs.rs` + new optional `svl_backend` member (ABI v2.10) or Rust-only fast path on a tessel host; template tables shipped as a module constant or an `.svb` section | The op log needs a line shape (`template #h <id> ...`) or the host expands it into the existing lines (keeps fixtures byte-identical; see §7). |
| interned ids | codegen emits ids instead of names: e.g. a per-module `const $$p = $n.ids(kind, [names])` resolved once (`host.propId`), or compile-time ids if the spec carries stable ids | Host already accepts ints (`with_name`). Op logs stay by name (log writers resolve back). |
| transaction end / flush hook | after Svelte's flush: `core/svelte.js` adapter (flush is Svelte's `flushSync`/microtask), or at the host: after `run_jobs` drains in `checkpoint` (`core.rs:1144`), `frame` (`:1188`), `call_sync` (`:1268`), `root_call` (`core.rs:1549`) | `style.js` `markStyleDirty` is today's only per-job batch (microtask); a toolkit "commit" call (`host.commit()` or a backend callback after drain) would bound one transaction = one JS task + microtasks (toolkit doc §5.4). |
| sync/urgent frame | host `call_sync` already runs JS at event time for egui DOM events | tessel's event-time dispatch maps onto `call_sync` + `run_jobs(0)`; note `call_sync` drains no jobs (`core.rs:1268`), so state writes in a sync handler render at the next checkpoint unless the handler flushes. |

---------------------------------------------------------------------------------------------------------------------

## 5. The host (`crates/svelters-host`)

### 5.1 Backend -> host callbacks (`svl_host`, `svelters-abi/src/lib.rs:261-281`; impl `crates/svelters-host/src/lib.rs:124-180`)

| Callback | Impl | Behaviour |
|---|---|---|
| `post_event(h, signal, args)` | `lib.rs:125-133` | queue + `wake`; delivered at the next checkpoint |
| `checkpoint()` | `lib.rs:135-143` -> `core::checkpoint` `core.rs:1073-1155` | order: offload settle (+drain) -> `reactive::collect` (stores) -> locale -> `deliver_changes` (one `__svelters_changed` batch, no drain) -> events (drain after each) -> due timers (drain before first, after each) -> final drain (`max_jobs_per_checkpoint`, default 10 000, `core.rs:57/103`) -> releases -> `wake` if pending |
| `call_sync(h, signal, args)` | `lib.rs:145-148` -> `core.rs:1207-1277` | JS busy -> queued deferred event + `SVL_SYNC_BUSY`; else `reactive::collect` + `deliver_changes` (fresh `$derived`), `dispatch_sync`, **`run_jobs(0)`** (no microtasks run), wake if pending |
| `frame()` | `lib.rs:150-153` -> `core.rs:1175-1198` | rAF callbacks + drain |
| `prop_changed(h, prop)` | `lib.rs:155-162` -> `core.rs:738-744` | marks an observed pair dirty (no JS, no read); first insert wakes |
| `on_destroyed(h, reason)` | `lib.rs:171-178` | `gone` delivered at the checkpoint |
| `invoke_callback` / `release_callback` | | callbacks (ABI) |

Budgets/interrupt: `turn_budget` (default 10 s, `core.rs:53-55`, `:102`), per-entry budgets set by embedders
(`core.rs:395-399`, ARCH-EMBED §4.2 `Budgets`: first-party warn 8 ms / interrupt 50 ms, mount 500 ms,
`plans/research/ARCH-EMBED.md:1041`), interrupt handler armed per entry (`quickjs.rs:219-230`, `:295-310`), FX4
`last_interrupt`, FX6 `__svelters_recover` (`index.js:1155-1165`): an aborted entry skips `finally`, so the runtime
resets `scope`, build depth, HMR context. `settle`/busy bits: ARCH-EMBED `settled()`, `BusyProbe` (`ARCH-EMBED.md:80`, `:209`, `:471-483`).

Observe (ABI v2.6): `host.observe(h, prop, on)` refcounted (`CoreState.observed`, `core.rs:337-339`); `take_changes`
reads natively, compares (`same_value` `core.rs:~1320`), proportional props carry no value; delivered as one batch
before events (REACTIVITY.md §5.2-§5.3).

### 5.2 Module loader and HMR

- Loader: `crates/svelters-modules` (`lib.rs:1-16`): `BundleProvider` (Vite `dist/app.js`), `SourceProvider` (live
  compile with `svelters-compiler`, Node ESM resolution, overlays for unsaved buffers), `SvbProvider` (`.svb` release
  bundle with optional QuickJS bytecode, Q2/Q4).
- Compile settings for every component of an app: `Compiler { sem, dev, hmr, root }`
  (`source/transform.rs:55-63`); `opts.scoped_css = self.styles()` where **`styles()` is hard-coded
  `self.sem.spec().backend == "egui"`** (`transform.rs:79-82`) — must become `style_engine_in_backend()` for tessel
  (also `vite.js:376` `toolkit === 'egui'`).
- Module cache by source hash (`source/mod.rs` `compiled`, ~`:470`); a component with a sheet imports its
  `svelters-style:` module; CSS-only edits re-run only the sheet module (`modules_at`, `source/mod.rs:600-614`).
- HMR emission (`--hmr`, dev only): `codegen/component.rs:215-221`:
  `if (import.meta.hot) { C = $n.hmr(C, id, sig); import.meta.hot.acceptExports(["default"], (m) => m && C[$n.HMR].update(m.default)); }`.
  - `$state` keying: top-level instance `$state` initialisers wrapped as `$n.hmr_keep("x", () => init, () => read)`
    (`transform/visit.rs:322-336`); signature = FNV-1a of ordered `rune:name:normalized initializer`
    (`transform/mod.rs:863-882`), `// @hmr:reset` -> `null` (`:884-891`). Modules: `$$hmr().keep` / `keep_object`
    (`transform/mod.rs:828-860`, `:986`).
  - Runtime `hmr(fn, id, sig)` (`index.js:1742-1863`): per-instance registry in the wrapper closure; new version built
    **before** the old one is closed (rollback on a throwing init, `reportHmr`); intros suppressed; instance paths
    with DZ0 call sites (`instancePath` `:1890`), `hmr_edit(file, runs)` offset translation (`:1926-1958`,
    emitted by the loader `source/mod.rs:458-466`), `parkDescendants`/`takeClaim` (H3c).
  - Host side: `JsEngine::hot_update` (`quickjs.rs:401-507`: swap budget, memory budget -> full reload fallback),
    `reload.rs:212-260`, `hot.js` (`import.meta.hot` subset).
- HMR.md §3.3 (`plans/research/HMR.md:455-506`) items still relevant: element identity prop (H4/EGUI D4),
  "setter declarations become data in HMR mode", "unchanged output -> no update".

**Constraints an optimization must respect to stay HMR-safe**

1. Component boundaries stay functions: `$n.hmr` swaps a component's whole region; a template table must be keyed per
   module version (a new version's templates must not overwrite the old version's ids while old instances still run;
   ids derived from `file + offset + content hash` work).
2. The keyed `$state` declarations and their signature come from the instance script; template hoisting must not move
   or reorder `$state` declarations or change their initializer text.
3. Old version first-version-wins: `wrapper[HMR].current` points every instance at the latest function; anything a
   compiled module registers globally (template tables, interned id tables, sheet ids) must be idempotent and
   replaceable by id (as `registerStyles` is).
4. Rollback: a partially instantiated template must be disposable through the same `dispose(scope, region)` path.
5. DZ0 offsets: `$n.source_loc(n, $$src, offset)` per element and call-site regions must survive (templates must carry
   per-node offsets in dev).
6. Release builds compile without `--hmr`; dev with it; optimizations may differ per mode but the semantics must not
   (fixtures run one mode).

---------------------------------------------------------------------------------------------------------------------

## 6. Designer and inspector integration (DZ)

- DZ0 (`plans/milestones/DZ0-designer-seams.md`): dev builds for a backend whose kinds have the `source-loc` prop emit
  `$n.source_loc(node, $$src, <element byte offset>)` per element (`codegen/component.rs:522-525`) and component call
  sites as `$n.region(container, $$src, offset)` (`:1569-1580`, `site_region`); `var $$src = "<file>"`
  (`:150-153`). Runtime sends `file#offset` + call sites (U+001F, innermost first) once at first placement
  (`index.js:1352-1390`). egui parses it (`crates/svelters-egui/src/scene/source.rs`), `SceneRef::by_source`,
  automation `source=` selectors, `hit_stack`.
- DZ0c: CSS rules to source (`ScopedCss.source_map`, `registerStyles(..., file, map)`, egui `matched_rules`,
  `Stylist::rule_sheet`); DZ0b2 `style.matched` / `style.computed` automation methods.
- DZ0d: designer overrides live in the backend (`overrides.rs`: `set_override`, `force_style`), "SV30 as scene overrides,
  not compiler hooks" — no compiled code, nothing in release.
- DZR / `crates/svelters-compiler/src/design` (`design/mod.rs:1-25`): the source model; nodes addressed by start offset,
  `Editability` (editable literal attributes, structural blocks, opaque expressions), `Design::plan(Intent) -> TextEdit`s.
- Anchor scopes (SV53): `anchor-scope` prop per keyed-each top node (`index.js:289-306`), AX1 `rowScope`.

**What a toolkit must carry:** a `source-loc` string prop on every kind (or the compiler's per-node offsets inside a
template record), the `anchor`/`anchor-scope`/author ids (EM1 a11y author ids), hit stacks, matched rules with sheet
offsets, overrides that hold back runtime writes. A template op must not drop `source_loc` for the nodes it creates.

---------------------------------------------------------------------------------------------------------------------

## 7. Diagnostics and conformance

- Codes: `crates/svelters-compiler/src/codes.rs` (`CodeInfo { code, id, origin, severity, summary, fix }`, header
  `:1-17`; table `CODES` from `:71`). A new code needs a row (the `cx1_codes` test scans sources; debug builds panic on
  an unregistered code; `UPDATE=1` regenerates `docs/site/src/content/docs/guides/diagnostics.md`). Native diagnostics
  per backend go through `Diagnostics::for_backend` / `native_error` / `native_warning`; `Unresolved::report`
  (`semantics/mod.rs:107-123`) with suggestions.
- Fix-its: `fixits.rs:1-60` (`derive`: "did you mean", remove modifier, one-way rewrite for `read_only_property`,
  `bind_set_only`, `bind_not_observable`, `nonreactive_redundant`, `bind_commit_unsupported`); each fix-it must clear
  its diagnostic (`cx1_check` tests).
- Portability mode (`compile.rs:192-196`, `lower/portability.rs`): `--backend egui,tessel` would add
  `observability_differs` warnings for free.

Conformance layers:

| Layer | What | New backend coverage |
|---|---|---|
| Svelte runtime-runes samples | `experiments/native-compiler/conformance` (`run.mjs`, `dom-host.mjs` = a `SveltersHost` over jsdom for `metadata/semantics/dom.json`), 628/774 pass (`REPORT.md`), `baseline.json` ratchet, `expected.json` rejections; CI `js.yml` `conformance` job (`docs/notes/cf1-handoff.md`) | Backend-independent: it checks the compiler + runtime semantics on the DOM test backend. Any new op shape (template, ids, transaction hook) must be implemented in `dom-host.mjs` too, and must keep the score. |
| Op-log fixtures | `fixtures/*/expected.oplog` (50 logs, byte exact, P1 TestHost (Node) and P2 log backend (Rust) must agree, format `plans/architecture/backend-abi.md:169-200`); `UPDATE=1 pnpm test` regenerates; egui twins `*-egui` replayed by `crates/svelters-egui/tests/oplog_replay.rs`; `tools/oplog-compare.mjs` compares **by state** at event boundaries | A tessel twin set (`counter-tessel`, ...) with `"toolkit": "tessel"`, replayed on the tessel backend. New op shapes change byte-exact logs: either add new line kinds (`template`, `ids`) to the format (+ TestHost, log backend, `oplog.rs` Mirror, `oplog-compare.mjs`) or have hosts expand them into today's lines (keeps every golden). `oplog-compare.mjs` state equality is the semantic gate. |
| Native behaviour references | `conformance/pins`, `conformance/golden/<platform>/<toolkit>-<version>.json`, `crates/svelters-conformance` (`Driver` trait, `state #h {...}` traces) | tessel needs its own goldens and a `Driver`. |
| Compiler fixtures | `experiments/native-compiler/fixtures/*` (`expected.<tk>.oplog`, `expected.<tk>.native.json`), `crates/svelters-compiler/tests/*` (`semantics_specs`, `cx1_*`) | add tessel expectations |
| Runtime unit tests | `packages/native-runtime/test/*.test.mjs` (138; `core-contract.test.mjs` checks `$` exports exactly match what the compiler can emit) | new `$n` primitives need tests and contract rows in `runtime-contract.md` |

---------------------------------------------------------------------------------------------------------------------

## 8. Existing plans and ideas for compiler optimizations

| Idea | Where | Status |
|---|---|---|
| **Static-template op**: "The compiler hoists each static subtree, with its constant props and slots, into a template table emitted next to the bundle, and the runtime calls `$n.template(id)` once. The egui backend instantiates it in Rust from the table, and other backends loop over it natively." | `docs/notes/egui-all-rust.md:284`; EGUI.md `:221` (better than partial AOT), G5 `:284`, `:287` ("cuts op counts and bytecode on every backend") | planned, not started |
| SV43 row templates = static-template op with column bindings (script-free row snippets rebinding without JS) | `plans/research/CADATOMIC.md:180`, LS2/LS3 in `TRACKS.md:313-320` | planned, depends on CF2 (merged) + SV42 row records |
| Precompute static elements' matched style at build time | `plans/research/CSS.md:408-410`, D9 `:900-903` | "only if first-paint measurements ask" |
| Interned ids (`setPropId` 0.77 vs 1.43 µs) | toolkit doc §9.1; host already has `*Id` resolvers (`quickjs.rs:1008-1050`) | host ready, runtime/compiler unused |
| Batching ops does **not** pay in QuickJS: Float32Array batch 1.2-1.4, JS encoder 2.1-2.3, X8 JSON 4.8-6.0 µs per op vs 1.0-1.1 µs direct | `plans/research/EGUI.md:105` | measured: keep one call per op, reduce op count instead |
| AOT paint subset (typed TS subset -> Rust for `<egui:custom paint>`), content-hash pick, differential test | `EGUI.md:115-125` (§2.3 #1) | planned (E-W2) |
| Element identity prop (H4 / EGUI D4) | `HMR.md:499-502`; `HMR.md:586-628` | partly via DZ0 offsets |
| Setter declarations as data in HMR mode | `HMR.md:483-488` | proposed |
| Unchanged compiled output -> no HMR update | `HMR.md:489` | proposed |
| Bytecode cache / `.svb` bytecode and optional sections (pre-parsed stylesheets) | `plans/milestones/Q2-typescript-bytecode.md`, `svb/mod.rs:1-18` | Q2/Q4 landed |
| Snippet hoisting to module level (`hoist_snippet`) | `codegen/component.rs:193-197`, `deviations.md:143` | landed (Svelte parity) |
| Handler memoization / template-effect memoization | `codegen/component.rs:668-743` | landed (Svelte parity) |
| Op batches for isolates (off-thread runtimes) | `CADATOMIC.md:72`, `ARCH-SCENE.md:536`, `TRACKS.md:392`, `ARCH-SECURITY.md:519` | later (ISO1) |
| `qmlsc`, "worklet", "main thread" compiler ideas | none in svelters plans (only `docs/notes/apple-gui-backend.md:71` mentions SwiftUI-ish "worklet" callbacks as a problem) | — |
| egui scene: "A runtime's host ops are not batched (one tick per op)" | `crates/svelters-egui/src/scene/journal.rs:20-21` | tessel's "one flush per transaction" (toolkit doc §9.1) is new |

---------------------------------------------------------------------------------------------------------------------

## 9. Integration points for a tessel-aware compiler (synthesis)

Ordered by payoff/cost; each names the existing seam.

1. **Spec carries what the runtime needs to be fast** (all generated from tessel's Rust kind table, `spec.rs`-style):
   - stable integer member ids (`id` per kind/prop/signal/slot) with a spec fingerprint; ids must not be positional if
     templates/bytecode are cached across binaries (registry today renumbers on insertion, `registry.rs:41-47`);
   - per prop: change class `layout | content | paint | property(compositor)` (toolkit doc §6.1) — today only
     `PropDef.layout`;
   - per signal: `cancelable`/`passive_default`, `continuous` (coalesced per frame), event-time vs deferred;
   - per kind: "templatable" (no construct-only side effects), `source-loc`, `anchor` props.
2. **Template instantiate op** (§4.4): IR pass over `Fragment`s marking maximal static subtrees (all ops `Op::Set`,
   static `Classes`, static `ChildAttr`, no blocks/components/spreads), emitted as a per-module template table +
   `$n.template(...)` returning the handles the rest of the code references. Per-node DZ0 offsets in dev. Lists
   (SV43) reuse it with column bindings.
3. **Ids on the wire**: codegen emits resolved ids (module-level `$n.ids` table or literal ids from the spec, checked
   against the host's fingerprint at load). Host fast path already exists (`with_name` int branch).
4. **Static styling folded in**: scope + static classes and statically matchable rule sets in the template record;
   use `ScopedCss.scoped_elements` to skip the scope class where no selector can match.
5. **Event lowering**: passive flag (`|nonpassive`, `preventDefault` reachability), continuous events per frame,
   one sync call per event (not per node/phase) with the propagation path in Rust.
6. **Native links generalised** (CX3 `host.link`): store<->prop today; extend to compile-time-proven node<->node and
   pure projections, moved from checkpoint to the UI phase.
7. **Native motion**: `element.animate` already goes native via `OP_ANIMATE`; add a spec fact for which channels a
   kind animates on the compositor, and a native `spring`/`tween` module (`svelte/motion` alias, as `transition.js`).
8. **Transaction boundary**: one host callback after each drain (`checkpoint`, `frame`, `call_sync`, `root_call`) or a
   runtime `$n.commit()` after Svelte's flush, so tessel can seal a scene revision (toolkit doc §5.4).

Constraints: Svelte fidelity (CF2 op order, `runtime-contract.md`; conformance 628/774 must not drop;
`core-contract.test.mjs`), byte-exact op logs (or a format extension + regenerated goldens; state-equal via
`oplog-compare.mjs`), HMR (§5.2 list), DZ0 (`source-loc`, offsets), `--strict-native`/portability, CX1 check mode
(`traverse` must stay output-free), `PlatformSemantics` object safety (one binary serves every platform), and the
CLAUDE.md rule that generated files are regenerated, never hand-merged.
