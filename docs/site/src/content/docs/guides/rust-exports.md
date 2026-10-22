---
title: Rust exports
description: Calling an app's own Rust types and functions from Svelte through svelters:app/<module>, the value types that cross, serde values with Json<T>, tagged unions, and the errors a bad value gives.
---

An app can expose its own Rust code to Svelte without writing bindings by hand. The `svelters` crate's macros register
types and functions, and the host serves each module as `svelters:app/<module>`. Exports live in the host, not in a
backend, so they work the same on GTK, Qt, AppKit, WinUI and egui.

```rust
use svelters::prelude::*;

#[derive(svelters::Record)]
pub struct Params { pub step: u32, #[js(default = 1.0)] pub scale: f64 }

#[svelters::export(module = "demo")]
pub fn greet(name: &str) -> String { format!("hello {name}") }
```

```svelte
<script>
  import { greet } from 'svelters:app/demo';
</script>

<label label={greet('Svelte')} />
```

`svelters::exports::dump(dir)` writes `exports.json`, `app.d.ts` and one stub per module. Point `tsconfig.json` at
`app.d.ts` and svelte-check type-checks the Svelte code against the binary it runs on. A debug host can call
`dump_if_debug` on start so the typings follow every Rust rebuild.

## Values

| Rust | JS | TypeScript |
|---|---|---|
| `bool`, integers, `f32`, `f64` | boolean, number | `boolean`, `number` (integers are range-checked; `#[js(bigint)]` gives `bigint`) |
| `String`, `&str` | string | `string` |
| `Option<T>` | value or `null` | `T \| null` |
| `Vec<T>`, `&[T]` | array | `T[]` |
| `HashMap<String, T>`, `BTreeMap<String, T>` | object | `Record<string, T>` |
| `#[derive(svelters::Record)] struct` | plain object, copied | `interface` |
| `#[derive(svelters::Enum)] enum` (unit variants) | string | `'a' \| 'b'` (kebab-case) |
| `#[derive(svelters::Enum)] #[js(tag = "kind")] enum` | plain object with a tag | discriminated union |
| `Json<T>` (serde and `schemars::JsonSchema`) | plain value | from `T`'s JSON Schema |
| `JsonAny<T>` (serde only) | plain value | `unknown` |
| `#[svelters::object]` types, `Arc<T>`, `Obj<T>` | object with methods | `class` |
| `Buffer<T>`, `Shared<[T]>` | typed array | `Float32Array`, `ReadonlyFloat32Array`, … |

## Tagged unions

A data enum with `#[js(tag = "kind")]` crosses as a plain object whose `kind` names the variant. This is serde's
internally tagged form. A variant can be a unit, a struct, or a newtype around a record, whose fields then sit next to
the tag.

```rust
#[derive(svelters::Enum)]
#[js(tag = "kind")]
pub enum Pick {
    Face { index: u32 },
    Edge { index: u32, both_sides: bool },
    Nothing,
}
```

```ts
export type Pick = { kind: 'face'; index: number; } | { kind: 'edge'; index: number; bothSides: boolean; } | { kind: 'nothing'; };
```

Tags are kebab-case and fields camelCase, as for records and unit enums. `#[js(rename)]` changes either.
`Option` and `#[js(default)]` fields are optional.

## Serde values

Some types cannot derive `svelters::Record`. Their crate may have to stay headless, or the orphan rule may forbid a
`JsType` impl in the crate that exports them. Those types can cross as `Json<T>`, provided they implement `Serialize`,
`Deserialize` and `schemars::JsonSchema`. The value crosses as a structured JS value, never as JSON text. serde reads
it on the Rust side.

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotStatus { Empty, Filled { value: f64 }, Refused(Refusal) }

#[svelters::export(module = "intent")]
pub fn check(slot: String, value: Option<f64>) -> Json<SlotStatus> { /* … */ }
```

The typings come from the type's JSON Schema. Every named schema a module uses is declared in that module, so the union
above narrows in TypeScript like any other:

```ts
import { check, type SlotStatus } from 'svelters:app/intent';

function label(s: SlotStatus): string {
  switch (s.kind) {
    case 'empty': return 'empty';
    case 'filled': return `value ${s.value}`;
    case 'refused': return s.message;
  }
}
```

`exports.json` carries each JSON Schema as well. Use `JsonAny<T>` for a serde type without a `JsonSchema`; it is typed
`unknown`.

## Errors

A value that does not fit its Rust type throws a `NativeError` with `domain: 'svelters'`. The `code` says what was
wrong, and the message starts with the path from the parameter, for example ``at layer.shapes[1]: missing field `h` ``.

| `code` | Cause |
|---|---|
| `type` | a value of the wrong type |
| `range` | an integer out of range for its Rust type |
| `missing-field` | a required field or the tag is missing |
| `unknown-variant` | a tag or enum name that names no variant |
| `unknown-field` | an extra field on a serde type with `deny_unknown_fields` |
| `invalid` | serde rejected the value (a `try_from` or a custom check) |

Errors that Rust returns on purpose, through `#[derive(svelters::Error)]`, use the module's domain, `app:<module>`.
