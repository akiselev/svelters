# SVR21 - platform services, artifact caches, external tooling, and packaging

References: `plans/architecture/ecosystem-integrations.md` and `plans/architecture/application-storage.md`.

## Goal

Make expensive generated data reproducible and cached, complete platform-specific application/resource integration on top of the appfs storage substrate established in SVR05, keep optional build tools out of the core dependency graph, evaluate a resident development helper only if justified, and produce distributable desktop applications.

## Artifactum

Model as reproducible derived artifacts:

- Qt/Shiboken API databases;
- GTK typelib/GIR API databases;
- generated `.d.ts` packages;
- Qt fat development bridges;
- release thin binding packs;
- QuickJS release module/bytecode bundles;
- API compatibility reports.

Action keys include exact upstream/toolkit/extractor versions, target, compiler flags, typesystem hashes and generator source identity. CI can fetch/cache artifacts but must be able to reproduce them from declared inputs.

Artifactum remains distinct from appfs: immutable/reproducible build outputs belong in Artifactum, while mutable application config/data/state/cache/runtime/logs and packaged resource access belong behind the appfs-backed Svelters storage service.

## Appfs platform/resource completion

Do not introduce appfs here; SVR05 already validated and established it as the default Svelters application-storage substrate. This phase completes the platform-facing pieces needed for distributable applications:

- verify `appfs-native::standard_appfs`/equivalent platform resolution on every supported desktop target;
- map packaged application assets into the Resources space without exposing bundle-installation paths as portable API;
- define development resource overlays with deterministic precedence and clear read-only/write semantics;
- integrate file/folder pickers and platform permissions with explicit External capabilities rather than ambient filesystem authority;
- ensure settings, window/session recovery and other framework-owned persistence use Config/State rather than backend-specific path helpers;
- ensure runtime sockets/leases/dev-session files use Runtime and logs use Logs;
- document backup/purge/migration expectations for each semantic space;
- package appfs/provider license notices and record the exact validated appfs version in release metadata.

Applications may still use other storage libraries for their own domain-specific needs, but Svelters-provided platform APIs and framework-owned persistence must not create a second competing path/storage convention.

Do not turn appfs into a secrets/keychain abstraction; preserve its own semantic boundary and add a separate platform secret service later if required.

## Outboard

Use Outboard for independently installed tooling implementations where a process boundary is appropriate, for example extractor frontends, packagers, signing/notarization helpers and optional API-report generators. Do not move renderer frame/event traffic onto Outboard JSON workers.

## Optional daemonkit evaluation

SVR06 deliberately uses a simple authenticated direct dev transport. Measure cold/repeated startup, compiler/API-DB cache reuse and multi-app workflows. Only if a resident helper has a material benefit, add `svelters-devd` with daemonkit owning lifecycle, authenticated local endpoint publication, stale-state repair and crash cleanup. Svelters/Vite owns the application protocol.

If the measurements do not justify a daemon, record the decision and do not add daemonkit.

If daemonkit is adopted, its private lifecycle/runtime state must live consistently with the Svelters Runtime-space policy rather than introducing another unrelated user-visible directory convention.

## Packaging

Create a Svelters application bundle format and platform packagers for the supported matrix. Distinguish system-shared GTK/Qt deployments from self-contained toolkit packaging. Include licenses/notices, API/binding provenance, source maps according to profile, resource manifests, storage schema/version metadata where relevant, and reproducible manifest hashes.

Packaged Resources must be addressable through the same `@svelters/storage` resource API used in development. An application must not need conditional code to locate an icon/module/data asset in dev versus a packaged release.

## Acceptance

- a clean machine can reproduce or fetch a pinned Qt API DB and binding pack by artifact identity;
- cache hits demonstrably skip expensive extraction/native bridge builds;
- the already-established appfs integration works on the supported platform matrix: a reference app persists Config/State, reads packaged Resources, uses Runtime/Logs, and exercises an explicit External resource without blocking the UI thread or exposing hard-coded install/config paths;
- development and packaged builds resolve the same logical Resource keys to identical bytes;
- deleting Cache/Runtime does not corrupt durable Config/Data/State, and packaging/install location changes do not invalidate application storage identity;
- daemonkit is either adopted with measured justification and crash/restart tests or explicitly rejected with measurements;
- build/package the reference app for at least Linux GTK, Linux QtWidgets and Linux egui, with Windows/macOS Qt/egui packaging jobs added where CI runners/toolkits are available.
