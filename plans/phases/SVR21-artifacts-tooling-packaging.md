# SVR21 - Artifactum caches, Outboard tooling, and packaging

## Goal

Make expensive generated data reproducible and cached, keep optional build tools out of the core dependency graph, and produce distributable desktop applications.

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

## Outboard

Use Outboard for independently installed tooling implementations where a process boundary is appropriate, for example extractor frontends, packagers, signing/notarization helpers and optional API-report generators. Do not move renderer frame/event traffic onto Outboard JSON workers.

## Packaging

Create a Svelters application bundle format and platform packagers for the supported matrix. Distinguish system-shared GTK/Qt deployments from self-contained toolkit packaging. Include licenses/notices, API/binding provenance, source maps according to profile, and reproducible manifest hashes.

## Acceptance

A clean machine can reproduce or fetch a pinned Qt API DB and binding pack by artifact identity. Build/package the reference app for at least Linux GTK, Linux QtWidgets and Linux egui, with Windows/macOS Qt/egui packaging jobs added where CI runners/toolkits are available. Cache hits demonstrably skip expensive extraction/native bridge builds.
