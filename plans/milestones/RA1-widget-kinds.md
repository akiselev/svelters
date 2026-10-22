# RA1: Rust-provided egui widget kinds (`#[svelters::widget]`)

Design: `plans/research/ARCH-EMBED.md` §8.1–§8.4, §8.6 and the §10 RA1 row, with D2(a) (`inventory`), D3(a)
(`measure`/`ui`/`a11y`) and D4(a) (kebab tags); `plans/research/ARCH-SCENE.md` "Decided" (S2(a): extension data
per key behind the `ExtKey` API) and §4.6. Builds on EG0 (`plans/milestones/EG0-module-split.md`) and RA2
(`Json<T>`, data enums). Merged with FW1 (`on:` forwarding) before the typings work.

## What landed

An app crate defines a native egui widget kind in Rust and uses it from Svelte as an element, typed end to end:

```rust
#[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]
pub struct QuantityField {
    #[prop(bind)] value: Option<f64>,      // bind:value, exact by construction (cx.set)
    #[prop] dim: Dim,                       // #[derive(svelters::Enum)] -> 'length' | 'angle'
    #[prop(default = true)] units_shown: bool,
    #[prop] label: String,                  // required in the typings (no default, not Option)
    #[prop(measure)] problem: Option<String>,
    #[event] accepted: Event<f64>,          // onaccepted={(v) => ...}
    #[event] edited: Event<String>,
    #[state] text: String,                  // Rust-only node state
}
impl Widget for QuantityField { fn measure(..); fn ui(..); fn a11y(..) }
```

```svelte
<quantity-field label="Width" {dim} bind:value={width} onaccepted={(v) => (accepted = v)} />
```

