# Native semantic specifications (`svelters-semantics/v0`)

The NC1 compiler does not hard-code widget knowledge. Each backend has a checked-in spec,
`metadata/semantics/gtk.json` and `metadata/semantics/qt.json`. The compiler reads a spec through
`svelters_compiler::semantics::BackendSemantics`, which `SpecBackend` implements
(`SpecBackend::builtin("gtk")` embeds the checked-in file; `from_path`/`from_json` load others).

## Generated vs hand-written

| Source | Gives | Where |
| --- | --- | --- |
| `metadata/gtk/*.json` (GIR + PyGObject, overrides already merged), through `tools/metadata/gtk/gen_backend.py`'s `Model` | every GTK object/interface kind, chain, props (types, enum nicks, flags bits, ranges, access, defaults, notify), signals (args, handler result), annotated slots and methods, element tags | generated |
| `metadata/overrides/qt/*.yaml` resolved against `metadata/qt/*.json` by `tools/metadata/qt/gen_cpp.py`'s `Model` | Qt elements (one kind per annotated tag), their annotated props/signals/methods/slots (with `inherit:` facts applied), enum member names, toplevel class (`emit_toplevel`) | generated |
| `bindings:` annotations (`metadata/overrides/gtk/*.yaml` merged into `metadata/gtk`, `metadata/overrides/qt/*.yaml` elements; schema in `docs/notes/fork-bindings.md` section 6) | `bind:<name>`: property, direction, change signal and value source, echo strategy, compare-before-set, read-back, sync signals on set, lifetime | generated |
| `metadata/semantics/rules/{gtk,qt}.yaml` | facts no upstream source encodes: event aliases, toplevel `present`, text-content prop, signal arg contracts, Qt defaults, missing-element hints, style declarations and units | hand-written |

Regenerate with `python3 tools/metadata/semantics/gen_semantics.py` (`--check` fails when a
checked-in output is stale). Output is byte-identical on rerun. Run the GTK and Qt generators
first when their inputs change: the GTK spec is cross-checked against
`crates/svelters-gtk/src/generated/tables.rs` (tags, props, signals, slots, methods must match).

On Qt only annotated elements, props and signals exist at runtime, so the Qt spec contains only
those. On GTK every prop and signal in the backend tables is in the spec.

## Format

One JSON object; `kinds` and `tags` have one entry per line.

- `schema`, `backend` (`gtk`|`qt`), `framework`, `source`.
- `event_aliases`: DOM-style name -> signal (`click` -> `clicked`), applied before plain signal
  lookup, as `@svelters/renderer` does.
- `tags`: element tag -> kind id. GTK kind ids are GIR names (`Gtk.Button`; tag `window` is
  `Gtk.ApplicationWindow`); Qt kind ids are the tags.
