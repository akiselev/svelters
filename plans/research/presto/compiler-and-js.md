# presto, the svelters compiler and the JS engine

Part of the GUI toolkit proposal (`README.md` in this folder). Date: 2026-10-25. Status: proposal
for the user; nothing is built.

The user asked for the toolkit to be **closely integrated with the svelters compiler**, and to
consider **forking rquickjs and QuickJS-NG** so JS objects integrate with the renderer and nothing
is serialized between the layers. This file covers both.

Evidence:
- `round2/compiler-integration-local.md`: the svelters compiler, runtime and host today, with
  file:line citations;
- `round2/compiler-prior-art.md`: Solid, Svelte 5, Vue Vapor, ivi, Marko 6, Lynx, Slint, qmlsc,
  Reanimated;
- `round2/js-host-local.md`: today's JS↔Rust boundary and its measured costs;
- `round2/zero-serdes-js.md`: V8 Fast API, Deno ops, Blink/Servo bindings, JSI, Static Hermes,
  PrimJS, QuickJS internals.

All are in `notes/`.

## 1. Where the time goes today

This decides the order of everything below.

| Measurement | Value | Source |
|---|---|---|
| A JS→native host call (rquickjs, 3 args, no-op body) | 837 ns min, ~177 ns of it the JS loop | `js-host-local.md` §2 |
| A plain JS→JS call in QuickJS-NG | 234–325 ns | same |
| `setProp` by name vs by integer id | 1,433 vs 768 ns (2,235 vs 1,607 after S1) | `js-native-serdes.md` |
| The same op done in Rust | ~6 ns | same |
| Allocating `{x,y,w,h}` in JS | ~1.2 µs, more than the native call | same |
| A `$state` proxy read of 4 fields | +2.5 µs | same |
| P5 fill, 11,000 ops: QuickJS time vs toolkit calls vs host/ABI | 1,479–1,907 ms vs 226–423 ms vs 16–18 ms | `js-host-local.md` §3 |
| Batching ops into typed arrays | slower than direct calls in QuickJS | EGUI.md:105 |
| Counter fixture (9 nodes): ops at build, of which never state-dependent | 49, of which 36 | `compiler-prior-art.md` §0 |
| Static writes among all `$n.set` | 84% | `compiler-integration-local.md` §2 |

Most numbers come from heavily loaded machines (load average 11–31), so the ratios hold better than
the absolute times.

**Executing JS dominates, and the boundary comes second.** Half of a ~1 µs host call is QuickJS
evaluating the argument expressions. A perfect zero-cost FFI saves at most about 0.5 µs per op,
while each op it saves outright saves the whole microsecond plus the JS that produced it. So the
order is:
1. **emit fewer ops**: templates, static writes resolved by the compiler;
2. **do less JS per op**: lazy node objects, inline compares, interned ids;
3. **make each crossing cheap**: raw fast calls;
4. **only then** consider changing the engine (§5).

## 2. One schema: the generated `presto.json`

The compiler already reads each backend's semantic spec through `PlatformSemantics`. Adding presto
is one more `include_str!` and one more match arm (`semantics/backend.rs:53-62`); the rest is data.

The spec is **generated from presto's Rust kind tables** (`Kind::spec()`, architecture §4.2). The
generator is `svelters-presto/src/spec.rs`, in the same shape as egui's `spec.rs` that emits
`egui.json`, the `.d.ts` files and the style section, guarded by a staleness test.

What presto's spec adds over egui's:

| Field | Purpose |
|---|---|
| **Stable ids** for kinds, props, signals, methods and slots, with a `fingerprint` (hash of kind name, member name and type) per id and a spec hash | Ids on the wire (§3.1). Today's egui ids are positional, so adding a row renumbers every later id |
| `change` per prop and per CSS property (`structure` / `layout` / `paint` / `property`) | The toolkit's invalidation class (architecture §4.3), so the compiler can classify writes, toggles and transitions |
| `passive`, `cancelable` per signal | Passive-listener inference (§3.5) |
| `native_state` per kind (scroll offset, selection, drag value) with its observe facts | One owner per property; `bind:` planning |
| `capabilities` (`templates`, `native_transitions`, `native_motion`, `native_expr`, `css_precompiled`) | Which new op shapes this backend accepts. Every other backend keeps today's path |
| `a11y` per kind (role, name sources, required labels) | Compile-time accessibility checks (§3.9) |

The host checks the spec hash at load. A mismatch in dev triggers a recompile; in release it is an
error naming the two hashes.