| Piece | Where |
|---|---|
| `Widget` trait (`measure`, `ui`, `a11y`, `busy`), `WidgetCx` (`id`, `rect`, `enabled`, `state`/`set_state`, `set` = the exact write, `emit`, `ext`/`ext_mut`, `request_measure`), `MeasureCx`, `A11yCx`, `AvailableSpace`, `Caps`, `Event<T>`, `KindEntry`/`PropEntry`/`EventEntry`/`KindRef`, `KindDecl`, `ErasedWidget`, `ExtWidget` (the node payload), `paint` (the walk's call) | `crates/svelters-egui/src/provider/mod.rs` |
| `StateBits` (built-ins 0–31), `StateBit` + `state_bits!` (provider bits 32–63, allocated at startup from `inventory`, sorted by name; >32 or duplicates panic), `state_by_name`, `Node::state`/`set_state` | `provider/state.rs` |
| `ExtKey<T>` (`new` sparse / `dense`, process-wide `ExtId`), `ExtBlock`, `ExtEntry` (`get`/`get_mut`/`set`/`remove`/`or_insert_with`), `Node::ext`/`ext_mut`/`ext_block` | `scene/ext.rs` |
| `KindRegistry` built once from `KINDS` + `inventory::iter::<KindRef>`; provider indexes `KINDS.len() + local`, ids `62 << 22 \| (local + 1)` sorted by spec id; member ids unchanged scheme; collisions with built-ins and duplicates panic at startup; `kind(k)`, `provider(k)`, `provider_member_desc`, `provider_prop_id`/`provider_signal_id` | `kinds/registry.rs` (`kinds/mod.rs` keeps the tables, `Ty::Provider`, `P::Ext(i)`) |
| The macro: struct form; `#[prop(bind, measure, default = expr, rename = "x")]`, `#[event]`, `#[state]`; emits the struct, the `KindEntry` (`offset_of!` per prop), `KindRef` and `KindTypesDef` registrations, and `KindDecl` (typed set/get/reset over `FromJs`/`IntoJs`) | `crates/svelters-macros/src/lib.rs` (`widget`), re-exported as `svelters::widget` |
| `KindTypesDef` (prop and event payload types described inside the export schema, module = the namespace), `Schema::kind_types`, `__private::with_cx` (a call context outside a host call) | `crates/svelters-host/src/exports/{mod,schema}.rs` |
| Spec fragment (`fragment`/`fragment_text`, `kinds.json`), built-in typings (`builtin_typings` → `packages/native-runtime/types/egui.d.ts`), provider typings (`kinds_typings` → `kinds.d.ts`), `dump(dir)` (exports + `kinds.json` + `kinds.d.ts`) | `crates/svelters-egui/src/spec.rs`, `svelters_egui::dump` |
| `Spec::merge_fragment`, `SpecBackend::merge_fragment(_path)`, `svelters-compile --spec-fragment` (repeatable, also with `--kinds`) | `crates/svelters-compiler/src/semantics/{spec,backend}.rs`, `bin/svelters-compile.rs` |
| Vite: `svelteNative({ specFragments })`, `svelteConfig(dir, { specFragments })` or `"svelters": { "specFragments": [...] }`; fragment tags count for selector checks | `packages/native-runtime/vite.js`, `packages/build/index.js` |
| `@svelters/native-runtime/types/egui` export (egui's own `IntrinsicElements`, with `on<event>`, `on:<event>` (FW1) and `bind:` keys, plus `Forward`/`ForwardFrom`) | `packages/native-runtime/package.json` |
| The example and acceptance tests | `examples/egui-kinds` (crate `egui-kinds-example`, app `@svelters-examples/egui-kinds`) |

Scene-side additions are small and behind accessors: `W::Ext(ExtWidget)`, `Node.state`, `Node.ext` (two fields in
`scene/node.rs`, set in `scene/tree.rs::new_node`), `P::Ext` routed at the top of `scene/props.rs` `set_prop`/`get_prop`,
one `W::Ext` arm each in `leaf.rs` and `layout.rs` (`intrinsic`, `wrapped` for `HEIGHT_FOR_WIDTH`), the dispatch
to `provider::paint` in `walk.rs::paint_node`, and `ops.rs` reading kinds through `kinds::kind(k)` (no more
`KINDS[k]` outside `kinds/` and `spec.rs`) with `describe` answering provider props from their `JsType` slot.

## Acceptance (ARCH-EMBED §10)

- **A test crate registers `quantity-field`**: `examples/egui-kinds/src/lib.rs`; `tests/kinds.rs`
  `registry_gives_the_kind_a_pack_62_id` (pack 62, id round trips, base `Widget` props resolve, ExtKey on a node,
  typed widget access).
- **The compiler rejects an unknown prop and accepts `bind:value` (`exact`)**:
  `compiler_rejects_unknown_props_and_accepts_bind_value` (also a static enum value outside the Rust enum; no
  bind warnings: the fact is `exact`/`delivery`).
- **svelte-check passes against the dumped typings**: `examples/egui-kinds/app` `pnpm test` (vite build with the
  fragment + svelte-check with `types/egui` + `kinds.d.ts`). Checked locally that it also *fails* on an unknown prop,
  a wrong enum literal, a missing required prop, a wrong handler payload type, a wrong `bind:` type, and a wrong
  built-in prop type (`<slider value="x">`).
- **kittest drives it by label**: `kittest_drives_the_field_by_label` (the field's TextInput is found by role and
  label, shows the bound SI value as `12 mm`, typing fires `onedited`, Enter commits through `cx.set` → bind:value
  and `onaccepted`; an unparsable entry sets the `cad.invalid` provider state bit and commits nothing).
- **Staleness test for `kinds.json`**: `checked_in_kinds_json_and_typings_are_current` (all five dumped files;
  `SVELTERS_UPDATE=1 cargo test -p egui-kinds-example --test kinds` rewrites). `tests/spec.rs` in svelters-egui
  also checks `egui.d.ts` now (`checked_in_typings_are_current`).

## Decisions and deviations inside the spec

- **Kind prop types live in the export schema.** The macro registers a `KindTypesDef` next to the `KindEntry`; the
  export schema describes those types in module `<namespace>` with its own `TypeBuilder`. So records/enums a kind
  uses get the same app type ids the host already converts with (`type_info` of pack-63 ids), `IntoJs` of records
  works (`record_type`), and the named types are declared in `svelters:app/<namespace>` (`app.d.ts`). A namespace
  with only kinds becomes a types-only `svelters:app/<ns>` module. The registry reads them back with
  `Schema::kind_types`. The prop slot the host sees is exactly the export slot.
- **Conversions outside a call** use `__private::with_cx(namespace, ..)`: a `CallCx` with an empty object table. A
  prop or payload holding an exported object (`Obj<T>`, `Arc<T>` of an `#[svelters::object]`) does not convert
  there (it errors). Not needed by RA1's acceptance; RA3 can thread the host's object table through the backend.
- **`cx.set` finds the prop by field offset** (`offset_of!` per prop in the `PropEntry`; the address arithmetic is
  safe Rust, K-7). A `set` on a non-prop field is stored and logged. It announces `prop_changed` only when the
  value changed (`T: PartialEq`), never an event (R18).
- **No `bindings` entry for provider kinds.** Bound props carry `observe: exact/delivery` in the fragment, so
  `bind:` lowers onto observation (`BindLowering::Prop`, no fallback signal). Built-ins keep theirs.
- **Handler typings follow the runtime**: plain signals call handlers with the arguments only
  (`onaccepted={(v) => ..}`, runtime `dispatch`); sync signals get `SvlEvent` first. ARCH-EMBED §8.3's
  `CustomEvent<number>` sketch is not what the runtime delivers.
- **`kinds.d.ts` is a separate file**, not appended to `app.d.ts`: it is a module (`import type` of the named
  types from `svelters:app/<ns>` and of `EguiAttrs` from `@svelters/native-runtime/types/egui`) with a
  `declare global` block, while `app.d.ts` is a script of ambient `declare module`s that an import would break.
  The app's tsconfig includes both.
- **egui typings** (coordinator's FW1 note): `packages/native-runtime/types/egui.d.ts` is generated from the
  built-in table and staleness-checked. An egui app uses `"types": ["@svelters/native-runtime/types/egui"]`
  instead of `.../types` (GTK's: the two declare the same tags differently). It carries its own `Forward`/
  `ForwardFrom`, so FW1's helpers resolve against egui's elements; `on:<event>` keys come from `OnDirectives`
  as on GTK. `label`/`button` (HTML names) merge into `HTMLAttributes` as GTK does.
- **`svelters_egui::dump(dir)`** is the dump entry (ARCH-EMBED says `svelters::dump`): the facade does not depend
  on svelters-egui. A crate using `#[svelters::widget]` depends on both `svelters` and `svelters-egui` (the
  generated code names `::svelters_egui`).
- **Defaults**: a `#[prop]` without `default` starts at `Default::default()` (so the type needs `Default`) and is
  required in the typings unless it is an `Option`. The spec fragment's `default` is read from a fresh widget.
  `null` from JS on a non-nullable prop resets it to its default (as built-ins do).
- **Spec type mapping**: bool/f64/i64(+range)/string/enum (values from the Rust enum) or `other` (records,
  lists, `Json<T>`, data enums: dynamic values; the fragment also carries the TS type as `ts`). `caps` are listed
  per kind in the fragment.
- **Measure**: `intrinsic` = `measure(MaxContent)`; `measure(Definite(w))` again only for `Caps::HEIGHT_FOR_WIDTH`.
  `request_measure()` and a `#[prop(measure)]` write from the widget invalidate the node in the walk (the next
  frame relayouts through the cleared taffy caches).
- **egui.json provenance** now names `crates/svelters-egui/src/kinds/builtin.rs` (EG0 left the old path for
  byte-identity; regenerated).
- **`ExtWidget` is `Clone`** (a fresh default widget) and `PartialEq` by identity, because `W` derives both.

## Notes for SG2, EV2b, RA3

- **SG2 (columns):** `ExtKey` already has `Storage` (`new` = sparse, `dense`) and a process-wide `ExtId` from a
  counter; `ExtBlock` is per node (`Node.ext`), so moving to columns is: replace `Node.ext` with the column table
  keyed by `ExtId`, keep `Node::ext/ext_mut/ext_block` and `WidgetCx::ext/ext_mut` signatures (`ExtEntry`'s
  methods are the whole write API). `StateBits` stays a node field (→ `NodeCore.state`). Hook the state-bit
  registry and ext writes into the journal there; nothing journals yet. `W::Ext` maps to `Payload::Widget`.
- **EV2b (behaviours):** reuse `KindEntry`'s shape and the `KindTypesDef` path for `#[svelters::behavior]`
  params; `provider/behavior.rs` is still empty. The registry's provider list is the place for attached kinds
  (no tag, `attaches_to` in the fragment).
- **RA3:** fn form, `wrap_widget!`, methods (`KindEntry` has no methods yet; the fragment emits `methods: {}`),
  containers (`Caps::CONTAINER` reserved; providers take no children today: `slot: None`), `cached_shapes`,
  `WidgetCx::children`/`store`/`object` (not landed). Object handles in kind props need the host object table
  (see `with_cx` above). `MeasureCx` has only text helpers.
- **EM1:** `provider::paint` uses `n.id` (the node's egui id) and `crate::a11y::name`; when EM1 changes id
  derivation it applies to provider nodes unchanged. `Widget::busy` exists but is not yet collected into
  `Settle`.

## Not run locally

- Full workspace nextest, conformance, the other backends' suites, the full JS test run (CI runs them).
- Local runs after the last code change: `cargo check --workspace --all-targets` (clean apart from pre-existing
  svelters-gtk/svelters-host warnings); nextest `-p egui-kinds-example` (4/4), `-p svelters-egui --test spec
  --test observe` (6/6), `-p svelters-compiler --test g1_egui_spec` (4/4); `pnpm test` in
  `examples/egui-kinds/app` (vite build + svelte-check, 0 errors).
