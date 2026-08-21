# Canonical native API IR

The native API IR is the common semantic database consumed by GTK, QtWidgets, generated TypeScript declarations, the binding linker, release thunk generators, docs, and compatibility tooling. Backend extractors must not invent private parallel models.

## Stable identity

Every entity has a canonical textual identity and a 128-bit stable ID derived from a versioned domain-separated hash.

Examples:

```text
svelters-api:v1:type:qt:QtWidgets:QTreeView
svelters-api:v1:method:qt:QtWidgets:QTreeView::scrollTo(QModelIndex,QAbstractItemView::ScrollHint)
svelters-api:v1:signal:gtk:Gtk:Button::clicked()
```

Hash canonical UTF-8 with BLAKE3 and truncate to 128 bits. Collision detection is mandatory while building a database. Dense u16/u32 IDs are build-local optimization only and never persisted as API identity.

## Core records

The implementation should model at least:

```rust
ApiDatabase { schema_version, producer, toolkit, target, types, callables, properties, signals, converters, adapters }
NativeType { id, name, canonical, kind, bases, interfaces, availability, ownership_defaults, child_policy }
Callable { id, owner, name, kind, params, result, qualifiers, invocation, availability, dependencies, annotations }
Parameter { name, ty, direction, nullable, ownership, default, callback_scope }
Property { id, owner, name, ty, readable, writable, notify, invocation }
Signal { id, owner, name, params, result, connection_semantics }
Converter { id, from, to, direction, implementation, dependencies }
StructuralPolicy { policy, slots, adapter }
```

`TypeKind` distinguishes object handles, value types, primitive scalars, strings, enums/flags, callbacks, containers, opaque values, borrowed views, and toolkit-specific aliases.

## Invocation classification

```text
Reflected       generic property/metaobject path
Introspected    runtime metadata callable, primarily GIRepository
GeneratedThunk  concrete native C/C++/Rust code required
CustomAdapter   handwritten/curated semantic bridge
RendererOp      immediate-mode or portable renderer operation
Unsupported     known but intentionally unavailable, with reason
```

This classification is data. The binding linker may select a different implementation by profile, e.g. GTK `Introspected` in dev and `GeneratedThunk` in release.

## Ownership and lifetime

Do not reduce ownership to a boolean. Minimum vocabulary:

```text
Borrowed
OwnedByCaller
OwnedByCallee
ParentOwned
ToolkitOwned
TransferFull
TransferContainer
SharedRefCounted
Ephemeral
Unknown
```

Include nullable, const/mutability, callback scope, thread affinity, and invalidation conditions. Backend-specific annotations may preserve source facts, but runtime decisions must map to canonical semantics.

All toolkit objects exposed to JS use `NativeHandle { backend, slot, generation }`. Opaque value handles are host-owned. Small value types may later gain tagged inline representations after measurement.

## Structural semantics

Widget containment is not a generic `appendChild`. Each type may define:

```text
StandardContainer
SingleChild
LayoutManaged
Indexed
NamedSlots
ModelDriven
Custom(adapter_id)
```

Named slots carry accepted child type constraints and insertion/removal operations. The portable widget layer compiles to these same semantics.

## Serialization

The canonical interchange form is deterministic JSON for inspection, diffing, licensing/provenance, and Artifactum storage. A compact runtime table is derived from it. Do not make an unstable Rust binary layout the persisted format.

Every database records:

- extractor name/version/commit;
- toolkit version and headers/typelibs identity;
- target triple/ABI-relevant configuration;
- source typesystem/override hashes;
- license/provenance metadata;
- schema version.

## Generated TypeScript

The same IR generates native toolkit `.d.ts` packages. Object methods retain overloads and return types. Native widget component props/signals are generated separately from imperative object APIs so Svelte completion can distinguish markup attributes from `bind:this` methods.

The generated declarations are also an input to native-call resolution; therefore declaration generation and binding resolution must share the same canonical IDs and overload model.

## Compatibility diff

`svelters api diff old.json new.json` classifies:

- added/removed types;
- signature changes;
- ownership/lifetime changes;
- availability/version changes;
- invocation-strategy changes;
- structural-policy changes.

Ownership or structural changes are breaking even when the C++/C signature is textually unchanged.