Hard-coded `"egui"` checks become capability checks:
- `svelters-modules/src/source/transform.rs:81`;
- `packages/native-runtime/vite.js:376,451`;
- testhost:437;
- compiler `backend.rs:59` and `kinds.rs:141`;
- `gen_props.py`/`gen_utilities.py`'s toolkit list.

## 3. Compiler integrations, ranked

Fidelity means staying faithful to stock svelte@5.57.2, which svelters must keep. The gates are:
- the DOM conformance score (628/774);
- `core-contract.test.mjs`;
- the 50 byte-exact `expected.oplog` files;
- CF2's init/update/after-update op order.

Every integration below is a **backend capability**: the DOM conformance backend and the other
native backends keep today's op path, so the conformance gates are unaffected.

| # | Integration | Gain | Complexity | Fidelity risk |
|---|---|---|---|---|
| 1 | Integer ids on the wire | −0.66 µs per prop write (measured) | low | none |
| 2 | Native template instantiation | build ops −70% (counter: 36 crossings → 1); a 200×10 list ~6 ms → <1 ms of crossings | medium | low |
| 3 | Lazy JS node objects, inline last-value compare | fewer allocations, less GC; no wrapper call for unchanged values | low–medium | low |
| 4 | Change class on writes, toggles and transitions | property-only writes skip style, layout and recording; enables 6–8 | low | none |
| 5 | Passive-listener inference | scroll and drag never wait for JS unless a handler can cancel | low–medium | none |
| 6 | Native transitions, `animate:flip`, springs | no JS per frame, real refresh rate, no forced layout reads per moved item | medium | low |
| 7 | CSS precompilation and rule hints | faster startup, restyle and HMR CSS updates | medium | none–low |
| 8 | Native expression bindings (pure subset, JS authoritative) | input-to-pixel feedback in the compositor frame even while JS is busy | medium–high | low (high if JS were dropped) |
| 9 | Compile-time accessibility checks from the spec | correctness | low | none |
| 10 | Splitting a fragment's template effect by dependency set | unclear; measure first | medium | medium |

**Not recommended:** a second JS runtime for "main-thread" worklets, Lynx- or Reanimated-style. Lynx
pays for it: captures are JSON-serialized, read-only and refreshed only on re-render. Reanimated 4
and the web both moved toward declarative native animation instead.

### 3.1 Integer ids

The runtime and generated code pass strings today: `$n.element("label")`, `$n.set(n, "label", v)`.
The host resolves each through SipHash nested `HashMap`s, though it already has
`kindId`/`propId`/`signalId`/`methodId`/`slotId` resolvers (`quickjs.rs:1008-1050`).

**What the compiler emits instead:** `$n.element(K_LABEL)` and `$n.setp(n, P_LABEL, v)`, with the
constants taken from the spec. The spec hash is checked when the module loads.

**Op logs.** They stay "by JS-facing names, never by IDs": the host logs names through the spec, so
every `expected.oplog` is unchanged.

**This works on egui too,** once egui's ids are made stable. It is a win before presto exists.

### 3.2 Native templates

**What is static.** Each fragment's static part is known at compile time:
- elements and their kinds;
- static props;
- the scope class and static classes;
- slot structure;
- listener registrations.

The lowering already separates static (`Op::Set`) from dynamic (`Op::SetDynamic`) writes
(`lower/mod.rs:1270`, `:1081`), and codegen's `phase()` splits them (`codegen/component.rs:600-619`).

**What the compiler emits instead of N ops:**
- **A template.** A compact module-scope table in the style of ivi's opcode arrays and Lynx's
  Element Template. It holds:
  - node kinds;
  - the parent index and slot of each node;
  - static props as `(prop id, typed constant)`;
  - classes as interned ids;
  - listener slots `(node index, signal id, flags, handler index)`;
  - holes for blocks (`{#if}`, `{#each}`, components, `{@render}`), which become regions.

  It is keyed by a content hash.
- **One instantiate call.** `const base = $n.instantiate(T, [h0, h1, …])` creates, inserts, sets
  and registers everything in one native call. presto allocates the handles contiguously, so node
  `k` is `base + k`: no navigation calls, unlike DOM template cloning.
- **Dynamic writes as today, with ids,** inside the fragment's single `$.template_effect`, which keeps
  CF2's ordering.
- **Teardown in one call.** `$n.destroy_range(base, n)` releases a whole non-hole subtree. Ownership
  scopes record the range, not N handles.

