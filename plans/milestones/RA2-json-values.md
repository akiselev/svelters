# RA2: serde values (`Json<T>`), tagged data enums, codegen over `&Schema`

Design: `plans/research/ARCH-EMBED.md` §8.6 (one type system for exports, kind props and root props) and §10 (RA2
row), decided by the user on 2026-10-22. Requests: CADATOMIC.md SVP5 (serde bridge) and SV24 (codegen over a
descriptor value). Builds on E0 (`E0-rust-exports.md`). User docs: `docs/notes/egui-rust-exports.md` §5.5 and the
site guide `docs/site/src/content/docs/guides/rust-exports.md`. This file records RA2 as built.

## Scope

- `Json<T: Serialize + DeserializeOwned + JsonSchema>` crossing Rust↔JS as a structured value, typed from `T`'s
  JSON Schema in `app.d.ts`, with the schema itself in `exports.json`.
- `#[derive(svelters::Enum)]` with `#[js(tag = "...")]`: data enums as discriminated unions (serde's internally tagged
  form), both directions.
- SV24: the typings, stubs, JSON and dump are methods of a `Schema` value; the free functions call them on `schema()`.
- The `JsType` schema stays the only schema: `Json<T>` and unions are `JsType`s like any other, so kind props (RA1),
  root props and `svelters:embed` fields can use them with no extra work.

## As built

### `Json<T>` and `JsonAny<T>` (`crates/svelters-host/src/exports/json.rs`)

- **Slot.** A variant slot (`SVL_NULL` typed `SVL_T_VARIANT`, nullable). The host's existing dynamic conversion turns a
  JS object into `Value::Map`, an array into `List`, an integral number into `I64` and any other number into `F64`.
  Nothing new crosses the ABI.
- **In.** A serde `Deserializer` over the owned `Value` (`De`). It handles `deserialize_any`, options, newtype structs
  and externally tagged enums (a string or a one-key object). Internally tagged, untagged and flattened types go through
  serde's buffered `Content` path, which `deserialize_any` serves.
