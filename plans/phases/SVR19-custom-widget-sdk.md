# SVR19 - custom widget and plugin SDK

References: `plans/architecture/ecosystem-integrations.md`.

## Goal

Let applications and third parties add renderer-specific widgets, converters and commands without editing Svelters core, then dogfood the egui embedding contract inside egui-workbench.

## SDK

Provide Rust APIs/macros or code-generation helpers conceptually equivalent to:

```text
#[svelters_widget]
#[svelters_command]
#[svelters_converter]
```

A widget extension declares stable type/binding IDs, graph properties/events, backend compatibility, state checkpoint/restore hooks, and renderer implementation. egui extensions render immediate operations; GTK/Qt extensions may own native objects through the same handle/lifetime system.

Package extensions as Switchyard static or trusted-native plugins. Non-renderer services may use process/Wasm backends when isolation is valuable.

Generate TypeScript/Svelte declarations from extension manifests so custom widgets are typed exactly like built-in/native ones.

Define namespace/version conflict rules and deterministic capability resolution. An extension cannot shadow a canonical built-in type silently.

## egui-workbench dogfood adapter

Add an optional integration crate/package (working name `svelters-egui-workbench`) rather than a core dependency. It mounts a Svelte component/root into an egui-workbench Pane or equivalent caller-owned egui surface using the SVR14 embedding contract.

The adapter must:

- use Workbench's existing application-shell lifecycle rather than creating a second eframe event loop;
- preserve Svelte HMR and stable NodeIdentity-derived egui state;
- route Svelters events after egui borrows end;
- share Switchyard generation/publication infrastructure rather than embedding another loader;
- keep Workbench commands/documents/services separate from the Svelters logical UI graph unless an explicit adapter maps them.

This is a consumer and architecture test. Svelters core must still build and run without egui-workbench.

## Acceptance

- build a nontrivial example extension, preferably a plot or terminal-like widget, for at least egui and one retained backend;
- hot-reload the native extension generation while a Svelte app remains running;
- its TS declarations, events, properties and release binding roots are generated automatically;
- mount a Svelte component into the egui-workbench demo through the optional adapter;
- edit that component and observe HMR without rebuilding egui-workbench or Rust.