**What the toolkit does:**
- `instantiate` is a Rust loop over the table: a slab allocation of N nodes, typed constant writes,
  and listener entries pointing at handler slots in a per-instance JS array.
- The per-node Rust cost must stay well under 1 µs for the gain to hold (M1 measures it).

**Fidelity.**
- Svelte 5 itself hoists and clones static templates, and the reactive code is unchanged.
- Op order: `instantiate` expands to exactly today's sequence: create, static sets, children
  inserted before their parent's insertion, then listens. With op logging on, the host writes those
  expanded lines, so the goldens stay byte-identical.
- The DOM backend and other backends don't advertise the capability and keep the per-op path.

**HMR.**
- Templates are module-scoped data, registered per module version and keyed by content hash. The
  native side owns their refcounts, which avoids Lynx's documented template leak after
  `reloadTemplate`.
- `$state` declarations are untouched.
- Partial instantiation is disposable, so a failed build rolls back through `dispose`.
- Dev builds keep per-node source offsets: a template node index maps to `source-loc`, which the
  designer (DZ0) needs.

**Where it lives.** A new IR item (`Item::Template` wrapping the static prefix of a fragment), a
`$n.instantiate` primitive, and a host/ABI op. It was planned already: `docs/notes/egui-all-rust.md:284`,
EGUI.md G5, and SV43 (row templates). Template tables can ship as a module constant, or in the
`.svb` bytecode's reserved optional sections (`svb/mod.rs`).

### 3.3 Lazy node objects and inline compares

Every native node gets a JS `Native` object today, which eagerly allocates 8 Maps, 1 Set and 1
object (`index.js:84-108`). `requested`, `inertFlag` and `snippets` are not set in the constructor,
so reads of them fall through the proxy-backed prototype into `providerMethod`'s Map lookups.

**What changes:**
- **No eager objects.** A static template node gets no JS object unless code takes a reference to
  it (`bind:this`, an action, an attachment, a dynamic write). Its Maps are created on first use.
- **Inline compares.** Typed, untracked writes compile to `v !== p.x && host.setp(h, ID, p.x = v)`,
  as Solid does, so an unchanged value costs no wrapper call. `$n.set` stays for writes that need
  `plainData`: proxies, functions, records.

This needs no toolkit change and helps egui too.

### 3.4 Change classes

The compiler tags each dynamic write site, `class:` toggle and `style:` directive with its change
class from the spec. For a `class:` toggle that means computing which properties the toggled
class's scoped rules can set, using the Rust port of Svelte's scoped CSS, which already does
per-selector analysis.

**Effects:**
- **Runtime:** a property-class write goes straight to the compositor's mailbox as a fast commit
  (architecture §7.3), and a paint-class toggle restyles without relayout.
- **Diagnostics:** a dev warning names a binding that drives a layout property from a per-frame
  source (pointer move, scroll), with a fix-it to `transform`.
- **Eligibility:** native transitions (§3.6) and native expressions (§3.8) depend on the class.

Lowering is unchanged when the class is unknown, as on other backends.

### 3.5 Passive listeners

**Today:** `|passive`/`|nonpassive` are accepted and ignored (`forward.rs:151`). Svelte itself marks
listeners passive only by event name (`touchstart`, `touchmove`; `utils.js:261`).

**The compiler infers passivity conservatively.** A listener is passive when either:
- the handler takes no parameter; or
- its event parameter never escapes and is never the receiver of `preventDefault`,
  `stopPropagation` or `stopImmediatePropagation`.

Anything else is non-passive. `|nonpassive` and `|passive` override, with a warning when the source
contradicts the inference.

**The result reaches the toolkit** as `LISTEN_PASSIVE` in `listen_ex`'s flags, alongside capture = 2
and after = 1.

**The diagnostic:** `nonpassive_scroll_listener` warns about a non-passive `wheel`, `pointermove`,
`touchmove` or `scroll` handler, because it makes that region's scrolling wait for JS.

**The gap:** no compiler we found infers passivity from handler bodies (`compiler-prior-art.md` §4).
This is new, and cheap.

### 3.6 Native transitions, FLIP and springs

**Today:**
- **Transitions.** Svelte's transitions compute keyframes in JS: `ceil(duration / 16.67)` frames of
  `css(t)`, with the easing baked in at 60 Hz (`round2/animation-scroll.md` §1). svelters already
  plays them natively through `OP_ANIMATE`, so no JS runs per frame.
