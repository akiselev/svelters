# SVR07 - canonical native API IR and generated typings

References: `plans/architecture/native-api-ir.md`.

## Goal

Implement the shared schema that prevents GTK, QtWidgets, egui, TypeScript generation, and binding codegen from becoming separate incompatible systems.

## Work

Create `svelters-api-schema`, `svelters-api-db`, and CLI support for validate/normalize/diff/query.

Implement versioned canonical JSON, BLAKE3 domain-separated stable 128-bit IDs, collision checks, deterministic ordering, source provenance, ownership/lifetime records, invocation classification, structural child policy, converter dependency graph, toolkit availability, and target metadata.

Create a small hand-authored synthetic API fixture containing inheritance, overloads, properties, signals, callbacks, enums/flags, ownership transfer, named child slots, and unsupported methods. Use it to develop consumers before GTK/Qt extractors exist.

Generate TypeScript from the same IR:

- imperative native object interfaces;
- Svelte markup/component props/events;
- canonical binding-ID annotations in machine-readable side metadata;
- version availability annotations.

Do not expose backend internals as TypeScript `any` simply to get early demos working.

## Acceptance

- canonical JSON round-trips and stable IDs are deterministic across declaration order;
- `api diff` detects ownership and structural changes as breaking;
- synthetic API generates valid `.d.ts` and resolves inherited overloads;
- golden tests prevent accidental canonicalization/ID drift.
