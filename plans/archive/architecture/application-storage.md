# Application storage and resource substrate

Svelters uses `appfs` as the default application-storage substrate. This decision is foundational for host/runtime APIs but deliberately independent of rendering: the logical graph, Svelte runtime overlay, and renderer backends do not depend on storage semantics.

## Why appfs is the default

Desktop frameworks otherwise accumulate ad hoc platform paths and subtly different persistence rules for settings, recovery state, caches, runtime sockets, logs, bundled assets, and user-granted files. Svelters must not grow its own second implementation of those concerns.

The host owns one application-scoped `AppFs` instance and mounts semantic spaces appropriate to the platform and deployment profile:

```text
Config     durable user/application configuration
Data       durable user/application data
State      recoverable internal/session/checkpoint state
Cache      purgeable regenerated data
Runtime    ephemeral sockets, locks, dev-session coordination
Temp       scratch data
Logs       operational logs
Resources  immutable packaged application assets
External   explicitly granted user files/directories
```

The exact durability and provider capabilities remain appfs policy, not duplicated Svelters enums.

## Validation gate

The current appfs repository must pass its own validation protocol before Svelters promotes it to a required release dependency. The Svelters phase that first integrates appfs must:

1. compile appfs on the pinned Svelters toolchain or document the compatible toolchain boundary;
2. run appfs provider conformance tests;
3. run its black-box probe and crash-recovery stress tests;
4. record any fixes upstream in appfs rather than carrying private copies;
5. pin a validated appfs revision/version.

Until that gate passes, an in-memory/native minimal storage shim may exist only inside tests/bootstrap work. It must implement the Svelters storage service boundary and be deleted once the validated appfs dependency lands; it must not become a second public storage API.

## Host ownership and JavaScript capability model

The native host owns `AppFs`. JavaScript never receives ambient filesystem authority or arbitrary absolute host paths through this API.

Expose a Promise-based host module, initially `@svelters/storage`, backed by appfs semantic spaces:

```ts
const settings = await storage.config.get('settings.json');
await storage.state.put('session.json', bytes);
const icon = await storage.resources.get('icons/save.svg');
```

`External` is the only normal path for user-selected/granted files and directories. Platform file pickers return capabilities/handles which can be mounted or represented through the External space rather than silently granting general filesystem access.

Durable I/O must never block the UI thread. The JS-facing API is asynchronous even when a provider can complete synchronously.

## Framework-owned data

Svelters itself uses the same semantic substrate for framework state:

- `Runtime`: HMR/control endpoint metadata, ephemeral session files, local development coordination;
- `Cache`: regenerable runtime caches which are not Artifactum build artifacts;
- `State`: crash/HMR restoration snapshots when application/runtime state must survive a process restart;
- `Logs`: host/runtime logs when file logging is enabled;
- `Resources`: packaged JS modules, source maps, icons/assets and other immutable application resources where the packaging profile uses appfs-backed resources.

Framework-owned keys live under a reserved `.svelters/` namespace so application keys do not collide with runtime internals.

## What does not belong in appfs

- Reproducible derived build products such as Qt/GTK API databases, generated typings, QuickJS bytecode bundles and thin binding packs belong in Artifactum.
- Live renderer/plugin generations belong in Switchyard.
- Secrets and OS keychain credentials are not modeled as files; use a future explicit secret/keychain service.
- Renderer-native widget state remains in the canonical Svelters graph and renderer realization, with only deliberate restoration/checkpoint data persisted to State.

## Testability

Use `appfs-memory` for headless runtime tests. This allows TestHost/QuickJS fixtures to verify settings/state/cache/resource behavior without host filesystem access and makes storage effects observable in deterministic tests.

At least the following invariants are required:

- Config/Data/State writes request durability appropriate to their semantic intent rather than silently degrading;
- Cache/Temp/Runtime data may be discarded without corrupting durable application state;
- packaged Resources are read-only from application JavaScript unless the provider explicitly represents a development overlay;
- External access is explicit and capability-scoped;
- no host-internal absolute path leaks into portable JS APIs;
- storage shutdown/flush participates in application shutdown ordering.

## Dependency boundary

The intended dependency shape after the validation gate is:

```text
svelters-host / svelters-storage
    -> appfs
    -> appfs-native (desktop production)

headless tests
    -> appfs-memory

renderers / graph
    -X-> appfs
```

This makes appfs the official Svelters application-storage implementation without coupling storage to rendering.
