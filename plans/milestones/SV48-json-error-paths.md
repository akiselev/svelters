# SV48: full error paths through serde's buffered types; JSON name clashes

Request: cadatomic SV48 (main-cadatomic, 2026-10-22), RA2's first open item (`RA2-json-values.md`). cadatomic's
command-path types are internally tagged serde enums (tags "source", "state", "intent"; "kind" with
`deny_unknown_fields`; "setting", "status"), and it needs a bad value's error to name the field inside them before a
package passes command args through `Json<T>`. RA2's second open item (a JSON definition and a svelters type with the
same name) is fixed too.

## The problem

serde reads internally tagged, adjacently tagged (content before the tag), untagged and flattened types from a buffer:
the derive first reads the value into serde's private `Content`, then deserializes the variant from that buffer with
serde's own `ContentDeserializer`. Errors from that second step never pass through our deserializer, so the path stopped
at the buffered value: `at s: invalid type: string "big", expected f64`.

## Approach: find the error in the value, guided by the schema, on failure only

Two options were on the table. Both were weighed against what cadatomic needs (the right path and code, with serde
deciding acceptance exactly as before).

1. **A path-tracking deserializer through `Content`** (`serde_path_to_error` style). It does not work from the outside:
   `Content` and `ContentDeserializer` are private to serde, and the derive calls them directly, so no wrapper sees the
   keys once the content is buffered. `serde_path_to_error` has the same limitation. Doing it anyway would mean
   reimplementing serde's tagged and untagged visitors, which would not be the derive's code path any more.
2. **Pre-validation against the schemars schema.** Validating before serde runs would change behaviour: wherever the
   schema and serde disagree (custom `Deserialize`, `deserialize_with`, `try_from`, a schema stricter or looser than
   serde), a value serde accepts could be refused, or the reported error would not be serde's. It also costs a schema
   walk on every call.

**Chosen: a variant of 2 that runs only after serde has failed and only places the error.** serde still decides
acceptance and the error's kind and message. The schema (the same one the typings come from) only says where the error
belongs. Successful calls cost nothing extra, and a schema that disagrees with serde can at worst leave the path where
it was before.

### As built (`crates/svelters-host/src/exports/json.rs`, `json/locate.rs`)

- **Which errors need placing.** The deserializer now borrows the `Value` (so it is still there after a failure) and
  keeps a thread-local stack of *frames*: one per value it hands to serde (the root, list elements, object values,
  variant content). Each `DeError` records at creation whether it is `exact`: made while the deserializer was visiting
  the innermost frame's value, and before that object's entries ran out. Errors serde makes from buffered content after
  the deserializer returned (internally tagged, untagged) or after the object was drained (flatten, a missing field)
  are not `exact`.
- **Hints.** Each `DeError` also keeps what it is about: the `Unexpected` value as serde prints it, the length, or the
  missing/unknown field or variant name, or `custom`.
- **Placing.** `locate` walks the value below the path the error already has, in the value's own order (serde stops at
  the first error in the same order), with the JSON Schema alongside: `$ref`, `allOf` (conjunctions), `oneOf`/`anyOf`
  (the branches whose `type`, tag `const`s and `required` keys fit the value, or all branches if none fits),
  `properties`/`additionalProperties`, `prefixItems`/`items`. It takes the first value that explains the error:
  - `type`/`range`/`invalid` value: the value prints like serde's `Unexpected` and the schema rejects it (type, const,
    enum, bounds);
  - `missing-field`: an object that lacks the field and whose schema requires it;
  - `unknown-field`: an object with the key whose schema is closed (`additionalProperties`/`unevaluatedProperties:
    false`) and does not declare it; the path ends with the key, as for direct errors;
  - `unknown-variant`: a string (a tag or a unit variant) the schema's `const`/`enum` rejects, or a one-key object;
  - `invalid` from an untagged enum: the innermost value with a `oneOf`/`anyOf` that the value does not fully validate
    against (a small validator for the keywords above). Other `custom` errors keep their path.
  A first pass needs the schema to confirm; a second pass matches on the value alone (for a schema that does not
  describe what serde checks). If neither finds anything, the path stays as it was.
