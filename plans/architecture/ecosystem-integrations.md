# Ecosystem integration boundaries

Svelters should reuse nearby projects only at semantic boundaries where they remove duplicated infrastructure or provide a valuable independent oracle. A similar problem domain is not sufficient reason to create a hard dependency.

Two integrations are foundational but at different layers: Switchyard owns live executable/plugin generations once the plugin phase lands, while appfs owns application/platform storage beginning in the host-runtime foundation. Neither belongs inside the logical renderer graph itself.

## Switchyard: shared hot-reload/plugin substrate

Svelters and egui-workbench both need generation-safe dynamic extensions, so generic requirements belong in Switchyard rather than in parallel private loaders.

Svelters should depend on or drive upstream Switchyard work for:

- globally unambiguous generation identity such as `(PluginId, GenerationId)`;
- staged candidate publication so a plugin generation and host-visible contribution/capability set commit atomically;
- last-known-good rollback semantics;
- real uninstall/retirement semantics;
- structured build/watch diagnostics instead of scraping Cargo text;
- stable host-owned resource/callback handles across reload boundaries;
- a versioned optional native-extension descriptor/vtable for high-frequency typed fast paths.

The native extension mechanism must remain generic. Svelters defines a `svelters.renderer` extension schema on top of it; Switchyard must not learn about Svelte nodes or GUI patches.

## appfs: default application storage substrate

Appfs provides semantic Config, Data, State, Cache, Runtime, Temp, Logs, Resources and External spaces with explicit durability/capability semantics, typed documents, memory testing and native/object-store providers. Svelters adopts it as the default application/platform storage implementation rather than inventing another filesystem/persistence facade.

This does not make appfs part of rendering. `svelters-graph`, Svelte reconciliation, native binding resolution, and renderer backends remain storage-independent. The native application host owns the appfs instance and exposes a capability-scoped asynchronous storage service to JavaScript.

SVR05 must validate appfs according to its own compile/conformance/black-box/crash-recovery protocol before pinning it as a required release dependency. Generic fixes belong upstream in appfs. The validation gate protects Svelters from depending on an unverified generated archive; it does not make appfs optional after the gate passes.

Use appfs for:

- application Config/Data/State/Cache/Runtime/Temp/Logs/Resources/External spaces;
- Svelters-owned runtime/session metadata and crash/HMR restoration state under a reserved namespace;
- packaged application resource access;
- platform-correct storage locations without hard-coded path helpers;
- headless storage tests through `appfs-memory`.

Do not use appfs for Artifactum's immutable reproducible build outputs, Switchyard live generations, or secrets/keychain material. See `plans/architecture/application-storage.md`.

## egui-workbench: sibling and dogfood consumer

Do not make `egui-workbench` a Svelters core dependency and do not adopt its workbench UI protocol as the Svelters graph protocol. The products have different abstraction levels: Workbench describes application-shell contributions; Svelters describes Svelte-rendered logical UI.

Align the reusable concepts instead:

- stable semantic widget identity mapped to `egui::Id`;
- deferred event/command delivery rather than arbitrary callbacks during rendering;
- revisioned host/extension updates with explicit acknowledgement;
- last-known-good behavior on invalid extension output;
- bounded validation of plugin-supplied UI data;
- staged Switchyard publication and structured build/watch diagnostics;
- agent-visible semantic observation.

After the Svelters egui renderer and custom-widget SDK exist, add an optional `svelters-egui-workbench` adapter. It mounts a Svelte component into an existing Workbench pane/`egui::Ui` and must retain Svelte HMR without rebuilding Rust. This is a dogfood integration and architectural embedding test, not a foundation.

## desktop-cli: external accessibility and agent oracle

`desktop-cli` is GPL-3.0-only and must remain an external executable/test dependency, never a linked Svelters library dependency.

The portable semantic/accessibility model is foundational in SVR03 and is defined in `plans/architecture/semantic-graph.md`. Use desktop-cli later as an independent black-box oracle against running Svelters applications:

- observe the native accessibility tree on Windows UIA, Linux AT-SPI2 and macOS AX;
- locate widgets by role/name/state rather than Svelters internals;
- invoke activate/focus/set-value/select/scroll actions;
- verify stable semantic behavior across GTK4, QtWidgets and egui;
- compare external observations with Svelters' canonical semantic graph/devtools output.

This catches failures that an internal TestHost cannot see: missing accessibility names, incorrect native roles, inaccessible custom widgets and agent-visible regressions.

## deterministic-rs: optional late-stage simulation/replay testing

Do not put deterministic-rs on the production runtime or early implementation critical path. Svelters is a desktop UI framework, not a distributed simulator.

Once the event-loop, HMR and Switchyard contracts are stable, a small optional test adapter may route virtual time, timers, queued events and controlled reload/failure decisions through deterministic-rs. The useful target is reproducible bugs such as:

- timer completion racing component destruction;
- queued native event arriving after a renderer generation swap;
- HMR update racing an async callback;
- failed candidate publication at each checkpoint;
- shutdown with pending JS jobs/tasks/storage completions.

If this does not find/reduce real bugs more effectively than normal model/property tests, keep it out of the normal dependency graph.

## daemonkit: optional resident dev-helper lifecycle

SVR06 should begin with the simplest authenticated local Vite/Svelters dev transport. Do not add a daemon merely because daemonkit exists.

If later measurements show a resident helper materially improves startup, shared caches or multi-application development, daemonkit is the preferred lifecycle owner. It already supplies authenticated Unix sockets/named pipes, startup serialization, readiness publication, stale-state repair and crash cleanup. In that design daemonkit owns `svelters-devd` lifecycle; Vite/Svelters still own the application protocol. Svelters-owned local runtime metadata follows the appfs Runtime-space policy rather than introducing another public path convention.

## Artifactum and Outboard

Artifactum remains the preferred store/provenance layer for expensive reproducible derived artifacts: Qt/Shiboken and GTK API databases, generated declarations, fat development bridges, thin binding packs, QuickJS bytecode bundles and compatibility reports.

Outboard remains appropriate for independently installed process-bound tooling such as extractor frontends, packagers, signing/notarization helpers and report generators. It is deliberately not used for per-frame rendering or event traffic.

## Dependency rule

The intended dependency shape is:

```text
svelters host/runtime
   -> appfs / appfs-native

headless host tests
   -> appfs-memory

svelters renderer/plugin layer (from SVR15)
   -> switchyard

rendering graph
   -X-> appfs
   -X-> egui-workbench

optional integrations
   -> egui-workbench adapter

test/development executables
   -> desktop-cli
   -> deterministic-rs adapter
   -> daemonkit only if resident devd is justified

build/provenance tooling
   -> artifactum
   -> outboard
```

Any proposal to move an optional/test/tooling project into Svelters core must demonstrate a concrete API or correctness benefit and measure dependency/build/runtime cost. Appfs is the explicit exception for application storage because duplicating platform storage semantics would itself create a competing framework subsystem.
