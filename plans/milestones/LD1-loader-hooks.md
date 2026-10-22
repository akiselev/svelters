# LD1: loader hooks and native modules

Spec: `plans/research/TRACKS.md` §2.1 (LOADER, row LD1), `plans/research/QUICKJS-RUNTIME.md` §3 and §6,
`plans/research/CADATOMIC.md` SV22 and SV25. Base: Q1 (`svelters-modules`, `ModuleProvider`).

## As built

| Piece | Where | What |
| --- | --- | --- |
| `SchemeHook` (SV22) | `svelters-modules/src/compose.rs` | A host-registered scheme (`cadatomic:`): `resolve(base, spec)` (default: identity) and `load(id)`, both synchronous like QuickJS's loader. `svelters` is reserved; a scheme registers once. |
| `Visibility` (SV25) | `compose.rs` | `All`, `Allow(names)`, `Deny(names)` or `Filter(module, importer)` over `svelters:app/*`. `svelters:core` is always visible. A hidden module fails with the same error as an absent one, so the filter does not reveal what it hides. |
| `ComposedProvider` | `compose.rs` | Wraps any `ModuleProvider` (`SourceProvider`, `BundleProvider`). Routes `svelters:core`/`svelters:app/*` to the native registry (through the filter), a registered scheme to its hook, the rest to the inner provider. Other trait methods delegate. |
| `NativeModules`, `NativeRegistry`, `StaticModules` | `svelters-modules/src/native.rs` | A set of native modules (ids + ES source); the registry asks sets in order. |
| `ExportModules`, `ModuleHooks` | `svelters-host/src/hooks.rs` | `ExportModules` serves the E0 `inventory` exports (`svelters:core`, one `svelters:app/<module>` per schema module) through `exports::builtin_module`. `ModuleHooks` is the builder: `.scheme()`, `.visibility()`, `.natives()`. |
| `Host::set_module_hooks` | `svelters-host/src/lib.rs` | `load_from`, `load_entry_from` (and so `load_bundle`/`load_entry`) wrap the given provider in a `ComposedProvider`. The reloader stores the composed provider, so hot reload keeps the hooks. |
| `quickjs.rs` | `svelters-host/src/quickjs.rs` | The `is_builtin` short-circuit in the resolver and loader is gone: QuickJS asks the provider for everything. |
| `DylibModules` | `svelters-modules/src/dylib.rs`, feature `dlopen` (default off) | `DylibModules::open(path)` dlopens a library that exports `svl_modules_v1() -> *const c_char`, a static NUL-terminated JSON object `{ "svelters:app/<name>": "<ES module source>" }`. Ids outside `svelters:app/` are refused. Register it with `ModuleHooks::natives`. |

## Decided

- Q3 option (b), as recommended in TRACKS §6: static modules always, dlopen behind the `dlopen` feature, which cadatomic leaves off. The feature adds `libloading` (already in the lockfile).
- The default `ModuleHooks` serves every export with no filter, so existing apps behave as before. A stricter default is a caller's choice.
- Hidden and absent `svelters:app/*` modules give one error text (`no such module: `id``).

## Not in LD1

- A dlopen'd library's native *ops* (kinds, handles, calls): `DylibModules` serves module source only, so a library's JS can use the existing host ops. Native kinds from a library need the pack ABI (CUSTOM-WIDGETS `svl_pack`), which is its own milestone.
- Per-importer visibility by package (the `Filter` closure gets the importer id; a policy language on top is for cadatomic).
- Bytecode for hook-served modules: a hook may return `ModuleCode::Cached`, which `declare()` already reads.

## Tests

- `svelters-modules`: `compose::tests` (routing, visibility, hidden-looks-absent, reserved/duplicate schemes).
- `crates/svelters-host/tests/hooks.rs`: a scheme and an extra native module load through a `Host`; an unregistered scheme is a load error; the filter hides a module; a reserved scheme is refused.
- Not tested: `DylibModules` (needs a built cdylib fixture; checked with `cargo check --features dlopen` only).
