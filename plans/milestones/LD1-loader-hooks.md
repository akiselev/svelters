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

## Decided (coordinator, 2026-10-22)

- Native ops from dlopen'd libraries (kinds, handles, calls) are a separate later milestone on the `svl_pack` ABI (CUSTOM-WIDGETS). LD1 stays a loader with hooks only; `DylibModules` loads JS modules only.
- `svelters:app/*` visibility defaults to `All` for the primary host (`ModuleHooks::default()`); cadatomic sets `Allow` for its own hosts.
- Amendment (main-cadatomic, accepted): visibility fails closed for isolates. Any extension or isolate host uses `ModuleHooks::isolate()`, which defaults to `Allow([])`: an isolate host nobody configured must not see every app export. The host has no isolate concept yet, so the flag is this constructor; the code that creates an isolate host calls it.

- User decision (2026-10-22, relayed by the coordinator): extensions run under a deny-by-default, auditable security policy; SEC0 (`ARCH-SECURITY.md`) designs it. LD1 leaves the hooks only. `ExtensionPolicy` (`#[non_exhaustive]`, in `svelters-modules/src/compose.rs`) holds `visibility` and `native_modules` (`NativeAccess`: `All`, `Allow(ids)`, `Deny`) and grows with SEC0. `ExtensionPolicy::open()` is the primary host; `deny_all()` is `ModuleHooks::isolate()`: no app module visible, no added native module loadable. The gate covers sets added with `ModuleHooks::natives` (dlopen'd, embedder's); the app's own exports are governed by visibility. SEC0 slot: a file-system scope. `SourceProvider` loads any path it resolves, so an extension host must reach it only through a `ComposedProvider` that carries the policy.

- Security fix (coordinator, from ARCH-SECURITY §5): `construct('app:mod.T')` and `callStatic('app:mod', ..)` reach an export with no import, so they are gated by the same policy. `Host::set_module_hooks` also sets `Core::app_gate` (the policy's visibility, importer `""`); `app_construct` and `app_call_static` check it (`apps.rs`, `app_gate_kind`) and fail with the same `unknown kind` text as a missing export, listing only the visible modules. Denied by default for `ModuleHooks::isolate()`. `ComposedProvider::load` re-checks the visibility of app exports too (not for `Visibility::Filter`, which needs an importer and is checked at `resolve`).
- Deferred to SEC1 (not one-line hooks): a `principal` on `ModuleHooks` passed to `Visibility::Filter` and `SchemeHook`; a typed internal denial carrying the capability (the JS message stays "no such module"); an on-evaluate callback (id, principal, content hash); gating instance methods and properties of objects a visible export returns from a hidden module; the `Policy` gate in `ExtensionPolicy`.

## Decided (LD1)

- Q3 option (b), as recommended in TRACKS §6: static modules always, dlopen behind the `dlopen` feature, which cadatomic leaves off. The feature adds `libloading` (already in the lockfile).
- The default `ModuleHooks` serves every export with no filter, so existing apps behave as before.
- Hidden and absent `svelters:app/*` modules give one error text (`no such module: `id``).

## Not in LD1

- A dlopen'd library's native *ops* (kinds, handles, calls): `DylibModules` serves module source only, so a library's JS can use the existing host ops. Native kinds from a library need the pack ABI (CUSTOM-WIDGETS `svl_pack`), which is its own milestone.
- Per-importer visibility by package (the `Filter` closure gets the importer id; a policy language on top is for cadatomic).
- Bytecode for hook-served modules: a hook may return `ModuleCode::Cached`, which `declare()` already reads.

## Tests

- `svelters-modules`: `compose::tests` (routing, visibility, hidden-looks-absent, reserved/duplicate schemes).
- `crates/svelters/tests/exports.rs::ops_without_an_import_obey_the_extension_policy`: `construct` and `callStatic` of an app kind are denied for `isolate()` and work once the module is allowed (and by default on the primary host).
- `compose::tests::load_rechecks_visibility_of_app_exports`.
- `crates/svelters-host/tests/hooks.rs`: both policy defaults (primary sees all; `isolate()` sees no app module and loads no added native module until allowed, and a visible module stays denied while the native gate is closed); a scheme and an extra native module load through a `Host`; an unregistered scheme is a load error; the filter hides a module; a reserved scheme is refused.
- Not tested: `DylibModules` (needs a built cdylib fixture; checked with `cargo check --features dlopen` only).
