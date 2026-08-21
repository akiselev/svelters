# Ecosystem integration boundaries

Svelters should reuse nearby projects only at semantic boundaries where they remove duplicated infrastructure or provide a valuable independent oracle. A similar problem domain is not sufficient reason to create a hard dependency.

## Switchyard: shared hot-reload/plugin substrate

Switchyard is the only direct foundational integration in this list. Svelters and egui-workbench both need generation-safe dynamic extensions, so generic requirements belong in Switchyard rather than in parallel private loaders.

Svelters should depend on or drive upstream Switchyard work for:

- globally unambiguous generation identity such as `(PluginId, GenerationId)`;
- staged candidate publication so a plugin generation and host-visible contribution/capability set commit atomically;
- last-known-good rollback semantics;
- real uninstall/retirement semantics;
- structured build/watch diagnostics instead of scraping Cargo text;
- stable host-owned resource/callback handles across reload boundaries;
- a versioned optional native-extension descriptor/vtable for high-frequency typed fast paths.

The native extension mechanism must remain generic. Svelters defines a `svelters.renderer` extension schema on top of it; Switchyard must not learn about Svelte nodes or GUI patches.

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

## appfs: optional application platform service

Appfs provides semantic Config, Data, State, Cache, Runtime, Temp, Logs, Resources and External spaces with explicit durability/capability semantics, typed documents, memory testing and native/object-store providers. That is a better basis for native-app storage than inventing a Svelters-specific filesystem facade.

Do not make appfs part of rendering, QuickJS execution or the core graph. Integrate it only after appfs has passed its own required compile, conformance and crash-recovery validation.

Expose an optional host module/package (working name `@svelters/appfs`) with Promise-based APIs for semantic spaces. Native operations must not block the UI thread. Resources can back packaged application assets; Config/State can support app settings and restoration; External represents explicitly granted user files/directories. Applications remain free to use other Rust or JS storage layers.

## desktop-cli: external accessibility and agent oracle

`desktop-cli` is GPL-3.0-only and must remain an external executable/test dependency, never a linked Svelters library dependency.

Use it as an independent black-box oracle against running Svelters applications:

- observe the native accessibility tree on Windows UIA, Linux AT-SPI2 and macOS AX;
- locate widgets by role/name/state rather than Svelters internals;
- invoke activate/focus/set-value/select/scroll actions;
- verify stable semantic behavior across GTK4, QtWidgets and egui;
- compare external observations with Svelters' internal devtools graph.

This catches failures that an internal TestHost cannot see: missing accessibility names, incorrect native roles, inaccessible custom widgets and agent-visible regressions.

## deterministic-rs: optional late-stage simulation/replay testing

Do not put deterministic-rs on the runtime or early implementation critical path. Svelters is a desktop UI framework, not a distributed simulator.

Once the event-loop, HMR and Switchyard contracts are stable, a small optional test adapter may route virtual time, timers, queued events and controlled reload/failure decisions through deterministic-rs. The useful target is reproducible bugs such as:

- timer completion racing component destruction;
- queued native event arriving after a renderer generation swap;
- HMR update racing an async callback;
- failed candidate publication at each checkpoint;
- shutdown with pending JS jobs/tasks.

If this does not find/reduce real bugs more effectively than normal model/property tests, keep it out.

## daemonkit: optional resident dev-helper lifecycle

SVR06 should begin with the simplest authenticated local Vite/Svelters dev transport. Do not add a daemon merely because daemonkit exists.

If later measurements show a resident helper materially improves startup, shared caches or multi-application development, daemonkit is the preferred lifecycle owner. It already supplies authenticated Unix sockets/named pipes, startup serialization, readiness publication, stale-state repair and crash cleanup. In that design daemonkit owns `svelters-devd` lifecycle; Vite/Svelters still own the application protocol.

## Artifactum and Outboard

Artifactum remains the preferred store/provenance layer for expensive reproducible derived artifacts: Qt/Shiboken and GTK API databases, generated declarations, fat development bridges, thin binding packs, QuickJS bytecode bundles and compatibility reports.

Outboard remains appropriate for independently installed process-bound tooling such as extractor frontends, packagers, signing/notarization helpers and report generators. It is deliberately not used for per-frame rendering or event traffic.

## Dependency rule

The intended dependency shape is:

```text
svelters-core/runtime
   -> switchyard (when plugin phase lands)

optional integrations
   -> appfs
   -> egui-workbench adapter

test/development executables
   -> desktop-cli
   -> deterministic-rs adapter
   -> daemonkit only if resident devd is justified

build/provenance tooling
   -> artifactum
   -> outboard
```

Any proposal to move one of the optional/test/tooling projects into Svelters core must demonstrate a concrete API or correctness benefit and measure the dependency/build/runtime cost.