- **Springs and tweens.** `svelte/motion` is a JS loop on rAF.
- **FLIP.** `animate:flip` reads layout through `getBoundingClientRect` for every moved item. svelters'
  own port has a scale bug: `sx = to.width / to.width`, always 1.

**With the `native_transitions` capability:**
- **Built-in transitions.** For transitions imported from `svelte/transition` (`fade`, `fly`, `slide`,
  `scale`, `blur`, `draw`), the compiler emits `$n.transition_native(node, flags, FADE, () => params)`.
  - presto evaluates the same formulas natively, at the display's real refresh rate, from the
    parameters. Opacity and transform tracks run on the compositor, and `slide` runs as a layout
    track.
  - Custom transition functions keep the JS `css()` path.
  - **Fidelity test:** sample the native curve at Svelte's 60 Hz keyframe times and compare against
    Svelte's JS output within 1e-6.
- **`animate:flip` imported from `svelte/animate`** becomes `$n.flip_native(node, () => params)`.
  presto measures from its own layout before and after the keyed reorder and plays a compositor
  transform track, with no JS layout reads. Fixing the scale bug is part of the port.
- **Springs and tweens.** With the `native_motion` capability, `svelte/motion` is aliased to a
  native `svelters:motion` module with the same API.
  - It uses a closed-form spring, mapped from Svelte's parameters through the 60 Hz recurrence (exact
    at 60 Hz).
  - It is frame-rate independent; Svelte's integrator overshoots 23% at 60 Hz and 0.7% at 240 Hz for
    `{0.1, 0.25}`.
  - A spring that feeds only `style:` properties runs on the compositor.

### 3.7 CSS precompilation

**Today:** on egui the compiler only *checks* `<style>` with the engine's parser (`engine_css.rs`).
Matching and the cascade run at run time on the scene, and classes, id and inline style travel per
element at the end of each job.

**With `css_precompiled`:**
- **Sheets** are parsed, interned and serialized at compile time: a binary sheet per component,
  keyed by content hash and loaded with `registerStyles`.
- **Static utility classes** are resolved to interned property sets, StyleX-style, deduplicated
  across the app.
- **Rule-candidate hints per element** come from Svelte's own pruning analysis: which rules can
  ever match this element. The runtime matcher then tests only those, plus the global sheets.
- **Static matches** for elements whose classes are all static are precomputed, as CSS.md
  suggested. A compile-time matcher already exists for the other backends
  (`match_selector`, `lower/mod.rs:1787`).

**Dev builds** keep the runtime path for HMR CSS edits, the split HMR.md §3.3 already makes for
style setters.

### 3.8 Native expression bindings (later)

**The pattern.** Some bindings are pure functions of native-owned state:
- a parallax `style:transform={`translateY(${scrollY * 0.5}px)`}` with `scrollY` from
  `bind:scrollTop`;
- a drag handle's transform;
- a progress bar driven by a native value;
- a spring feeding only styles.

The compiler can lower these to a small, typed expression program (arithmetic, comparisons,
conditionals, clamp/min/max, string templates of numbers) that presto evaluates on the UI thread at
input time, or on the compositor for property-class targets.

**Fidelity rule: JS stays authoritative.** The original JS still runs: native applies the value
early, and JS's later identical write is a no-op. The cost is an exact JavaScript `Number`-to-string
formatting in Rust and sequence-number reconciliation.

**Why it is later:** cadatomic's 26 `.svelte` files have 3 `style:` directives, 34 `class:`
directives, no pointer-move handlers and 4 `in:` transitions. So construction cost and discrete
updates matter now; this matters once canvases and designers are written in Svelte.

**Before building it, measure** how often real handlers fit the pure subset.

### 3.9 Accessibility diagnostics

