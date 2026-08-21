# SVR21 - platform services, artifact caches, external tooling, and packaging

References: `plans/architecture/ecosystem-integrations.md`.

## Goal

Make expensive generated data reproducible and cached, provide a clean optional native-application storage service, keep optional build tools out of the core dependency graph, evaluate a resident development helper only if justified, and produce distributable desktop applications.

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

## Optional appfs platform service

Before integration, validate the current appfs repository according to its own `AGENTS.md`: compile it, fix any dependency/API drift, run provider conformance, black-box observation and crash-recovery tests. Svelters must not promote an unvalidated generated archive to a foundational dependency.

After validation, provide an optional host module/package (working name `@svelters/appfs`) exposing semantic application spaces rather than raw ambient filesystem access:

```text
Config  Data  State  Cache  Runtime  Temp  Logs  Resources  External
```

Use Promise-based JS APIs and never block the UI thread on durable I/O. Native application resources may come from appfs Resources; settings/restoration may use Config/State; External represents explicitly granted user files/directories. The Svelters renderer, graph and QuickJS core remain independent of appfs, and applications may choose another storage layer.

Do not turn appfs into a secrets/keychain abstraction; preserve its own semantic boundary.

## Outboard

Use Outboard for independently installed tooling implementations where a process boundary is appropriate, for example extractor frontends, packagers, signing/notarization helpers and optional API-report generators. Do not move renderer frame/event traffic onto Outboard JSON workers.

## Optional daemonkit evaluation

SVR06 deliberately uses a simple authenticated direct dev transport. Measure cold/repeated startup, compiler/API-DB cache reuse and multi-app workflows. Only if a resident helper has a material benefit, add `svelters-devd` with daemonkit owning lifecycle, authenticated local endpoint publication, stale-state repair and crash cleanup. Svelters/Vite owns the application protocol.

If the measurements do not justify a daemon, record the decision and do not add daemonkit.

## Packaging

Create a Svelters application bundle format and platform packagers for the supported matrix. Distinguish system-shared GTK/Qt deployments from self-contained toolkit packaging. Include licenses/notices, API/binding provenance, source maps according to profile, and reproducible manifest hashes.

## Acceptance

- a clean machine can reproduce or fetch a pinned Qt API DB and binding pack by artifact identity;
- cache hits demonstrably skip expensive extraction/native bridge builds;
- if appfs integration is enabled, its upstream validation protocol is green and a reference app can persist Config/State, read packaged Resources, and exercise an explicit External resource without blocking the UI thread;
- daemonkit is either adopted with measured justification and crash/restart tests or explicitly rejected with measurements;
- build/package the reference app for at least Linux GTK, Linux QtWidgets and Linux egui, with Windows/macOS Qt/egui packaging jobs added where CI runners/toolkits are available.
