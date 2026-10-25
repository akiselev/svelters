# Compiler-driven UI optimization: prior art, and what svelters + tessel should take

Date: 2026-10-25. Research only: nothing was built, run or measured for this note. It feeds
`cadatomic/docs/research/2026-10-25-own-gui-toolkit.md` (§5 the frame, §6.1 change kinds, §7 input,
§9 svelters and QuickJS), whose question is how the svelters compiler should be co-designed with the
`tessel` toolkit so the runtime is as fast and low-latency as possible.

Local pins read: svelters `1d6656c2`, cadatomic master `0a1c345` (the toolkit doc's pin).

## Grades

Every claim carries one grade:

- **[doc]** stated in the project's own documentation, blog or release notes (URL given);
- **[src]** read in the source at the commit given;
- **[meas-other]** measured and published by someone else (their number, not ours);
- **[meas-local]** measured earlier in this repository family (file cited; not re-run here);
- **[inferred]** my reasoning from the cited facts; alternatives named where they matter.

Upstream sources read (shallow clones, deleted after reading):

| Project | Repository | Commit | Version |
|---|---|---|---|
| Svelte | sveltejs/svelte | `10fdca7` (tag `svelte@5.57.2`) | 5.57.2 |
| Solid's compiler | ryansolid/dom-expressions | `27a88be` (2026-10-07) | babel-plugin-jsx-dom-expressions 0.40.11 |
| Vue Vapor | vuejs/core branch `minor` | `0f59df2` (2026-10-23) | 3.6.0-rc.13 |
| ivi | localvoid/ivi | `12ca379` (2026-08-12) | 5.1.0 |
| Million | aidenybai/million | `c507318` (2026-05-20) | 3.1.10 |
| Marko 6 | marko-js/marko | `5277df3c` (2026-10-25) | @marko/runtime-tags 6.4.6 |
| ReactLynx | lynx-family/lynx-stack | `d8f2392` (2026-10-25) | @lynx-js/react 0.129.0 |
| Slint | slint-ui/slint | `592a25e` (2026-10-25) | 1.19.0 |
| Reanimated / Worklets | software-mansion/react-native-reanimated | `1735e03` | react-native-worklets 0.14.0-main |

Sibling notes this one does not repeat (read them for the runtime side):
`cadatomic/docs/research/notes/2026-10-25-own-gui-toolkit/js-runtimes.md` (RN Fabric, Reanimated
runtimes, Lynx dual threads, Valdi's commit protocol, QuickJS-NG costs, Svelte 5 scheduling) and
`cadatomic/docs/research/2026-10-22-slint-lessons.md` (the Slint compiler pipeline and live preview).

---

## 1. Baseline: what svelters emits today, and what it costs

### 1.1 Output shape [src]

`svelters/docs/notes/native-compiler/runtime-contract.md` ("Shape of generated code", CF2 order):
each fragment is built by **one host call per op**. A component's init creates each element
(`$n.element(kind)`), writes its static props (`$n.set(node, 'name', value)`), inserts children
before parents, then one `$.template_effect` holds every reactive write of the fragment, then
listeners (`$n.on`) and bindings. Prop names are strings.

The counter on egui (`fixtures/counter-egui/expected.oplog`, 9 native nodes) builds with 49 ops:
2 style-sheet ops, 9 `create`, 8 `insert`, 16 `set` (10 static, 3 first reactive text writes,
3 binding first writes), 1 `listen`, 3 `observe`, 1 `present` and 9 `style #n classes` writes.
**36 of them are fully static** (9 create, 8 insert, 10 static sets, 9 class writes whose class
strings are compile-time constants): they never depend on state. [src, counted]

Each native node also gets a JS `Native` object (`packages/native-runtime/src/index.js:84–108`)
that allocates **eight `Map`s, one `Set` and one plain object** at construction (`last`, `captured`,
`tokens`, `handlers`, `listening`, `cattrs`, `passive`, `slots`, `attrs`), whether the node is ever
touched again or not. [src]

`$n.set` (`index.js:617–`) runs, per write: a `RESTORE` check, `alive()`, `plainData(value)`
(proxy snapshot), a `requested` map probe, a `last` map probe and set, a `typeof value ===
'function'` check, then the host call. [src]

### 1.2 Cost model [meas-local]

From `svelters/docs/notes/js-native-serdes.md` (§0, §4.3; 2026-10-17, rquickjs 0.14, a loaded
WSL PC) and `svelters/docs/notes/egui-js-widgets.md` via `js-runtimes.md` §6.2:

- one JS→native host call ≈ 0.8–1.1 µs; the same op issued from Rust ≈ 6 ns;
- `setProp(h, 'spacing', i)` 1.43 µs against `setPropId(h, id, i)` 0.77 µs: **passing names is the
  largest avoidable cost**;
- batching into a typed array is *slower* in QuickJS (1.53 µs vs 0.77 µs per int);
- a JS→JS method call costs more than a JS→native call;
- callback registration ≈ 2 µs (persistent root plus table insert);
- reading 4 fields through a `$state` proxy adds ≈ 2.5 µs;
- Rust→JS call 0.14 µs; passing an object built in Rust adds 2–3 µs.

**Implication [inferred].** Today a fragment's construction cost is roughly
`(creates + inserts + static sets) × ~1 µs` for the crossings, plus the interpreted JS of each
`$n.*` wrapper and the ~11 JS allocations per node (GC pressure: QuickJS-NG's cycle collector walks
every live object, `js-runtimes.md` §6.3). The counter's 36 static ops are ≈ 30–40 µs of crossings
alone; a 200-row list of 10-node rows at ~3 static ops per node is ≈ 6 ms of crossings before any reactive work. The wins below come
from cutting **crossings per node**, **JS objects per node** and **interpreted work per write**,
in that order. Batching ops into buffers is not one of them (measured above).

---

## 2. Template instantiation

### 2.1 Svelte 5.57.2 [src]

- The client compiler hoists each fragment's static markup into a module-level template factory:
  `transform-template/index.js` emits `$.from_html(\`...\`, flags)` (or `$.from_tree(structure)`
  with `fragments: 'tree'`, since 5.33, for CSP setups; `compiler/types/index.d.ts:130–138`).
  Identical templates in one module are deduplicated by a key (`get_template_key`, namespace +
  flags + raw text), except in dev.
- `internal/client/dom/template.js` builds the fragment **once, lazily**, then `cloneNode(true)`s
  it per instance (`from_html` :53, `from_tree` :219). Dynamic nodes are reached by
  `$.child`/`$.sibling(node, n)` navigation.
- Static attribute values are folded into the template; the compiler's `scope.evaluate()`
  (`phases/scope.js:197`, class `Evaluation`: `is_known`, `is_defined`, `is_string`, …) folds
  expressions with one known value into static output (`visitors/shared/utils.js:139–169`).
- Reactive writes go into one `$.template_effect(fn, sync, async, blockers)`
  (`reactivity/effects.js:390`), a `RENDER_EFFECT` whose callback rewrites every dynamic value of
  the fragment; setters skip unchanged values.

**Transfer [inferred].** Svelte already treats "static subtree, cloned once per instance, then
dynamic writes" as its semantics. A native template op is the same contract with the clone done
natively: it changes no reactive behaviour, only how the static part reaches the host.

### 2.2 Solid / dom-expressions 0.40.11 [src]

- DOM target: `_$template(\`<div id=main>…\`)` hoisted per template, `_tmpl$()` clones, dynamic
  nodes found by `firstChild`/`nextSibling` chains (`test/__dom_fixtures__/attributeExpressions/output.js`).
- All dynamic attributes of one template go into **one** `effect(_p$ => { … }, {e: undefined, …})`
  with **the previous-value comparison inlined in the generated code**:
  `_v$4 !== _p$.e && _$setAttribute(_el$27, "min", (_p$.e = _v$4))`
  (`src/universal/template.js` `wrapDynamics`, `src/dom/…`). No wrapper call happens for an
  unchanged value.
- **Universal (custom-renderer) target has no template cloning**: it emits one
  `createElement`/`insertNode`/`setProp` call per node and prop
  (`test/__universal_fixtures__/simpleElements/output.js`), exactly svelters' shape today.
  Template cloning exists only where the host offers a native bulk clone (`cloneNode`).

**Transfer [inferred].** (1) A custom renderer only gets template-clone economics if the host
provides the bulk op; tessel is the host, so it can. (2) Solid's inlined compare is worth copying:
in QuickJS a JS→JS wrapper call is itself more expensive than a native call (§1.2), so
`v !== p.a && host.setPropId(h, ID, (p.a = v))` in generated code beats `$n.set(node, 'prop', v)`
for the common case (see §2.9 for which writes qualify).

### 2.3 Vue Vapor 3.6.0-rc.13 [src, doc]

- `compiler-vapor` emits `const t0 = _template("<div><p> </p> <p> ", 1)` per static shape, then
  `n3 = t0()`, `_child`, `_next(n, true)`, `_nthChild(n, 2)`, `_txt(n)` navigation, and one
  `_renderEffect(() => { _setText(x0, …); _setText(n1, …) })` per block with shared reads hoisted
  (`const _msg = _ctx.msg`) (`__tests__/transforms/__snapshots__/transformChildren.spec.ts.snap`,
  "children & sibling references", "efficient traversal").
- The compiler optimizes **navigation** ("efficient find" uses `nthChild`; shared parent paths are
  reused in a `let p0` cursor), because each step is a DOM call.
- Release notes (3.6.0-rc.1): Vapor "has demonstrated the same level of performance as Solid and
  Svelte 5" in unnamed third-party benchmarks; opt-in per component; no Options API, no `v-memo`;
  VDOM interop through a plugin, "avoid mixed nesting" [doc]
  https://newreleases.io/project/github/vuejs/core/release/v3.6.0-rc.1 . A third-party Mount
  N=5000 test reports −31% to −42% scripting time vs VDOM Vue [meas-other, low confidence]
  https://ithelp.ithome.com.tw/articles/10412801 .

**Transfer [inferred].** With our own handles we can make navigation free: if a template instance
allocates its nodes' handles contiguously, node *k* is `base + k` and JS needs no navigation calls
and no array result at all. That is strictly better than any DOM framework can do.

### 2.4 ivi 5.1.0 [src]

- Templates compile to an HTML string plus three small-integer opcode arrays: **state opcodes**
  (DFS walk saving node references), **prop opcodes** (`SetNode`, `Common`, `Attribute` with an
  expression index and a data index into a **deduplicated shared data array**) and **child
  opcodes** (`packages/ivi/src/template/compiler.ts:116–139, 300–354`;
  `docs/internals/template-compiler.md`). Flags pack slot counts
  (`TemplateFlags.ChildrenSizeShift`).
- Invariants remove opcodes: a dynamic child at the end of the root needs no traversal
  (template-compiler.md "OpCodes").
- The docs consider and reject encoding opcodes as strings ("probably not worth it") and propose
  sorting the shared data by frequency so common keys fit in 4 bits.

**Transfer.** ivi is the clearest prior art for a *binary* template: SMI opcode arrays + a shared,
deduplicated key table. For us the opcode stream is read by Rust, not JS, so its encoding cost is
paid once per module load, not per instance.

### 2.5 Marko 6 (runtime-tags 6.4.6) [src, doc]

- The compiler emits a template HTML plus a **walk string**: each character is a walk code
  (`Get` 32, `Replace` 37, `BeginChild` 47, `Next` 67–91, `Over` 97–106, `Out` 107–116,
  `Multiplier` 117–126; `common/constants/walk-code.ts`). `dom/walker.ts` interprets it with
  `charCodeAt`, storing node references into a scope array ("Cloned templates are small, where a
  TreeWalker's per-step cost dominates").
- The compiler lowers `.marko` into a dependency graph that becomes either streaming HTML with
  resume state or fine-grained DOM code; Marko 6 became npm `latest` in 2026 [doc]
  https://markojs.com/docs/newsletter/june-2026 . No independent benchmark found.
- Resumability (serialize only the closures handlers need) has no use in a desktop app with no
  server render [inferred]; its analysis of *what a handler captures* is the relevant part (§3, §5).

### 2.6 Million 3.1.10 [src, doc]

- `block()` turns a component into a static template plus an `edits` list of **holes** (paths to
  dynamic attributes, children, events, style); updates compare hole values, not trees
  (`packages/million/types.ts`, `block.ts:51–128`).
- Claims "over 70% faster than React" on synthetic benchmarks (creator's talk) [meas-other,
  synthetic] https://gitnation.com/contents/virtual-dom-back-in-block .

**Transfer.** Low: Svelte has no VDOM to replace. The hole list is the same structure as ivi's prop
opcodes and Lynx's attribute slots.

### 2.7 ReactLynx Snapshot and Element Template (0.129.0) [src, doc]

- **Snapshot** (default backend): the SWC plugin `swc_plugin_snapshot` turns each JSX tree into
  `createSnapshot(id, create, update[], …)` where `create` still issues one Element PAPI call per
  node (`__CreateView`, `__SetAttribute`, `__AppendElement`) on the main-thread runtime and returns
  the element array; `update[i]` handles dynamic value *i*
  (`transform/crates/swc_plugin_snapshot/tests/__swc_snapshots__/lib.rs/*.js`).
- **Element Template (ET)**, new in 2026, behind `experimental_useElementTemplate` (default false)
  [doc] https://lynxjs.org/next/api/rspeedy/react-rsbuild-plugin.pluginreactlynxoptions.experimental_useelementtemplate :
  the plugin `swc_plugin_element_template` serializes each static tree to a **Template Definition**
  (JSON in the bundle asset, not JS code), keyed by a content hash (`_et_ea6d5e931fea`), with
  `{"kind":"static", key, value}` attributes, `{"kind":"slot", key, attrSlotIndex}` dynamic
  attributes and `{"kind":"childSlot", elementSlotIndex}` child holes
  (`tests/__combined_snapshots__/should_handle_id_attributes.snap`). The engine instantiates it:
  `__CreateElementTemplate(templateKey, bundleUrl, attributeSlots[], childSlots, uid)` returns one
  handle; updates are **slot-addressed**: `__SetAttributeOfElementTemplate(handle, slotIndex,
  value)`, `__InsertNodeToElementTemplate(handle, childSlot, node, ref)`
  (`packages/react/runtime/src/element-template/types.d.ts:40–84`). On the web platform the
  definitions are decoded in a worker and assembled by Wasm `leo_asm` opcodes
  (`web-platform/web-core/AGENTS.md`).
- Lifecycle lessons in the changelog [doc] (`packages/react/CHANGELOG.md`): 0.126.x "Run a GC
  pass after `reloadTemplate` … under reference counting the replaced tree is not freed on its
  own"; 0.129.0 "Drop stale cross-thread function calls … after `reloadTemplate`"; benchmark
  README notes ET constructor trace events carry a provisional id `-1`, so leak pairing does not
  yet work for ET.
- No published Snapshot-vs-ET numbers: CI runs both on CodSpeed (`benchmark/react/README.md`, cases
  `et-007-four-layer-views-hydrate`, `et-018-use-state-full-attribute-update`) but the README warns
  that "equal workload does not imply that individual Snapshot and ET spans measure equivalent work".

**Transfer [inferred].** Lynx moved, in production, from "compiler emits N PAPI calls" (our today)
to "compiler emits a definition, engine instantiates in one call, updates by slot index" (what we
want). Their engine API shape is directly reusable. Their trouble spots are ours to avoid: template
lifetime across reloads, and object-lifetime tracing with provisional ids.

### 2.8 React Compiler 1.0 [doc]

Lowers to a CFG-based HIR and infers data-flow and mutability to memoize values and JSX; its
validation passes encode the Rules of React and report violations. Meta Quest Store: "initial loads
and cross-page navigations improve by up to 12%, while certain interactions are more than 2.5×
faster. Memory usage stays neutral" [meas-other] https://react.dev/blog/2025/10/07/react-compiler-1 .

**Transfer.** Not its output (Svelte is already fine-grained) but its posture: precise data-flow
analysis that *bails out* where it does not understand the code, plus diagnostics that teach the
pattern it can optimize. Our handler/binding analyses (§3, §5) should work the same way.

### 2.9 Proposal: the native template (`tessel`) [inferred]

**What the compiler emits.** Per module, a template table (one entry per static fragment shape,
deduplicated by content hash as Svelte and Lynx do). Each entry:

```text
Template {
  id: u32 (bundle-local) + hash: u64 (stable across builds, for HMR and caches)
  nodes: [ { kind: KindId, parent: u16, slot: SlotId?, static_props: [(PropId, TypedValue)],
             classes: StyleSetId?, flags: EXPOSED | LISTENS | LAYOUT_ONLY } ]   // DFS order
  attr_slots:  [ (node: u16, prop: PropId, class: Layout|Paint|Property) ]   // dynamic props
  child_slots: [ (node: u16, slot: SlotId) ]                                  // {#if}/{#each}/components
  listeners:   [ (node: u16, signal: SignalId, flags) ]                       // registered natively at instantiation
}
```

`KindId`, `PropId`, `SignalId`, `SlotId` are integers from the semantic spec (§7). Values are typed
by the spec, so the host never parses strings. The table lives in a bundle sidecar section read by
the Rust module loader (QUICKJS-RUNTIME Q1/Q5), as Lynx keeps definitions in the asset, not in JS.

**What the generated JS looks like.**

```js
const b = $n.instantiate(T3);          // one crossing: all nodes created, inserted, static props set,
                                       // static classes applied, listeners registered
// node k is b + k: no navigation calls, no array result
$.template_effect(() => {              // unchanged Svelte semantics
  const v0 = `Count: ${$.get(count) ?? ''}`;
  v0 !== p.a && $n.setp(b + 2, P_TEXT, (p.a = v0));   // inlined compare, interned id
});
$n.handler(b + 3, S_CLICKED, () => $.update(count)); // JS-side table only: the host already listens
```

**Runtime contract changes.**
- A template instance is one ownership unit: teardown is one `host.destroy(root)` (native
  post-order inside Rust) instead of `remove` + `destroy` per node from JS
  (runtime-contract.md "Ownership and teardown").
- JS `Native` objects are created **lazily**, only for exposed nodes (`bind:this`, actions,
  attachments, nodes with handlers, dynamic class/style, `track`ed props). Static nodes have no JS
  object. Independently of templates, the eight `Map`s of `Native` should be created on first use.
- First reactive writes stay as ordinary writes after instantiation (the template effect's first
  run), which keeps CF2 op order for everything observable to JS.

**Expected gain [inferred, to be measured in M1].** Construction crossings drop from
`creates + inserts + static sets + listens` to 1 per fragment, plus the Rust-side cost of creating
nodes (allocation into the arena; estimated sub-µs per node, not measured). For the counter that is
~37 crossings (~30–40 µs; 36 static ops plus the listen) → 1, and 9 JS `Native` objects (~100 JS allocations) → ~4. For 200 list rows
of 10 nodes: ~6 ms → well under 1 ms, and a much smaller GC heap. Updates gain ~0.6 µs per write
from ids (measured name-vs-id delta) plus the avoided wrapper call for unchanged values.

**Svelte-fidelity risk: low.** Svelte's own output already clones static templates; reactive
semantics, effect order and the template effect are untouched. Points to watch: (1) a kind whose
construction emits native signals would now emit them inside one host call (tessel kinds can be
made silent at construction); (2) `expected.oplog` fixtures change shape (a `template` op), so the
op-level tests need a "flattened" view; (3) the conformance backend (DOM) keeps today's op path:
templates are a backend capability (`"templates": true` in the spec), so conformance is unaffected.

---

## 3. Moving bindings and handlers off JS

### 3.1 ReactLynx main-thread scripts and worklets [src, doc]

- A function whose body starts with `'main thread'` is extracted by `swc_plugin_worklet`: the
  background bundle keeps a stub `{ _c: { captured… }, _wkltId: "a77b:test:1" }`; the main-thread
  (Lepus) bundle gets `registerWorklet("main-thread", id, function () { let {…} = this["_c"]; … })`
  (`swc_plugin_worklet/tests/__swc_snapshots__/*/should_capture_env_{js,lepus}.js`). Captured
  objects go through `captureMainThreadObject(x) ?? { field projection }`, i.e. the compiler
  narrows captures to the fields used.
- Docs: captures are JSON-serialized, read-only inside the function, and refreshed only when the
  owning component re-renders; main-thread handlers bind with `main-thread:bindscroll` /
  `main-thread:bindtap` and mutate `MainThread.Element`s directly (`setStyleProperty`) [doc]
  https://lynxjs.org/react/main-thread-script.html .

### 3.2 Reanimated / react-native-worklets [src, doc]

- The Babel plugin workletizes functions with `'worklet'` **and automatically** the callbacks of a
  fixed list of APIs: `useAnimatedStyle`, `useAnimatedProps`, `useDerivedValue`,
  `useAnimatedScrollHandler`, `useFrameCallback`, `withTiming/withSpring/withDecay/withRepeat`
  callbacks, `runOnUI*`, plus Gesture Handler builder methods
  (`packages/react-native-worklets/plugin/src/autoworkletization.ts:15–60`,
  `gestureHandlerAutoworkletization.ts`). `closure.ts` collects captured identifiers and module
  bindings to import ("import forwarding").
- Reanimated 4 (stable 2025-07-23) made **CSS-style declarative animations and transitions** the
  headline API, because "CSS animations are easier to optimize: Reanimated can see which
  components and attributes are animating" [doc]
  https://swmansion.com/blog/reanimated-4-stable-release-the-future-of-react-native-animations-ba68210c3713 .
- The dual-write-path problems (native fast path vs the React commit) are in `js-runtimes.md` §2.

### 3.3 Qt Quick Compiler (qmlsc / qmltc) [doc]

- qmlcachegen/qmlsc emit per document a compilation unit with bytecode for all bindings and
  functions, and **C++ only for those the compiler fully understands**; the rest run as bytecode
  (interpreted or JIT) [doc] https://doc.qt.io/qt-6/qtqml-qtquick-compiler-tech.html ,
  https://doc.qt.io/qt-6/qtqml-qml-script-compiler.html ("generates C++ only for functions that can
  be exhaustively analyzed").
- Direct mode calls C++ methods through included headers; static mode compiles more by assuming no
  shadowed properties, and "generates invalid code if any properties are shadowed" — Qt's own
  author calls it "a slight deviation from the usual QML semantics" [doc]
  https://www.qt.io/blog/the-numbers-performance-benefits-of-the-new-qt-quick-compiler (Qt 6.3,
  2022-01-18; numbers only as charts: "substantial speedup when compiled to C++", more in direct
  mode). QProperty→QProperty bindings are the easiest case (only the read is needed).
- qmltc compiles QML *types* to C++ classes (no `QQmlComponent` instantiation overhead) and refuses
  documents with unsupported features.

**Transfer.** The pattern to copy is "compile what you fully understand, keep the interpreter for
the rest, never change semantics silently". Qt's static-mode deviation is the counter-example:
optimizations that are only correct under an extra assumption must be opt-in and checked.

### 3.4 Slint 1.19.0 [src]

- The compiler lowers the whole binding graph: `passes/binding_analysis.rs` (loops, constness),
  `const_propagation.rs` (fold constants, re-promote bindings to constant after folding),
  `remove_aliases.rs` (two-way bindings collapse to one property), `inlining.rs`,
  `deduplicate_property_read.rs`, `remove_unused_properties.rs`,
  `optimize_useless_rectangles.rs` ("Rectangles which do not draw anything and have no x or y …
  we can just remove them"), `llr/optim_passes/inline_expressions.rs`.
- Details and live preview: `2026-10-22-slint-lessons.md` §1–2, §7.

**Transfer [inferred].** Slint can do all this because *every* binding is in its language. Svelte
bindings are arbitrary JS with `$state` proxies, so the analog is the Qt subset rule: a binding
qualifies for native lowering only if its expression is in a small pure subset (§3.8).

### 3.5 Jetpack Compose: phase-scoped reads [doc]

"By switching to the lambda version of the modifier, you can make sure the function reads the
scroll state in the layout phase … Compose can skip the composition phase entirely"; `drawBehind {}`
"skip[s] the composition and layout phases entirely"; "You should be suspicious if you are causing
recomposition just to re-layout or redraw a Composable"
https://developer.android.com/develop/ui/compose/performance/bestpractices .

**Transfer.** The single most transferable idea in this section: **where a value is read decides
which phases re-run**. For us: a binding whose inputs are native-owned (scroll offset, pointer,
drag delta, hover) and whose output is a property-class prop (transform, opacity, clip) should be
evaluated in the compositor/property phase, never in JS.

### 3.6 Makepad, Flutter, Valdi [doc]

- Makepad `live_design!` registers DSL blocks; the live system matches DSL definitions to Rust
  structs and updates them at runtime from Makepad Studio [doc, community tutorial]
  https://publish.obsidian.md/makepad-docs/Tutorials/Image+Viewer/2+-+Creating+an+Empty+Window .
  Internals not documented.
- Flutter `const` widgets: canonicalized instances let `Element` skip updating a subtree whose
  widget is `identical` [doc] https://api.flutter.dev/flutter/widgets/Element/rebuild.html . No
  measured gain published. Svelte never re-runs static parts at all, so nothing to take.
- Valdi compiles TSX to native views and generates Kotlin/ObjC/Swift bindings from TS interfaces
  [doc] https://github.com/Snapchat/Valdi ; its runtime protocol is in `js-runtimes.md` §4.

### 3.7 The web's answer: declarative, not scripted

Chrome's Animation Worklet never shipped beyond experiments; scroll-driven animations (declarative)
run off the main thread (`js-runtimes.md` §2, citing
https://developer.chrome.com/docs/css-ui/scroll-driven-animations ). Reanimated 4 went the same way
(§3.2). **Evidence across three ecosystems points to declarative native expressions over a second
scripting runtime** [inferred].

### 3.8 Proposal: native bindings compiled from a pure subset [inferred]

Classes of bindings/handlers the compiler can move off JS, in order of value:

| Class | Example | Native form | Condition |
|---|---|---|---|
| A. Pseudo-state styling | `onpointerenter={() => hot = true}` / `leave` + `class:hot={hot}` | the class is toggled by native state, or rewritten to `:hover` | `hot` is component-local, written only by those handlers, read only by `class:`/`style:` of nodes in this component |
| B. Native-owned input → property prop | `bind:scrollTop={y}` + `style:opacity={1 - y / 200}` | expression evaluated in the property phase from the scroll offset | expression in the pure subset; output prop is property-class |
| C. Pointer/drag → transform | `onpointermove={(e) => x = e.clientX - x0}` + `style:transform={\`translate(${x}px)\`}` | drag binding: native updates transform per sample; JS gets the semantic event | handler body is a pure assignment from event fields and constants |
| D. Motion | `new Spring(...)` whose `.current` feeds only `style:` | native spring on the compositor (`svelters:motion`) | `.current` read only in property-class outputs |

**The pure subset**: number/string/bool literals, arithmetic, comparisons, ternary,
template literals, `Math.min/max/abs/round/floor/ceil/sqrt`, `$state` reads (passed to the native
expression as parameters, re-sent when JS changes them), event fields from the spec's signal args,
and native-owned props declared in the spec. No calls to user functions, no proxies, no
allocation. Qt's rule applies: compile only what is fully understood, keep JS for the rest.

**Fidelity rule: JS stays authoritative.** The compiler emits the native expression *in addition
to* the unchanged JS. Native applies it early (in the compositor or at event time); the JS effect
still runs, computes the same value and its write is a no-op. If they disagree, JS wins on the next
commit (Chrome's main/impl reconciliation, `own-gui-toolkit.md` §5.4). This keeps Svelte semantics
exactly (`$state` still updated, `$effect`s still fire, `$inspect` still sees values) and makes the
optimization purely a latency improvement. Two technical requirements: values must compare at the
parsed level, not as strings (or Rust must format numbers exactly as ECMAScript `Number::toString`),
and the native owner must mark the prop "native-driven" so a stale JS write during a drag is
rejected by sequence number (§5.4 of the toolkit doc).

**Expected gain.** Latency, not throughput: a scroll- or pointer-linked visual follows input in the
compositor frame (no JS, no UI phases), and keeps moving when the UI thread is busy. For today's
cadatomic sources the immediate payoff is small: 26 `.svelte` files contain 3 `style:` directives,
34 `class:` directives, no pointer-move handlers and 4 `in:` transitions (counted at `0a1c345`);
splitters, docking and scrolling are already native kinds. It matters for future canvases, custom
sliders and drag-heavy panels.

**Not recommended now: a second JS runtime for worklets** (Lynx MTS, Reanimated UI runtime). Its
costs are documented: serialized, read-only, stale captures (Lynx), two write paths (Reanimated),
a second heap. Revisit only if the pure subset proves too small in practice.

---

## 4. Compile-time CSS

### 4.1 What svelters does today [src]

- `scoped_css.rs`: a Rust port of Svelte's CSS pipeline, "scoped byte-identically to svelte@5.57.2",
  with `css_unused_selector` warnings, i.e. Svelte's per-selector candidate analysis already exists
  in the compiler.
- `style.rs`: for backends with setter declarations, the compiler matches rules at compile time and
  lowers them to `$n.set` / `$n.child_attr` (ST1).
- `lower/engine_css.rs` (CS5g): for a backend that runs the CSS engine itself (egui), rules are
  **not** lowered; the compiler only runs the engine's parser and table check for diagnostics. At
  run time the sheet text is sent to the host (`style sheet "<file>" "<css text>"` in
  `expected.oplog`) and parsed, matched and cascaded by `svelters-style` (Stylo `selectors`,
  `Invalidation` maps in `invalidation.rs`).
- The semantic spec has no per-prop invalidation class yet: the `egui.json` prop records carry
  `type`, `observe`, `default`, … but nothing saying layout/paint/property [src, counted keys].

### 4.2 Prior art [doc]

- Svelte: a hash class (`svelte-xyz`) per component; unused selectors pruned at compile time; only
  attributes on plain elements are traced, so classes added by children or at run time need
  `:global` [doc] https://svelte.dev/docs/svelte/scoped-styles , issue
  https://github.com/sveltejs/svelte/issues/5804 .
- StyleX: a Babel plugin extracts `stylex.create()` into atomic classes (one declaration each,
  shared across the codebase); "Atomic classes reduced CSS size by 80%"; values unknown at build
  time become CSS variables set inline at run time [doc, meas-other]
  https://engineering.fb.com/2025/11/11/web/stylex-a-styling-library-for-css-at-scale/ .
- Browsers: only `transform` and `opacity` are compositor-only in the classic guidance [doc]
  https://web.dev/articles/stick-to-compositor-only-properties-and-manage-layer-count .
- (vanilla-extract, Lightning CSS: not researched for this note.)

### 4.3 Proposal [inferred]

1. **Precompiled sheets.** The compiler already parses each sheet with the engine; emit the parsed,
   interned form (rules, selector components, typed declarations with interned property ids) into
   the bundle sidecar instead of CSS text. Saves parse time at startup and on every HMR CSS update.
   Gain small (Rust CSS parsing is fast), complexity low, fidelity risk none (same engine).
2. **Interned property sets (StyleX-style).** Deduplicate declaration blocks into `StyleSetId`s;
   resolve **static utility classes** (`class="flex gap-2 p-4"`, svelters' U0/U1 utility table)
   into one property set at compile time, keeping state variants (`hover:`) and theme-dependent
   values as rules/variables. Templates reference `StyleSetId`s directly.
3. **Rule candidate hints.** Svelte's pruning analysis knows, for each scoped selector, which
   template elements it can match. Emit per template node the scoped rules it may match, so the
   engine skips selector matching against this component's own sheet for that node. Global sheets
   and `:global` selectors still match at run time. As reliable as Svelte's pruning, which the
   compiler already ports byte-identically.
4. **Invalidation class per prop and per class toggle.** Add `effect: layout | paint | property`
   to every prop in the spec and every property in `metadata/style/*.yaml`. The compiler tags each
   `attr_slot` (§2.9) and each `class:` toggle with the union class of the scoped rules it can flip,
   so tessel knows at write time that a class toggle is paint-only and skips layout. The runtime
   could derive this itself from the rule diff; compile-time tagging just removes that work from
   the hot path (toolkit doc §6.1 already relies on the classification).

Full compile-time cascade resolution is **not** possible in general: ancestors across component
boundaries, global sheets, pseudo-states, media queries and runtime classes all feed the cascade.

---

## 5. Static analysis for the event system

### 5.1 Prior art

- Svelte 5.57.2 marks listeners passive **by name only**: `PASSIVE_EVENTS = ['touchstart',
  'touchmove']` (`src/utils.js:253–268`), applied in `visitors/shared/events.js:39`. No handler body
  analysis. [src]
- No production compiler found that infers passivity. The only proposal: a 2015 WHATWG thread
  suggesting that a handler with no parameters cannot call `preventDefault`, treating ambiguous
  cases conservatively [doc]
  https://lists.w3.org/Archives/Public/public-whatwg-archive/2015Jul/0059.html . Linters instead
  require explicit `passive: true` (eslint-plugin-github `require-passive-events`) [doc].
- Field evidence that most wheel/touch listeners never cancel: `js-runtimes.md` / toolkit doc §7.1.

### 5.2 Proposal [inferred]

1. **Passive inference.** A listener is passive when its handler (a) takes no parameter, or (b)
   never lets the event value escape: the parameter is used only for field reads, and no call
   receives it except known-pure helpers. Anything else is non-passive. Emit the flag in the
   template's listener table. Only non-passive listeners on scroll/drag paths create
   "wait-for-JS" regions; the inspector shows them. Fidelity risk: none, since a passive listener
   that tries `preventDefault` is impossible by construction (the analysis is conservative).
2. **Handler write sets and the dependency graph.** For each handler, the set of `$state` it may
   write (direct assignments; unknown calls → "any"), and for each state, the template slots and
   blocks that read it. Uses: the native-binding classifier (§3.8 classes A and C), an "affects
   structure?" bit (does any `{#if}`/`{#each}` read the write set: if not, the frame needs no
   structural work), and inspector tooling. Gain mostly enabling; complexity medium.
3. **Native listener registration in templates** (§2.9): saves one crossing per listener at
   construction (`listen`), plus the ≈ 2 µs callback-registration cost measured for per-item
   callbacks (§1.2) when the JS side keeps one handler table per component instead of per node.
4. **Compile-time accessibility checks.** The egui spec already carries per-kind `a11y`
   (`role`, `name_from`, `actions`), but svelters reports no `a11y_*` diagnostics. Port the
   spirit of Svelte's DOM a11y warnings to the native vocabulary: an icon-only button with no
   `accessible-name`, a `canvas` with listeners but no role, a clickable `box` (handler on a kind
   without a focusable role). Cheap, no runtime cost, no fidelity issue (warnings only).

---

## 6. Native animations from the compiler

- Svelte 5.57.2 samples a `css` transition at `n = Math.ceil(duration / (1000 / 60))` points and
  calls `element.animate(keyframes, { duration, fill: 'forwards' })`; only `tick` runs JS per frame
  (`internal/client/dom/elements/transitions.js:382–480`). `animate:flip` measures
  `getBoundingClientRect()` before and after a keyed reorder and plays a `transform` keyframe
  animation (`animation()` :83–135, `animate/index.js` `flip`). [src]
- svelters already implements this natively: `packages/native-runtime/src/transition.js` (Svelte's
  built-ins, reading native computed visuals and bounds) and `motion.js` (TR1/TR2): a backend that
  answers `OP_ANIMATE` gets each animation's keyframes and timing once and plays it, "no JS runs per
  frame". [src]

What the compiler can still add [inferred]:

1. **`animate:flip` as a native layout animation.** When `flip` is imported from `svelte/animate`
   (recognized by import identity, the way `bind_sources` rows match imports) and its params are
   static or numeric, lower to `$n.flip(node, params)`: tessel records `from` rects before the keyed
   reorder commits and `to` rects after its own layout, then animates the transform in the
   compositor. Removes the forced layout read per moved item. Fidelity: same timing formula
   (`d => Math.sqrt(d) * 120`, `cubicOut`); fall back to the JS path for function-valued params.
2. **Parametric transitions.** For built-in transitions with static params, send easing id plus
   endpoints instead of 60 fps samples, so 120/144 Hz displays get the exact curve. Quality, not
   speed; low priority.
3. **Springs** only through class D of §3.8 (native when `.current` feeds only property-class
   outputs).

---

## 7. Interop surfaces: one schema for compiler and toolkit

- React Native codegen reads typed TS/Flow specs (`codegenNativeComponent<Props>('Name')`, files
  named `*NativeComponent.ts`) and generates C++ interfaces and typed props parsing per platform
  [doc] https://reactnative.dev/docs/fabric-native-components-introduction ,
  https://microsoft.github.io/react-native-windows/v1/docs/native-platform-components . Fabric
  parses `RawProps` into typed structs with per-type converters (`js-native-serdes.md` §4.4).
- Static Hermes (now shipped as Hermes V1, experimental in RN 0.82 per a 2025 podcast summary)
  compiles soundly typed JS ahead of time, with a "zero-cost FFI" as a 2023 design goal [doc]
  https://speakerdeck.com/tmikov2023/static-hermes-react-native-eu-2023-announcement ,
  https://www.callstack.com/podcasts/from-static-hermes-to-hermes-v1-the-road-to-default . Not
  usable with QuickJS-NG; the transferable idea is typed call sites.
- Lynx Element PAPI (`__CreateElement`, `__SetAttribute`, …) and the ET PAPI (§2.7).
- svelters' semantic spec (`metadata/semantics/*.json`, `docs/notes/native-compiler/semantic-spec.md`)
  is already this schema for the compiler: kinds, chains, typed props, signals with args, slots,
  bindings, `bind_sources` lowering rows, a11y. It is generated for GTK/Qt from upstream metadata
  and hand-extended for egui.

Proposal [inferred]: make the spec the **single contract for both sides**, with four additions:
(1) dense integer ids for kinds, props, signals, slots and style properties, plus a spec hash the
bundle carries and the host checks at load (a mismatch is a load error, so ids can never drift);
(2) `effect` (layout/paint/property) per prop (§4.3); (3) a `templates` capability and the template
ABI version; (4) generate the Rust side from the same spec for tessel kinds: typed prop decoders by
id (as Fabric's props structs), so `set_prop` is a table dispatch with no string handling. Tessel
kinds written in Rust produce their spec fragment (a derive or a build step), so a new kind is
visible to the compiler without hand-written JSON.

---

## 8. Hot reload: which optimizations survive HMR

- Svelte 5.57.2 `hmr: true` wraps each component in `$.hmr(fn)` (`internal/client/dev/hmr.js`):
  a `block` re-runs when the component source is replaced, destroys the old branch and re-mounts;
  transitions are suppressed on the swap. Component-local `$state` is lost [src]; the full
  analysis and svelters' H1–H5 plan are in `svelters/plans/research/HMR.md` (§2.1, §3.3).
- React Fast Refresh and Flutter keep state keyed by declaration identity with a signature reset
  (HMR.md §2).
- Lynx ET: template definitions keyed by content hash; their reload path needed explicit GC and
  stale-call filtering (§2.7).
- Slint's live preview interprets instead of recompiling (`slint-lessons.md` §7); Makepad updates
  Rust structs from the DSL at run time (§3.6).

Rules for svelters + tessel [inferred]:

1. **Every compile-time artifact is module-scoped data keyed by content hash**: templates, style
   sets, precompiled sheets, native-binding expressions. An HMR update registers the new module's
   tables; instances of old templates live until their component re-mounts; the table entry is
   freed when its instance count reaches zero (avoid Lynx's reload leak by making the native side
   own the refcount, not the JS GC).
2. **Interned ids are spec-scoped, not build-scoped**, so they never change during a dev session
   unless the spec (i.e. the Rust kinds) changes, which is a hot restart anyway (HMR.md H5).
3. **Native bindings keep their JS twin** (§3.8), so a module swap that changes an expression out
   of the pure subset silently falls back to JS: no special case.
4. HMR.md §3.3 item 7 already establishes the dev/release split for setter lowering ("Release
   builds keep compile-time lowering"; HMR builds emit rules as data). The same split applies to
   utility resolution and rule hints: dev may skip them for faster rebuilds, release does them.
5. H4's compiler-emitted element identity (hash of component id + static template position) is
   exactly the template node index of §2.9: `(template hash, node index, each-key path)`. One
   identity serves HMR state restore, automation locators and the template.

---

## 9. Ranked list of compiler↔toolkit integrations

Gains are inferred unless marked; "measure" names the M1 measurement that decides it.

| # | Integration | Expected gain | Complexity | Svelte-fidelity risk |
|---|---|---|---|---|
| 1 | **Interned ids from the spec** (kinds, props, signals, slots) + spec hash check | −0.66 µs per prop write [meas-local 1.43→0.77 µs], every op, all backends that adopt it | Low (compiler emits ints; host table) | None |
| 2 | **Native template instantiation** (`$n.instantiate(T)`, contiguous handles, static props/classes/listeners inside, one-call teardown) | Construction crossings ~N→1 per fragment; e.g. counter ~36 static crossings (~30–40 µs) → 1; 200×10-node list ~6 ms → <1 ms; fewer JS objects, smaller GC heap. Measure: panel open and list fill | Medium (template table, ABI op, runtime ownership change, oplog test view) | Low (Svelte already clones static templates; reactive path unchanged) |
| 3 | **Lazy JS node objects + inlined write compare** (no `Native`/Maps for static nodes; `v !== p.x && host.setp(h, ID, p.x = v)` for typed, untracked writes) | Fewer allocations per node (~11 today); avoids a JS wrapper call per unchanged write. Measure: update-heavy panel | Low–medium (runtime refactor; compiler knows which writes are plain) | Low (same compare semantics as `$n.set`; keep `$n.set` for tracked/function/proxy values) |
| 4 | **Invalidation class per prop and per class toggle** (spec `effect`, style-table classes, compile-time tags on slots) | Property-only writes skip style/layout/record (toolkit §6.1); enables 6 and 7 | Low (metadata + tags) | None |
| 5 | **Passive-listener inference + native listener table** | Scroll/drag never wait for JS unless a handler can cancel; saves a crossing per listener | Low–medium (conservative escape analysis) | None (conservative) |
| 6 | **Native bindings from a pure subset, JS kept authoritative** (classes A–D of §3.8) | Latency: scroll/pointer-linked visuals update in the compositor frame, even when JS is busy | Medium–high (subset compiler, expression VM, reconciliation, number formatting parity) | Low if JS stays authoritative; high if JS were dropped |
| 7 | **`animate:flip` and springs as native animations** | No forced layout per moved item; no per-frame JS for springs | Medium | Low (same formulas; JS fallback for dynamic params) |
| 8 | **Precompiled sheets, interned style sets, static utility resolution, rule candidate hints** | Faster startup/HMR CSS and style recalc; moderate | Medium | None–low (hints only as reliable as Svelte's own pruning, which is ported byte-identically) |
| 9 | **Compile-time a11y checks** from the spec's `a11y` fields | Correctness, not speed | Low | None (warnings) |
| 10 | Template-effect splitting (one effect per dependency set instead of per fragment) | Less re-evaluation for large fragments when one value changes | Medium | Medium (changes effect count/order; prior art — Svelte, Solid, Vapor — all group per fragment) — measure before doing |
| — | Second JS runtime for worklets (Lynx MTS / Reanimated UI runtime) | — | High | Medium–high | **Not recommended** (§3.8) |

Ordering rationale: 1–3 hit every component today (cadatomic's UI is mostly construction and
discrete updates, §3.8 counts); 4–5 are cheap enablers that tessel's frame model (§5, §7 of the
toolkit doc) relies on; 6–7 pay off for drag- and scroll-heavy UI that cadatomic does not have yet.

## 10. Open questions and measurements for M1

1. Rust-side cost of creating a template instance per node in tessel's arena (allocation, style
   attach, a11y node): the template gain depends on it being ≪ 1 µs.
2. QuickJS-NG cost of today's `Native` constructor and `$n.set` wrapper (not measured; only the
   host-call cost is).
3. GC pause vs heap size with and without lazy node objects (`js-runtimes.md` §6.3 has no NG
   numbers).
4. Whether template-effect splitting (#10) pays in real cadatomic panels.
5. How often real handlers fit the pure subset (count in svelters' gallery and cadatomic once
   canvases land).