- **Out.** `serde_json::to_value`, then `from_json` builds `Value::Map`/`List`/`I64`/`U64`/`F64`. No JSON text crosses.
- **`JsonAny<T>`** (serde only) is typed `unknown`, for a type without a `JsonSchema` (cadatomic's `Refusal` today).
  Both wrappers are `Deref`/`DerefMut`/`From<T>`, with `into_inner()`.

### Typings from `JsonSchema`

- `TypeBuilder::json_schema::<T>()` asks one `schemars::SchemaGenerator` (draft 2020-12) per schema build for
  `subschema_for::<T>()`. One generator per build keeps `$defs` names unique across modules.
- `json::schema_ts` converts the schema to TypeScript. It handles `$ref` (also next to other keywords, which is how
  schemars writes a newtype variant of an internally tagged enum), `const`, `enum`, `oneOf`/`anyOf` (unions), `allOf`
  (intersections), `type` arrays, objects (properties, `required`, `additionalProperties`), arrays (`items`,
  `prefixItems` tuples), `true`/`false`. A property's `description` becomes an inline doc comment.
- **Declared types.** Every definition a module uses is declared in that module as `export type Name = …;`, closed over
  its own `$ref`s. That includes definitions reached through a record or union that has a `Json<T>` field
  (`NamedType::json_deps`). The definition's `description` becomes the doc comment.
- **`exports.json`.** These types appear with `kind: "json"`, the TS `type`, and the definition itself as `schema`.

### Tagged data enums (`crates/svelters-macros/src/lib.rs` `tagged_enum`)

- **Variants.** Unit, struct (named fields), or a newtype around a record or another object (its fields sit next to
  the tag, as in serde). Tags are kebab-case, field names camelCase, and both take `#[js(rename)]`. `Option` and
  `#[js(default [= e])]` fields are optional.
- **Rejected at compile time.** Tuple variants with more than one field (serde's internally tagged form has none),
  `#[js(bigint)]` inside a variant, a field named like the tag, and two variants with the same tag.
- **TS.** `type Shape = { kind: 'circle'; r: number; strokeWidth?: number; } | ({ kind: 'pt'; } & Pt) | { kind: 'empty'; };`.
- **Wire.** The slot is an untyped `SVL_MAP`, so the host converts the object structurally. A union works as a
  parameter, a result, a record field, a list element or an option. It gets no host type entry.
- **Generated code.** `FromJs` splits off the tag (`__private::tagged`) and reads fields by name (`Tagged::field`,
  `field_or`, `rest` for newtypes). `IntoJs` builds a map with the tag first (`tagged_value`). A newtype's record is
  turned into named fields through the schema's record descriptor (`tagged_inner`). Object fields inside variants get
  a JS wrapper (a `switch` on the tag) like record fields.
- **Supporting changes.** Records and unit enums inside a union arrive as plain objects and strings, so their `FromJs`
  now also reads a `Map` by field name and a string by variant name. `record_fields` takes `(name, optional)` pairs, and
  `enum_value` replaces `enum_index` in the derive. As a side effect, `HashMap<String, Record>` parameters work, which
  they did not before.
- **`exports.json`.** `kind: "union"`, `tag`, and `variants` (name, docs, fields, `inner`).

### Errors

- **Codes.** A value that does not fit throws `NativeError { domain: 'svelters', code }`, the same for `Json<T>` and
  for unions:
  - `type`;
  - `range`: serde's `invalid_value` with a number, or our integer `FromJs`;
  - `missing-field`;
  - `unknown-variant`;
  - `unknown-field`;
  - `invalid`: serde's `custom`.
- **Path.** The message starts with the path, `at l.shapes[1]: missing field \`h\``. The deserializer tracks map keys and
  list indices. Generated code adds field names (records, variants), list indices and map keys (`Vec`, `HashMap`
  `FromJs`), and the parameter name (the thunk, `__private::at`). Only `svelters`-domain value errors get a prefix.
- **Integer messages.** The integer `FromJs` range message now prints the number (`300 does not fit u8`) instead of
  `I64(300)`.

### SV24

- `Schema::typescript(&self)`, `json(&self)`, `javascript(&self, module, core, sibling)`, `dump(&self, dir)`.
- The free functions `exports::{typescript, json, javascript, dump}` are thin wrappers over `schema()`.
- `Schema::fingerprint` hashes `self.json()`.

### Facade

- `svelters::{Json, JsonAny}`, also in the prelude, plus a crate-doc section.
- `schemars = "1"` and `serde = "1"` are workspace dependencies. cadatomic uses schemars 1.2.

## Decisions made inside the design

1. **schemars is a plain dependency of svelters-host, not a feature** (ARCH-EMBED §8.6 said "when the `schemars`
   feature is on"). A feature would change `Json<T>`'s bound (`JsonSchema` or not), which is not additive under
   Cargo's feature unification. Instead there are two types: `Json<T>`, which needs `JsonSchema`, and `JsonAny<T>`,
   which needs only serde and is typed `unknown`.
2. **Error codes**: `NativeError` with domain `svelters` and the codes above, not JS `TypeError`/`RangeError`. Host
   argument checks (wrong primitive type, not an object) still throw `TypeError`/`RangeError` before Rust runs.
3. **Data enums use `#[derive(svelters::Enum)]`** (unit enums already did) with `#[js(tag)]`, rather than
   `#[derive(Record)]` on an enum as the note's sketch had it. The note's §5.2 example is updated.
4. **Variant tags are kebab-case** like unit enum values. serde's default is the variant name, but these are svelters
   types; serde types crossing as `Json<T>` keep their own serde names.
5. **Unions cross as maps with no host type entry.** There are no new ABI tags or `TypeDesc` kinds. The cost is that
   the host does not validate a union before Rust does, and Rust's errors carry the codes above.
6. **JSON definitions are declared per module**: each module that uses `Refusal` declares it. Module typings stay
   self-contained, with no cross-module imports of JSON types.

## Tests

- `crates/svelters/tests/json.rs` (log backend, real QuickJS host):
  - `json_values_round_trip`: a `Refusal`-like struct and an internally tagged serde enum round-trip Rust → JS → Rust,
    including a nested `serde_json::Value` and a list of `Json<T>`. Errors: a wrong tag (`unknown-variant` at
    `s.kind`), a missing tag, a missing field, `300` and `-1` for a `u8` (`range` at `r.attempts`), a wrong type, a
    bad unit-enum name, and a type error inside a tagged variant.
  - `tagged_enums_round_trip`: a `#[js(tag)]` enum with struct, unit and newtype (record with a unit enum) variants,
    inside a record and a list, and as an option. Errors: a wrong tag, a missing tag, a non-string tag, a missing field,
    `300` for a `u8` field, a wrong type, a bad unit-enum name in the newtype's record, a path through a list
    (`l.shapes[1]`) and through a `Json<T>` record field (`l.status`), and a non-object (`TypeError` from the host).
  - `typings_and_json_from_one_schema`: `Schema::typescript()`/`json()` equal the free functions. The typings declare
    the union, records, unit enums and the JSON definitions (in both modules that use them, closed over `$ref`s).
    `exports.json` has `union` (tag, variants, `inner`) and `json` (the schema) types.
- `svelters-host` unit tests `exports::json::tests`: `schema_ts` cases (nullable, tuples, maps, enums with quotes,
  `$ref` next to properties, `oneOf`) and path composition.
- `examples/exports`: a new `svelters:app/intent` module (`src/intent.rs`). The committed dump in
  `app/.svelters/app/` is regenerated. `app/src/intent.ts` narrows both unions with exhaustive switches and has
  `@ts-expect-error` cases (a wrong tag, a missing variant field, a missing serde field). svelte-check passes locally
  (0 errors, 93 files), and it reports a deliberately wrong field type, so the file is checked.

## Open items

- **Path inside serde's internally tagged/untagged/flattened types.** serde buffers their content, so an error's path
  stops at the enum (`at s: invalid type: string "big", expected f64`).
  **Required by cadatomic before its first package passes command args through `Json<T>`** (main-cadatomic,
  2026-10-22). Its command-path types are internally tagged serde enums: `invocation.rs` ("source"),
  `command/def.rs` ("state"), the executor ("intent"), `command/body.rs` ("kind", `deny_unknown_fields`),
  `action_bar` ("setting") and `lifecycle/acquire.rs` ("status"). Tell main-cadatomic if the fix changes
  `exports.json` or the error codes.
- **Name clashes between a JSON definition and a svelters type** of the same name in one module (a `Record` `Point`
  and a `JsonSchema` `Point`) produce a duplicate TS declaration. schemars dedups names only among JSON definitions.
- **Property order.** JSON definitions list their properties alphabetically, because schemars' maps are sorted.
  Enabling `preserve_order` would turn on serde_json's for the whole workspace.
- **Handles and buffers inside `Json<T>`** are not supported: serde reads plain data only. Objects (`Obj<T>`,
  `Arc<T>`) inside a tagged variant are wrapped in JS but untested. `Buffer<T>`/`Shared<[T]>` inside a variant are not
  supported (the variant slot copies typed arrays as bytes).
- **Nesting depth.** The host converts at most 16 levels (`MAX_DEPTH`), and that applies to `Json<T>` values too.
- **`#[js(bigint)]` inside variants** is rejected. Records nested inside a union lose their `bigint` fields' flag on the
  way out, because the host decodes maps without slots.
- **Run-time API descriptors (SV23)** can now take a `&Schema`, but a `Schema` is still built only from `inventory`.
  Building one from a descriptor is SV23's work.
- **Not run locally:** the GTK `examples/exports` tests (`committed_typings_are_current` included), the full workspace
  nextest, the docs build and the other gates. CI runs them.