The spec's `a11y` fields let the compiler warn where egui 0.37's kittest fails at run time. Examples:
- an input without an accessible name or a `labelled-by` relation (kittest's "widget(s) lack
  accessible name");
- an icon-only button without a label.

Every new diagnostic code gets a row in `codes.rs`, and the `cx1_codes`/`cx1_check` tests cover it.

### 3.10 Event-time dispatch

There is no compiler change here. The runtime delivers every listener, widget signals included, at
event time (architecture §11.1). That removes the outbox path behind cadatomic's 66 `onclicked`
handlers.

The host gains a "dispatch and drain microtasks" entry: `call_sync` drains nothing today
(`run_jobs(0)`).

## 4. Constraints the integrations respect

| Constraint | How it is met |
|---|---|
| Svelte fidelity | capabilities per backend; the DOM conformance backend keeps the generic path; transitions and springs tested against Svelte's own JS output |
| Op logs | the host expands new op shapes into today's lines when logging; names, never ids |
| CF2 op order | `instantiate` expands to the CF2 init order; dynamic writes stay in the fragment's template effect |
| HMR | every compile-time artifact is module-scoped data keyed by content hash and replaceable by id; the native side refcounts; `$state` untouched; native expressions keep their JS twin, so an edit that leaves the subset falls back to JS |
| Designer (DZ0) | `source-loc` per node is kept in dev, mapped from template node indices; anchor scopes and overrides held in the backend as today |
| Diagnostics | each new code gets a row in `codes.rs` |

## 5. The JS engine: forking rquickjs and QuickJS-NG

### 5.1 The recorded decision

"svelters uses stock QuickJS-NG; there is no rquickjs fork and no determinism fork" (user,
2026-10-21; DECISIONS.md "UI framework", svelters `CLAUDE.md`). A fork plan therefore reverses a
user decision and needs the user's sign-off (README Q4).

The vetoed fork was about determinism. cadatomic's decision 9 wanted to replace `Math.random` and
`Date.now`, and svelters now does seeded random and a virtual clock without a fork. It was **not**
about JS/scene integration.

The research also argued for keeping the engine swappable:
- "widgets go through handles and the C ABI so the engine stays swappable" (QUICKJS-RUNTIME §4
  item 13);
- the planned isolates (ISO1) send serializable op batches "provided the protocol never relies on
  shared memory" (ARCH-SECURITY).

### 5.2 Today's boundary

From `round2/js-host-local.md`.

**Ops and conversion.**
- **Registration.** About 30 host ops are plain rquickjs `Function::new` closures on
  `globalThis.__svelters_host`: no classes, no `JS_SetOpaque`.
- **Conversion.** The host's own engine-agnostic `JsReader`/`JsWriter` builds a borrowed 24-byte
  `svl_value` tree. ASCII strings are zero-copy through `JS_ToCStringLen2`.
- **egui copies every write.** It does not override `set_prop_ref`, so each write is copied into an
  owned `svelters_abi::Value`, and strings are cloned again.

**Value representation on each side.**
- **Handles:** JS numbers below 2^53.
- **Enums:** kebab-case strings, stored by egui as `&'static str` chosen by linear search.
- **Records:** plain objects, read field by field.

**Scene storage:** typed Rust columns. A typed Rust read is zero-copy; a write goes through `Value`.

### 5.3 Where the boundary cost really is

From `round2/zero-serdes-js.md`, which reads QuickJS-NG `7a23d8e`, rquickjs `c0bef90`, PrimJS
`60e1b41` and Servo `99d1012`. The per-op figures are estimates from reading source for an unloaded
x86-64 release build. No published rquickjs or QuickJS call benchmark exists.

- **The C-level crossing is already cheap.** QuickJS-NG hands the interpreter's own stack slots to
  a C function: no argument array is copied, and an integer `magic` slot can carry a prop id.
- **rquickjs adds most of the overhead.** A `Function::new` closure is a class object, not a C
  function.
  - Each call duplicates the context twice and builds a parameter list.
  - Each argument costs about four more out-of-line C calls, so three arguments come to roughly
    15–25 extra C calls plus a dynamic dispatch.
  - `#[qjs(get,set)]` accessors are worse: they are whole closures on the slowest property path.
- **QuickJS-NG has no inline caches.** They were removed in #884 (2025-02-06) as a mixed bag on
  speed that always costs memory.
- **Svelte's runtime dominates each update:** about 5.5–7 µs per batched state change
  (REACTIVITY.md).
- **`$n.set` does JS-side work before it crosses:** a proxy copy, two `Map` dedupe checks, and the
  string name.

### 5.4 Options

Today's crossing is about 0.8–1.4 µs on the loaded machine, an estimated 250–450 ns unloaded.

| Option | What | Est. per op (unloaded) | Maintenance | Conflicts |
|---|---|---|---|---|
| **(a) No fork** | hot ops as raw `rquickjs-sys` C functions with `magic` = prop id; numbers read from the value tag; literal strings looked up as atoms; ASCII borrowed in place; compiler-emitted typed setters (`set_f64(node, P_W, v)`); typed `set_prop_ref` writes into presto's columns, no owned `Value` | **~40–80 ns** (call 25–50, Rust body 5–20) | none | none |
| **(b) Light fork** | native slots per object, typed call signatures, skipping the interrupt poll and stack frame | ~30–60 ns: only 5–20 ns better, because there is no argument array to bypass | rebase each release, 0.5–2 days; native frames drop out of stack traces | engine swap harder |
| **(c1) Exotic class, no fork** | `node.label = v` caught by QuickJS's public property hook, which receives the property as an integer atom | ~50–100 ns | none | every property access, method lookups included, hits the hook; enumeration and expando properties need handling. RN's HostObject measured 181–188 ns against 27–38 ns for plain functions |
| **(c2) Deep fork** | nodes as a custom value tag, with no JS object (as PrimJS) | ~30–60 ns, plus one object saved per node | effectively permanent: PrimJS's tag appears at 37 sites, and PrimJS no longer tracks upstream | ISO1 isolates (serializable protocol), engine swap (V8 for throughput), the C ABI's toolkit independence |

**Upstream churn** (from git):
- **Volume:** QuickJS-NG had 367 commits in 12 months, 221 of them touching `quickjs.c`.
- **Releases:** 10 tags from v0.11.0 (2025-10-28) to v0.17.0 (2026-09-18).
- **Size of change:** `quickjs.c`/`.h` gained 9.5k lines and lost 2.6k across that span.
- **API stability:** the public header changes every release. After 0.17 the interrupt handler
  gained a context argument (#1802), and `JS_NewArrayBuffer` moved to a realloc function.
- **rquickjs** shipped 7 releases in 12 months, each pinning an NG commit.

**Comparison with other engines.**
- **Deno:** removing its JSON op ABI gained about 15×. Codegen ops went from ~50 to ~28 ns, and below
  that Deno relies on V8 Fast API calls.
- **Bun/JSC:** a no-op costs 2 ns through the C++ API.

So option (a) brings QuickJS's crossing within about 2× of what Deno reaches on V8, without a fork.

### 5.5 Lifetimes and strings (option (a))

**Wrappers.**
- One wrapper per node, created lazily (§3.3). The node's id and generation sit in the object's
  native pointer slot, so creating a wrapper allocates nothing in Rust.
- The tree holds the wrapper strongly while the node lives.
- Event handlers are reported to the GC by the wrapper's mark hook, so a handler that captures its
  own node becomes collectable garbage after `destroy`. A plain Rust-held `JSValue` would be an
  untraced root and leak.
- Finalizers run mid-allocation, so they only clear the node's wrapper slot and queue the rest.

**Strings.**
- Static labels are interned at compile time and pass as ids.
- Runtime literals are looked up as atoms.
- Short dynamic strings borrow ASCII in place, are transcoded once when non-ASCII, and are deduped in
  Rust first.
- Large text lives in a Rust-owned rope behind a handle object (quire-text for editors).

QuickJS-NG has no external-string API, but none of this needs one.

### 5.6 Recommendation (README Q4)

**Don't fork rquickjs or QuickJS-NG now.** Take option (a) in this order:
1. **Templates, ids, lazy nodes (§3.1–§3.3).** These cut ops by ~70% and remove most per-op JS.
2. **Raw `rquickjs-sys` C functions for the hot ops.** These live in svelters-host's QuickJS adapter,
   which keeps the engine-agnostic `JsReader`/`JsWriter` and the C ABI for everything else. The
   compiler emits the typed setters.
3. **Move `$n.set`'s last-value dedupe into Rust columns,** keeping REACTIVITY.md's batched
   invalidation. That saves another ~10–25% of a batched update.
4. **Offer (c1) only as an optional surface for `bind:this` users who want `node.label = v`;** the
   compiler never emits it.
5. **Measure (plan W-J0).** Run a one-day A/B microbench on a quiet machine: rquickjs closure, raw C
   function, exotic class, with strings and the `Map` dedupe. Then measure the full update with and
   without steps 1–3.
6. **Reopen the fork question only if the crossing is still above about 30% of a batched update**
   after steps 1–3, or if a QuickJS-NG release blocks one of them.

CLAUDE.md allows a fork only when a release blocks the work, and nothing is blocked. (b) gains 5–20 ns
per op for a rebase every month. (c2) is permanent and conflicts with isolates and the engine-swap
option.

**If raw throughput becomes the limit, that is an engine question, not a binding question.**
V8 measured about 20× QuickJS on Svelte signals (QUICKJS-RUNTIME §5). The handle-based C ABI is what
keeps that option open, which is one more reason not to tie nodes to QuickJS's value tags.