- **Exact errors are checked too.** serde's adjacently tagged visitor reads buffered content (content before the tag)
  *before* it drains the object, so such errors look exact. An exact error keeps its path only if the value there shows
  it; otherwise it is placed like the others. The common case (a wrong value the deserializer saw itself) is confirmed
  without a schema (`locate::settled`), so the schema is generated (`into_root_schema_for::<T>()`) only for errors
  that need it.
- **`Json<T>`** uses `from_value_schema`. **`JsonAny<T>`** (no schema) uses `from_value`, which places errors by the
  value alone: everything except a missing field, which stays at the buffered value.
- **Bounds.** The walk stops after 20,000 steps, 64 alternatives per value, and depth 64.

### Name clashes (RA2's second open item)

A JSON definition whose TS name equals a svelters type or class name (`Record` `Amount` and serde `Amount`) is declared
as `AmountJson`; its references use the new name. The schema build is a pure function of the `inventory` registrations,
and the svelters names are known only after everything is described, so `build()` builds once, and if any definition
clashes, builds again with the svelters names reserved (`TypeBuilder::json_reserved`, `json::json_ident`). Reservation
is global, not per module: a definition is renamed everywhere if any module has a svelters type of that name, so one
name means one type across modules. Apps without a clash build once and get the same output as before.

## Compatibility (main-cadatomic asked)

- **Error codes: unchanged.** Same `NativeError { domain: 'svelters', code }`, same six codes, same message text after
  the path. Only the path gets longer where it used to stop at a buffered value. One existing expectation changed
  accordingly: `at s: invalid type: string "big", expected f64` is now `at s.value: …`.
- **`exports.json`: unchanged** for apps without a name clash (the committed `examples/exports` dump has none and is
  unchanged). With a clash, the `kind: "json"` type's `name` (and the TS name) is `<Name>Json`; before, the output
  declared the name twice and did not type-check.

## Tests

- `crates/svelters/tests/json.rs` `errors_inside_buffered_serde_types_have_full_paths` (log backend, real QuickJS
  host), with cadatomic-shaped types: `Invocation` (tag `source`) → `Args` → `Body` (tag `kind`,
  `deny_unknown_fields`) → `Extent` (tag `mode`), plus an untagged `Amount`, an adjacently tagged `Setting` and a
  `Placement` with a flattened struct and a flattened internally tagged enum. Each asserts the full message and code:
  - internally tagged in internally tagged: `type` (`at args.body.extent.depth`), `range` (`args.body.count`),
    `missing-field` (`args.body.extent`, also a missing tag), `unknown-variant` (`args.body.extent.mode`), an
    `Option` field;
  - `deny_unknown_fields`: `unknown-field` at `args.body.extra`;
  - lists inside buffered content: `args.body.at[1][1]`, and a wrong length at `args.body.at[0]`;
  - untagged: `invalid` at `args.amount`;
  - adjacently tagged: content after the tag, content before it (`type` and `missing-field` at `args.setting.c…`),
    an unknown tag;
  - flattened: a struct field (`args.placement.x`), a flattened enum's missing field and wrong type;
  - three levels of buffering through `Invocation` (`inv.args.body.extent.depth`, `unknown-field`, untagged,
    adjacently tagged);
  - `JsonAny`: placed by value; a missing field stays at the buffered value.
- `typings_and_json_from_one_schema`: the clash case (`export interface Amount` once, `export type AmountJson`,
  `amount?: AmountJson | null`, `exports.json` type `AmountJson`).
- `svelters-host` unit tests `exports::json::locate::tests`: placing with handwritten schemas (nested tags, closed
  variants, a value the schema accepts before the wrong one, the value-only pass, untagged, nothing found).
- Run locally: `cargo nextest run -p svelters --test json` (4 tests) and `-p svelters-host --lib exports::json`
  (4 tests). Not run locally: the full workspace nextest, the GTK `examples/exports` tests, the docs build and the
  other gates; CI runs them.

## Open items

- Placement is a search, not a trace. When two values in the buffered content would show the same error (the same
  wrong string in two fields the schema both rejects), the first in the value's order is chosen; serde reads in the
  same order, so this matches for derived types. A hand-written `Deserialize` that reads fields out of order could be
  placed on the wrong one.
- A `custom` error other than an untagged mismatch (`try_from`, validation in a hand-written impl) inside buffered
  content keeps the path to the buffered value.
- A missing field inside buffered content of a `JsonAny<T>` (no schema) keeps the path to the buffered value.