- `kinds.<id>`:
  - `native` (GType / C++ class), `abstract`, `interface`.
  - `chain`: nearest first, starting with the kind. GTK: kind, its interfaces, then each
    ancestor (the same order as `svelters-gtk` `lookup::chain`). Qt: `[tag, class, bases...]`.
    Props, signals, bindings and slots are looked up along the chain.
  - `lifetime` (`owned`|`toplevel`) and `present` (the zero-arg method called after a
    toplevel's subtree is complete); `text_content` (prop that text children fold into).
    On GTK these three are inherited along the chain from the rule that sets them.
  - `props.<js>`: `js` (canonical; what generated code passes to the host), `native`, `type`
    (`bool|i32|u32|i64|f64|string|enum|flags|object|list|record|boxed|other`; u64 is folded
    into i64, f32 into f64; `other` = the ABI v3 types with no static form: date, time,
    datetime, bytes, variant, pointer), `nullable`, `readable`, `writable`, `construct_only`,
    `default`; optional `ref` (the raw neutral type ref, for every non-scalar type), `values`
    (enum/flags names), `bits` (GTK flags), `enum` (type id), `of` (object kind, list item type,
    record/boxed type name), `range`, `virtual`, `notify` (the native notification, which the
    backend may not expose). Static attributes coerce only scalars, enums, flags and
    `list<string>` (split on whitespace); the rest must be expressions.
  - `signals.<name>`: `name` (what the host resolves), `js_event`, `args [{name,type}]`,
    `handler_result` (`none|bool_stop|value`), `sync_required`, `disconnect_on_drop`, Qt `synthetic`.
  - `bindings.<name>` (from the `bindings:` annotations): `property` (JS name), `direction`
    (`two_way|read_only`), `event`, `value` (`{"arg": n}` from `change.value: argN`, or
    `"readback"` from `prop`), `echo` (`host_suppressed` -> `backend_blocks`, `compare` ->
    `runtime_compare`, `none`), `compare_before_set`, `read_back`, `natively_normalizes`,
    `sync_signals_on_set`, `lifetime`, `provenance`.
  - `children`: `policy`, `default_slot`, `slots.<name>` `{many, accepts [kind ids or Qt classes],
    route_by_kind, order (append_only|before_sibling|native_index|none)}`. Absent: no children.
  - `methods`: annotated methods callable through `host.call`.
  - `provenance`: where each rule-derived fact on the kind came from.
- `style.units.<unit>` `{scale}`; `style.declarations.<name>`: `applies_to` (kind or class, `is_a`),
  `contract`: `common` (`prop` + `value` kind `enum|length|bool`) or `native_intent`
  (`intents.<name>` = actions `[{op: set, prop, value} | {op: add_token, prop, token}]`, plus
  `unsupported.<name>` = reason). Lengths need a unit (except `0`) and are scaled, range-checked
  and must be whole for integer props.
- Qt only: `element_hints.<tag>`, advice for elements Qt does not have (diagnostics).
- Optional, used only by the DOM conformance backend (`metadata/semantics/dom.json`, generated by
  `tools/metadata/semantics/dom/gen_dom.py`; harness: `experiments/native-compiler/conformance`).
  Absent on GTK/Qt, whose behaviour is unchanged:
  - `text_node` `{kind, prop}`: the backend has text nodes. Text and `{expression}` runs that are
    not inside a `text_content` element become elements of tag `kind` whose `prop` holds the text
    (Svelte's `clean_nodes` whitespace rules), placed like any child. Without it, such text is
    `text_not_allowed`.
  - `html_syntax: true`: templates are parsed with HTML rules (`ParseOptions.native = false`:
    void elements, implicit closing).
  - A kind with a writable `class` prop (a token list) also receives `class="..."`,
    `class={expr}` and `class:name={cond}` (as `$n.token`), in addition to class-based style
    matching; on GTK/Qt `class` only drives native styles.

## Provenance rules

Every hand-written fact carries a `provenance` string naming its source: toolkit docs, a probe,
a backend contract (with the file or section), or a renderer convention. The generator refuses a
rule without one and refuses rules that reference anything that does not exist: unknown kinds,
props, signals, methods, intents on read-only props, defaults that do not fit the prop type, aliases with no target signal, style declarations
whose `applies_to` matches no kind with that prop. Binding annotations are checked the same way
(property in the chain and writable for `two_way`, change signal in the chain, `argN` in range
and of the property's type, a known echo strategy, a lifetime block). Generated facts keep their upstream origin in
`metadata/<toolkit>/*.json`; the spec does not repeat it.

## Adding a fact

1. If the fact is about what exists at runtime (a Qt element, prop, signal or slot; a GTK slot or
   method), add it to `metadata/overrides/<toolkit>/` and rerun that toolkit's generator, so the
   backend and its verification tests learn it too.
2. Otherwise add it to `metadata/semantics/rules/<toolkit>.yaml` with a `provenance`:
   `kinds.<id>.{lifetime,present,text_content}`, `kinds.<id>.signals.<name>.args`,
   `defaults.<kind>.<prop>`, `style.declarations.<name>`. Bindings are not rules: annotate
   them under `bindings:` in the toolkit overrides (the renderer's `gen_bindings.py` reads the
   same annotations).
3. Run `python3 tools/metadata/semantics/gen_semantics.py`, then
   `cargo test -p svelters-compiler --test semantics_specs`.

## Diagnostic codes

`unknown_element`, `unknown_property`, `read_only_property`, `unknown_event`, `bind_no_plan`,
`children_not_allowed`, `unknown_slot`, `slot_rejects_child`, `no_default_slot`,
`style_unknown_declaration`, `style_not_applicable`, `style_bad_value`,
`style_intent_unsupported`. Each `Unresolved` has a message, suggestions (closest names by edit
distance, available slots or intents) and optional help; `Unresolved::report` records it as a
backend diagnostic.
